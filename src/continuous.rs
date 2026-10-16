//! Nonhardware specification model. No transport, threads, randomness or device authority.
//!
//! This module deliberately cannot start a continuous session. Passing these rules does
//! not establish restricted pipe inheritance, authentic receipt origin or physical cleanup.
//! All times are durations from ONE local monotonic origin, never wire timestamps.
use std::time::Duration;

const RESPONSE_BUDGET: Duration = Duration::from_secs(2);
const LEASE: Duration = Duration::from_secs(8);
const INITIALIZATION: Duration = Duration::from_secs(60);
const INACTIVITY: Duration = Duration::from_secs(5);
const REPORT_AGE: Duration = Duration::from_millis(100);
pub const RECORD_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Protocol,
    Clock,
    Lease,
    Initialization,
    NoTransfer,
    NoFrame,
    OwnerUnresponsive,
    Drain,
    Cleanup,
    Receipt,
    Exit,
    Reap,
    Unlock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Response = 1,
    Cancel = 2,
    Receipt = 3,
}

/// Fixed records only; identity/binding are never formatted or logged by this module.
/// There is no cryptographic authenticity claim without the required OS capability.
#[derive(Clone, PartialEq, Eq)]
pub struct Record {
    pub kind: Kind,
    pub session: [u8; 32],
    pub binding: [u8; 32],
    pub sequence: u64,
    pub challenge: [u8; 32],
}
impl Record {
    pub fn encode(&self) -> [u8; RECORD_BYTES] {
        let mut b = [0; RECORD_BYTES];
        b[..8].copy_from_slice(&[b'C', b'S', 1, self.kind as u8, 128, 0, 0, 0]);
        b[8..40].copy_from_slice(&self.session);
        b[40..72].copy_from_slice(&self.binding);
        b[72..80].copy_from_slice(&self.sequence.to_le_bytes());
        b[80..112].copy_from_slice(&self.challenge);
        b
    }
    pub fn decode(b: &[u8]) -> Result<Self, Fault> {
        if b.len() != RECORD_BYTES
            || b[..3] != [b'C', b'S', 1]
            || b[4..8] != [128, 0, 0, 0]
            || b[112..].iter().any(|x| *x != 0)
        {
            return Err(Fault::Protocol);
        }
        let kind = match b[3] {
            1 => Kind::Response,
            2 => Kind::Cancel,
            3 => Kind::Receipt,
            _ => return Err(Fault::Protocol),
        };
        Ok(Self {
            kind,
            session: b[8..40].try_into().unwrap(),
            binding: b[40..72].try_into().unwrap(),
            sequence: u64::from_le_bytes(b[72..80].try_into().unwrap()),
            challenge: b[80..112].try_into().unwrap(),
        })
    }
}

/// One instance per direction. Commit only after ALL message semantics validate.
pub struct Direction {
    session: [u8; 32],
    binding: [u8; 32],
    last: u64,
    kind: Kind,
}
impl Direction {
    pub fn new(session: [u8; 32], binding: [u8; 32], kind: Kind) -> Self {
        Self {
            session,
            binding,
            last: 0,
            kind,
        }
    }
    fn validate(&self, b: &[u8]) -> Result<Record, Fault> {
        let r = Record::decode(b)?;
        if r.session != self.session
            || r.binding != self.binding
            || r.kind != self.kind
            || self.last.checked_add(1) != Some(r.sequence)
        {
            return Err(Fault::Protocol);
        }
        Ok(r)
    }
}

/// Helper supervisor rules, also reusable for the parent's worker challenge lease.
/// Production acquisition must NOT hold a mutable reference to this object.
pub struct Guards {
    responses: Direction,
    now: Duration,
    accepted: Duration,
    last_response: Duration,
    outstanding: Option<([u8; 32], Duration)>,
    previous_challenge: Option<[u8; 32]>,
    response_proved: bool,
    enabled: Option<Duration>,
    transfer: Option<Duration>,
    frame: Option<Duration>,
    first_frame: bool,
    transfer_sequence: u64,
    frame_sequence: u64,
    pub cancelled: bool,
    pub first_fault: Option<Fault>,
}
impl Guards {
    pub fn new(session: [u8; 32], binding: [u8; 32], now: Duration) -> Self {
        Self {
            responses: Direction::new(session, binding, Kind::Response),
            now,
            accepted: now,
            last_response: now,
            outstanding: None,
            previous_challenge: None,
            response_proved: false,
            enabled: None,
            transfer: None,
            frame: None,
            first_frame: false,
            transfer_sequence: 0,
            frame_sequence: 0,
            cancelled: false,
            first_fault: None,
        }
    }
    pub fn fault(&mut self, reason: Fault) {
        self.first_fault.get_or_insert(reason);
        self.cancelled = true;
    }
    pub fn stop(&mut self) {
        self.cancelled = true;
    }
    pub fn eof(&mut self) {
        self.fault(Fault::Lease);
    }
    pub fn tick(&mut self, now: Duration) {
        if now < self.now {
            self.fault(Fault::Clock);
            return;
        }
        self.now = now;
        if self.cancelled {
            return;
        }
        if now - self.last_response >= LEASE {
            self.fault(Fault::Lease);
        } else if !self.first_frame && now - self.accepted >= INITIALIZATION {
            self.fault(Fault::Initialization);
        } else if self.transfer.is_some_and(|t| now - t >= INACTIVITY) {
            self.fault(Fault::NoTransfer);
        } else if self.frame.is_some_and(|t| now - t >= INACTIVITY) {
            self.fault(Fault::NoFrame);
        }
    }
    /// The caller must supply OS randomness; this model cannot certify its origin.
    /// Missed challenges may be replaced only after their deadline, never queued.
    pub fn challenge(&mut self, value: [u8; 32], now: Duration) -> Result<(), Fault> {
        self.tick(now);
        if self.cancelled {
            return Err(self.first_fault.unwrap_or(Fault::Protocol));
        }
        if value == [0; 32]
            || self.previous_challenge == Some(value)
            || self
                .outstanding
                .is_some_and(|(_, issued)| now - issued < RESPONSE_BUDGET)
        {
            self.fault(Fault::Protocol);
            return Err(Fault::Protocol);
        }
        self.previous_challenge = Some(value);
        self.outstanding = Some((value, now));
        Ok(())
    }
    pub fn next_response_sequence(&self) -> Option<u64> {
        self.responses.last.checked_add(1)
    }
    /// A late otherwise valid response consumes its sequence/challenge but grants no lease.
    pub fn respond(&mut self, b: &[u8], now: Duration) -> Result<bool, Fault> {
        self.tick(now);
        if self.cancelled {
            return Ok(false);
        }
        let result = self.responses.validate(b).and_then(|r| {
            let (challenge, issued) = self.outstanding.ok_or(Fault::Protocol)?;
            if r.challenge != challenge {
                return Err(Fault::Protocol);
            }
            Ok((r, issued))
        });
        let (r, issued) = match result {
            Ok(r) => r,
            Err(e) => {
                self.fault(e);
                return Err(e);
            }
        };
        self.outstanding = None;
        self.responses.last = r.sequence;
        if now - issued >= RESPONSE_BUDGET {
            return Ok(false);
        }
        self.last_response = now;
        self.response_proved = true;
        Ok(true)
    }
    pub fn enable_live(&mut self, now: Duration) -> bool {
        self.tick(now);
        if self.cancelled || !self.response_proved || self.enabled.is_some() {
            self.fault(Fault::Protocol);
            return false;
        }
        self.enabled = Some(now);
        self.transfer = Some(now);
        self.frame = Some(now);
        true
    }
    /// Bounded capacity-one event mailbox must deliver same-process sampled_at.
    /// Receipt time is authoritative; delayed or duplicate events fail closed.
    pub fn report(&mut self, frame: bool, sequence: u64, sampled_at: Duration, now: Duration) {
        self.tick(now);
        if self.cancelled {
            return;
        }
        let last = if frame {
            self.frame_sequence
        } else {
            self.transfer_sequence
        };
        if self.enabled.is_none()
            || sampled_at > now
            || now - sampled_at > REPORT_AGE
            || sequence <= last
        {
            self.fault(Fault::Protocol);
            return;
        }
        if frame {
            self.first_frame = true;
            self.frame_sequence = sequence;
            self.frame = Some(now);
        } else {
            self.transfer_sequence = sequence;
            self.transfer = Some(now);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofState {
    Draining,
    Cleaning,
    HandlesReleased,
    Receipt,
    Exited,
    Reaped,
    Released,
    Unproved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proof {
    PendingZero,
    CleanupAndHandlesReleased,
    ValidatedReceipt,
    AcceptableExit,
    Reaped,
    ExactSessionUnlock,
}
/// Models required ordering only. Events must originate from independent validated
/// observations in a future adapter. No filesystem removal or synthetic success here.
pub struct Shutdown {
    pub state: ProofState,
    pub first_fault: Option<Fault>,
    deadline: Duration,
    now: Duration,
}
impl Shutdown {
    pub fn new(now: Duration, first_fault: Option<Fault>) -> Self {
        Self {
            state: ProofState::Draining,
            first_fault,
            deadline: now.saturating_add(Duration::from_secs(1)),
            now,
        }
    }
    pub fn fail(&mut self, reason: Fault) {
        self.first_fault.get_or_insert(reason);
        self.state = ProofState::Unproved;
    }
    pub fn tick(&mut self, now: Duration) {
        if now < self.now {
            self.fail(Fault::Clock);
            return;
        }
        self.now = now;
        if matches!(self.state, ProofState::Released | ProofState::Unproved) {
            return;
        }
        if now >= self.deadline {
            self.fail(match self.state {
                ProofState::Draining => Fault::Drain,
                ProofState::Cleaning => Fault::Cleanup,
                ProofState::HandlesReleased => Fault::Receipt,
                ProofState::Receipt => Fault::Exit,
                ProofState::Exited => Fault::Reap,
                _ => Fault::Unlock,
            });
        }
    }
    pub fn observe(&mut self, proof: Proof, now: Duration) {
        self.tick(now);
        if matches!(self.state, ProofState::Unproved | ProofState::Released) {
            return;
        }
        match (self.state, proof) {
            (ProofState::Draining, Proof::PendingZero) => {
                self.state = ProofState::Cleaning;
                self.deadline = now.saturating_add(Duration::from_secs(5));
            }
            (ProofState::Cleaning, Proof::CleanupAndHandlesReleased) => {
                self.state = ProofState::HandlesReleased;
                self.deadline = now.saturating_add(Duration::from_secs(2));
            }
            (ProofState::HandlesReleased, Proof::ValidatedReceipt) => {
                self.state = ProofState::Receipt
            }
            (ProofState::Receipt, Proof::AcceptableExit) => self.state = ProofState::Exited,
            (ProofState::Exited, Proof::Reaped) => self.state = ProofState::Reaped,
            (ProofState::Reaped, Proof::ExactSessionUnlock) => self.state = ProofState::Released,
            _ => self.fail(Fault::Protocol),
        }
    }
    pub fn clean_cancel(&self) -> bool {
        self.state == ProofState::Released && self.first_fault.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }
    fn guard() -> Guards {
        Guards::new([1; 32], [2; 32], ms(0))
    }
    fn response(sequence: u64, challenge: [u8; 32]) -> [u8; RECORD_BYTES] {
        Record {
            kind: Kind::Response,
            session: [1; 32],
            binding: [2; 32],
            sequence,
            challenge,
        }
        .encode()
    }
    fn renew(g: &mut Guards, sequence: u64, t: u64) {
        let mut challenge = [3; 32];
        challenge[..8].copy_from_slice(&sequence.to_le_bytes());
        g.challenge(challenge, ms(t)).unwrap();
        assert_eq!(g.respond(&response(sequence, challenge), ms(t)), Ok(true));
    }
    #[test]
    fn parser_exact_bounds_and_all_reserved_bytes() {
        let b = response(1, [3; 32]);
        for length in 0..RECORD_BYTES {
            assert!(Record::decode(&b[..length]).is_err());
        }
        let mut oversized = b.to_vec();
        oversized.push(0);
        assert!(Record::decode(&oversized).is_err());
        for i in (0..8).filter(|i| *i != 3).chain(112..128) {
            let mut corrupt = b;
            corrupt[i] ^= 0x80;
            assert!(Record::decode(&corrupt).is_err(), "byte {i}");
        }
        for kind in [0, 4, 255] {
            let mut corrupt = b;
            corrupt[3] = kind;
            assert!(Record::decode(&corrupt).is_err());
        }
        assert!(Record::decode(&b).unwrap() == Record::decode(&b).unwrap());
    }
    #[test]
    fn corrupt_identity_binding_sequence_type_challenge_latch_first_fault() {
        for i in 3..112 {
            if (4..8).contains(&i) {
                continue;
            }
            let mut g = guard();
            g.challenge([3; 32], ms(0)).unwrap();
            let mut b = response(1, [3; 32]);
            b[i] ^= 0x40;
            assert_eq!(g.respond(&b, ms(1)), Err(Fault::Protocol), "byte {i}");
            assert_eq!(g.responses.last, 0);
            g.eof();
            g.tick(ms(100_000));
            assert_eq!(g.first_fault, Some(Fault::Protocol));
            assert!(!g.enable_live(ms(100_000)));
        }
    }
    #[test]
    fn lease_exact_boundary_replay_eof_and_no_resurrection() {
        let mut g = guard();
        renew(&mut g, 1, 0);
        g.tick(ms(7999));
        assert!(!g.cancelled);
        g.tick(ms(8000));
        assert_eq!(g.first_fault, Some(Fault::Lease));
        assert_eq!(g.respond(&response(2, [4; 32]), ms(8001)), Ok(false));
        let mut g = guard();
        renew(&mut g, 1, 0);
        assert_eq!(
            g.respond(&response(1, [3; 32]), ms(1)),
            Err(Fault::Protocol)
        );
        let mut g = guard();
        g.eof();
        assert!(g.cancelled);
        let mut g = guard();
        g.stop();
        g.eof();
        assert!(g.cancelled);
        assert_eq!(g.first_fault, Some(Fault::Lease));
    }
    #[test]
    fn delayed_backlog_and_sequence_exhaustion_never_renew() {
        let mut g = guard();
        g.challenge([3; 32], ms(0)).unwrap();
        assert_eq!(g.respond(&response(1, [3; 32]), ms(2000)), Ok(false));
        assert!(!g.response_proved);
        g.tick(ms(7999));
        assert!(!g.cancelled);
        g.tick(ms(8000));
        assert!(g.cancelled);
        let mut g = guard();
        g.responses.last = u64::MAX;
        g.challenge([3; 32], ms(0)).unwrap();
        assert_eq!(
            g.respond(&response(0, [3; 32]), ms(1)),
            Err(Fault::Protocol)
        );
    }
    #[test]
    fn initialization_includes_first_frame_at_exact_deadline() {
        for first in [59_999, 60_000] {
            let mut g = guard();
            for n in 0..30 {
                renew(&mut g, n + 1, n * 2000);
            }
            assert!(g.enable_live(ms(59_000)));
            g.report(false, 1, ms(first), ms(first));
            g.report(true, 1, ms(first), ms(first));
            assert_eq!(
                g.first_fault,
                (first == 60_000).then_some(Fault::Initialization)
            );
        }
    }
    #[test]
    fn no_data_and_fragments_only_have_independent_exact_deadlines() {
        for fragments in [false, true] {
            let mut g = guard();
            renew(&mut g, 1, 0);
            assert!(g.enable_live(ms(0)));
            g.report(true, 1, ms(0), ms(0));
            for n in 1..=2 {
                renew(&mut g, n + 1, n * 2000);
                if fragments {
                    g.report(false, n, ms(n * 2000), ms(n * 2000));
                }
            }
            g.tick(ms(4999));
            assert!(!g.cancelled);
            g.tick(ms(5000));
            assert_eq!(
                g.first_fault,
                Some(if fragments {
                    Fault::NoFrame
                } else {
                    Fault::NoTransfer
                })
            );
        }
    }
    #[test]
    fn event_backlog_duplicate_and_clock_regression_fail_closed() {
        for age in [100, 101] {
            let mut g = guard();
            renew(&mut g, 1, 0);
            g.enable_live(ms(0));
            g.report(false, 1, ms(0), ms(age));
            assert_eq!(g.cancelled, age > 100);
            if age == 100 {
                g.report(false, 1, ms(100), ms(100));
                assert_eq!(g.first_fault, Some(Fault::Protocol));
            }
        }
        let mut g = guard();
        g.tick(ms(1));
        g.tick(ms(0));
        assert_eq!(g.first_fault, Some(Fault::Clock));
    }
    #[test]
    fn six_hours_single_initialization_fixed_storage_then_stop() {
        let mut g = guard();
        renew(&mut g, 1, 0);
        assert!(g.enable_live(ms(0)));
        let size = std::mem::size_of_val(&g);
        for n in 1..=10_800 {
            let t = n * 2000;
            renew(&mut g, n + 1, t);
            g.report(false, n, ms(t), ms(t));
            g.report(true, n, ms(t), ms(t));
            assert!(!g.cancelled);
        }
        assert_eq!(g.enabled, Some(ms(0)));
        assert_eq!(size, std::mem::size_of_val(&g)); // no Vec, channel or frame/log accumulation
        g.stop();
        g.report(true, 10_801, ms(21_600_001), ms(21_600_001));
        assert_eq!(g.frame_sequence, 10_800);
        assert!(!g.enable_live(ms(21_600_001)));
    }
    const PROOFS: [Proof; 6] = [
        Proof::PendingZero,
        Proof::CleanupAndHandlesReleased,
        Proof::ValidatedReceipt,
        Proof::AcceptableExit,
        Proof::Reaped,
        Proof::ExactSessionUnlock,
    ];
    #[test]
    fn every_missing_failed_or_reordered_proof_prevents_clean_release() {
        for skip in 0..6 {
            let mut s = Shutdown::new(ms(0), None);
            for (i, p) in PROOFS.iter().enumerate() {
                if i == skip {
                    continue;
                }
                s.observe(*p, ms(i as u64 + 1));
            }
            s.tick(ms(10_000));
            assert_eq!(s.state, ProofState::Unproved);
            assert!(!s.clean_cancel());
        }
        for fault in [
            Fault::Drain,
            Fault::Cleanup,
            Fault::Receipt,
            Fault::Exit,
            Fault::Reap,
            Fault::Unlock,
        ] {
            let mut s = Shutdown::new(ms(0), Some(Fault::NoFrame));
            s.fail(fault);
            for p in PROOFS {
                s.observe(p, ms(1));
            }
            assert_eq!(s.state, ProofState::Unproved);
            assert_eq!(s.first_fault, Some(Fault::NoFrame));
        }
        for fault in [None, Some(Fault::NoTransfer)] {
            let mut s = Shutdown::new(ms(0), fault);
            for p in PROOFS {
                s.observe(p, ms(1));
            }
            assert_eq!(s.state, ProofState::Released);
            assert_eq!(s.clean_cancel(), fault.is_none());
            assert_eq!(s.first_fault, fault);
        }
    }
    #[test]
    fn drain_cleanup_and_reap_deadlines_do_not_reset() {
        for (steps, deadline, fault) in [
            (0, 1000, Fault::Drain),
            (1, 5001, Fault::Cleanup),
            (2, 2002, Fault::Receipt),
            (3, 2002, Fault::Exit),
            (4, 2002, Fault::Reap),
            (5, 2002, Fault::Unlock),
        ] {
            let mut s = Shutdown::new(ms(0), None);
            for (n, p) in PROOFS.iter().take(steps).enumerate() {
                s.observe(*p, ms(n as u64 + 1));
            }
            s.tick(ms(deadline - 1));
            assert_ne!(s.state, ProofState::Unproved);
            s.tick(ms(deadline));
            assert_eq!(s.state, ProofState::Unproved);
            assert_eq!(s.first_fault, Some(fault));
            for p in PROOFS {
                s.observe(p, ms(deadline));
            }
            assert_ne!(s.state, ProofState::Released);
        }
    }
}
