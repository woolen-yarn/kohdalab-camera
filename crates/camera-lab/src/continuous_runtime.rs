//! Independent, fixed-memory acquisition supervision. OwnerPort cannot renew a
//! lease. LeasePort belongs exclusively to the external parent-IPC adapter.
//! This component is not a bootstrap, persistent-lock or terminal-release API.
use crate::continuous::{Fault, Guards};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Duration;
}
pub struct Monotonic(Instant);
impl Default for Monotonic {
    fn default() -> Self {
        Self(Instant::now())
    }
}
impl Clock for Monotonic {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

#[derive(Default)]
struct Reports {
    enabled: Option<Duration>,
    transfer: Option<(u64, Duration)>,
    frame: Option<(u64, Duration)>,
    transfers: u64,
    frames: u64,
}
struct Shared {
    phase: std::sync::atomic::AtomicU8,
    cancelled: AtomicBool,
    authorized: AtomicBool,
    first_fault: Mutex<Option<Fault>>,
    reports: Mutex<Reports>,
    frame: Mutex<Option<crate::Frame>>,
    frame_ready: std::sync::Condvar,
    frame_publication_ready: AtomicBool,
    measurement: Mutex<crate::measurement::Source>,
    clock: Arc<dyn Clock>,
}
impl Shared {
    fn fail(&self, fault: Fault) {
        let mut first = self.first_fault.lock().unwrap_or_else(|e| e.into_inner());
        first.get_or_insert(fault);
        self.cancelled.store(true, Ordering::Release);
    }
}

/// Only the acquisition thread receives this capability. No protocol response,
/// settings/session identity, challenge, lease timestamp or mutable Guards here.
pub struct OwnerPort(Arc<Shared>);
impl OwnerPort {
    pub fn phase(&self, phase: u8) {
        self.0.phase.store(phase, Ordering::Release);
    }
    pub fn authorized(&self) -> bool {
        self.0.authorized.load(Ordering::Acquire)
    }
    /// Capacity-one coalescing publication; never performs IO in acquisition.
    pub fn publish_frame(&self, mut frame: crate::Frame) -> Result<(), String> {
        self.checkpoint()?;
        frame.validate()?;
        if let Ok(m)=self.0.measurement.lock() {m.bind(&mut frame);}
        let mut slot = self.0.frame.lock().map_err(|_| "Frame mailbox poisoned")?;
        self.checkpoint()?;
        *slot = Some(frame);
        self.0.frame_ready.notify_one();
        Ok(())
    }
    pub fn checkpoint(&self) -> Result<(), String> {
        if self.0.cancelled.load(Ordering::Acquire) {
            Err(crate::physical::CANCELLED.into())
        } else if !self.0.authorized.load(Ordering::Acquire) {
            self.0.fail(Fault::Protocol);
            Err("Initial parent lease not proved; acquisition refused".into())
        } else {
            Ok(())
        }
    }
    pub fn enable_live(&self) -> Result<(), String> {
        self.checkpoint()?;
        let mut r = self
            .0
            .reports
            .lock()
            .map_err(|_| "Supervisor report storage poisoned")?;
        // STOP may win while this producer waits for the report mutex.
        self.checkpoint()?;
        if r.enabled.is_some() {
            self.0.fail(Fault::Protocol);
            return self.checkpoint();
        }
        r.enabled = Some(self.0.clock.now());
        Ok(())
    }
    fn report(&self, frame: bool) -> Result<(), String> {
        self.checkpoint()?;
        let mut r = self
            .0
            .reports
            .lock()
            .map_err(|_| "Supervisor report storage poisoned")?;
        self.checkpoint()?;
        let count = if frame { r.frames } else { r.transfers };
        let Some(next) = count.checked_add(1) else {
            self.0.fail(Fault::Protocol);
            return self.checkpoint();
        };
        let event = Some((next, self.0.clock.now()));
        if frame {
            r.frames = next;
            r.frame = event;
        } else {
            r.transfers = next;
            r.transfer = event;
        }
        Ok(())
    }
    pub fn nonempty_transfer(&self) -> Result<(), String> {
        self.report(false)
    }
    pub fn complete_frame(&self) -> Result<(), String> {
        self.report(true)?;
        if let Ok(mut m)=self.0.measurement.lock() {m.complete();}
        Ok(())
    }
}

#[derive(Clone)]
pub struct Monitor(Arc<Shared>);
impl Monitor {
    pub(crate) fn fault(&self, fault: Fault) {
        self.0.fail(fault);
    }
    pub fn phase(&self) -> u8 {
        self.0.phase.load(Ordering::Acquire)
    }
    /// Sticky side channel: never competes with the bounded command mailbox.
    pub fn pipe_lost(&self) {
        if !self.cancelled() {
            self.0.fail(Fault::Lease);
        }
    }
    /// Bounded coalesced readiness; timeout retains phase/STOP supervision cadence.
    pub fn wait_frame(&self, timeout:Duration) {
        let slot=self.0.frame.lock().unwrap_or_else(|e|e.into_inner());
        let _ = self.0.frame_ready.wait_timeout_while(slot,timeout,|f|!self.cancelled() && (f.is_none() || !self.0.frame_publication_ready.load(Ordering::Acquire)));
    }
    pub fn take_frame(&self) -> Option<crate::Frame> {
        if !self.0.frame_publication_ready.load(Ordering::Acquire) {return None;}
        let mut slot = self.0.frame.lock().unwrap_or_else(|e| e.into_inner());
        if self.cancelled() { return None; }
        slot.take()
    }
    pub fn stop(&self) {
        // Serialize producer events and delivery with STOP; fixed-size critical
        // sections only, with no IO or first-frame wait.
        let _reports = self.0.reports.lock().unwrap_or_else(|e| e.into_inner());
        let mut slot = self.0.frame.lock().unwrap_or_else(|e| e.into_inner());
        self.0.cancelled.store(true, Ordering::Release);
        slot.take();
        self.0.frame_ready.notify_all();
    }
    pub fn cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    pub fn first_fault(&self) -> Option<Fault> {
        *self.0.first_fault.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Kept by the independent IPC supervisor, never by a producer/preview/export.
/// All protocol semantic checks still execute in the production Guards parser.
pub struct LeasePort {
    guards: Guards,
    shared: Arc<Shared>,
    enabled: bool,
}
impl LeasePort {
    fn command(&mut self, command: SupervisorCommand) {
        match command {
            SupervisorCommand::Challenge(random) => {
                let _ = self.challenge(random);
            }
            SupervisorCommand::AcceptChallenge(random, ack) => {
                let accepted = self.accept_challenge(random);
                if ack.try_send(accepted).is_err() {
                    self.pipe_lost();
                }
            }
            SupervisorCommand::Response(record) => {
                let _ = self.response(&record);
            }
            SupervisorCommand::AcceptResponse(record, ack) => {
                let accepted = self.response(&record);
                if ack.try_send(accepted).is_err() {
                    self.pipe_lost();
                }
            }
            SupervisorCommand::PipeLost => self.pipe_lost(),
        }
    }
    pub fn tick(&mut self) {
        let reports = self.shared.reports.lock();
        match reports {
            Ok(mut r) => {
                // The producer samples under this same mutex. Sampling before
                // acquiring it can classify a valid intervening publication as
                // future-dated. Keep every guard and deadline unchanged.
                let now = self.shared.clock.now();
                self.guards.tick(now);
                if let Some(sampled) = r.enabled {
                    if !self.enabled {
                        if sampled > now || now.saturating_sub(sampled) > Duration::from_millis(100)
                        {
                            self.guards.fault(Fault::Protocol);
                        } else {
                            if self.guards.enable_live(now) {
                                // Presentation cannot outrun the supervisor's existing
                                // enable acceptance, particularly an immediate Full STOP.
                                self.shared.frame_publication_ready.store(true,Ordering::Release);
                                self.shared.frame_ready.notify_one();
                            }
                        }
                        self.enabled = true;
                    }
                }
                if let Some((sequence, sampled)) = r.transfer.take() {
                    self.guards.report(false, sequence, sampled, now);
                }
                if let Some((sequence, sampled)) = r.frame.take() {
                    self.guards.report(true, sequence, sampled, now);
                }
                // Validate submitted events before one-way guard cancellation.
                // STOP is already visible to producer/consumer, so this bounded
                // drain cannot deliver a first frame or postpone cancellation.
                if self.shared.cancelled.load(Ordering::Acquire) {
                    self.guards.stop();
                }
            }
            Err(_) => self.guards.fault(Fault::Protocol),
        }
        self.publish();
    }
    fn publish(&self) {
        if let Some(fault) = self.guards.first_fault {
            self.shared.fail(fault);
        } else if self.guards.cancelled {
            self.shared.cancelled.store(true, Ordering::Release);
        }
    }
    pub fn challenge(&mut self, random: [u8; 32]) -> Result<(), Fault> {
        let result = self.guards.challenge(random, self.shared.clock.now());
        self.publish();
        result
    }
    pub fn response(&mut self, record: &[u8]) -> Result<bool, Fault> {
        let result = self.guards.respond(record, self.shared.clock.now());
        if result == Ok(true) {
            self.shared.authorized.store(true, Ordering::Release);
        }
        self.publish();
        result
    }
    pub fn pipe_lost(&mut self) {
        // A deliberately closed IPC adapter after one-way STOP is teardown,
        // not a new lease failure. Preserve an earlier genuine fault verbatim.
        if !self.shared.cancelled.load(Ordering::Acquire) {
            self.guards.eof();
        }
        self.publish();
    }
}

pub fn session(
    session: [u8; 32],
    binding: [u8; 32],
    clock: Arc<dyn Clock>,
) -> (OwnerPort, LeasePort, Monitor) {
    let now = clock.now();
    let shared = Arc::new(Shared {
        phase: std::sync::atomic::AtomicU8::new(0),
        cancelled: AtomicBool::new(false),
        authorized: AtomicBool::new(false),
        first_fault: Mutex::new(None),
        reports: Mutex::new(Reports::default()),
        frame: Mutex::new(None),
        frame_ready: std::sync::Condvar::new(),
        frame_publication_ready: AtomicBool::new(false),
        measurement: Mutex::new(crate::measurement::Source::default()),
        clock,
    });
    (
        OwnerPort(shared.clone()),
        LeasePort {
            guards: Guards::new(session, binding, now),
            shared: shared.clone(),
            enabled: false,
        },
        Monitor(shared),
    )
}

/// A real independently runnable supervisor. Its adapter must be nonblocking;
/// native pipe/metadata/collection calls belong to retained I/O/collection owners.
/// Dropping the thread guard cancels the owner and joins this supervisor. It
/// cannot prove pending-zero, device Drop, process exit/reap or lock release.
pub struct SupervisorThread {
    end: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    monitor: Monitor,
}
#[derive(Debug)]
pub enum SupervisorCommand {
    Challenge([u8; 32]),
    AcceptChallenge(
        [u8; 32],
        std::sync::mpsc::SyncSender<Result<ChallengeAccepted, Fault>>,
    ),
    Response([u8; crate::continuous::RECORD_BYTES]),
    AcceptResponse(
        [u8; crate::continuous::RECORD_BYTES],
        std::sync::mpsc::SyncSender<Result<bool, Fault>>,
    ),
    PipeLost,
}
#[derive(Debug)]
pub struct ChallengeAccepted {
    pub sequence: u64,
    pub challenge: [u8; 32],
    pub accepted_at: Duration,
}
impl LeasePort {
    fn accept_challenge(&mut self, random: [u8; 32]) -> Result<ChallengeAccepted, Fault> {
        let now = self.shared.clock.now();
        let sequence = match self.guards.next_response_sequence() {
            Some(sequence) => sequence,
            None => {
                self.guards.fault(Fault::Protocol);
                self.publish();
                return Err(Fault::Protocol);
            }
        };
        let result = self.guards.challenge(random, now);
        self.publish();
        result.map(|_| ChallengeAccepted {
            sequence,
            challenge: random,
            accepted_at: now,
        })
    }
}
pub struct SupervisorInput(std::sync::mpsc::SyncSender<SupervisorCommand>);
pub struct SupervisorInbox(std::sync::mpsc::Receiver<SupervisorCommand>);
pub fn ipc_mailbox() -> (SupervisorInput, SupervisorInbox) {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    (SupervisorInput(tx), SupervisorInbox(rx))
}
impl SupervisorInput {
    pub fn try_send(
        &self,
        command: SupervisorCommand,
    ) -> Result<(), std::sync::mpsc::TrySendError<SupervisorCommand>> {
        self.0.try_send(command)
    }
}
impl SupervisorThread {
    pub fn spawn(mut lease: LeasePort, inbox: SupervisorInbox) -> std::io::Result<Self> {
        let end = Arc::new(AtomicBool::new(false));
        let ending = end.clone();
        let monitor = Monitor(lease.shared.clone());
        let failure = lease.shared.clone();
        let thread = std::thread::Builder::new()
            .name("continuous-supervisor".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    while !ending.load(Ordering::Acquire) {
                        // Fixed capacity and one command per iteration prevent
                        // I/O, unbounded callbacks or traffic from starving guards.
                        match inbox.0.try_recv() {
                            Ok(command) => lease.command(command),
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => lease.pipe_lost(),
                            Err(std::sync::mpsc::TryRecvError::Empty) => (),
                        }
                        lease.tick();
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }));
                if result.is_err() {
                    failure.fail(Fault::OwnerUnresponsive);
                }
            })?;
        Ok(Self {
            end,
            thread: Some(thread),
            monitor,
        })
    }
}
impl Drop for SupervisorThread {
    fn drop(&mut self) {
        self.monitor.stop();
        self.end.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continuous::{Kind, Record};
    use std::sync::atomic::AtomicU64;
    #[test]
    fn early_stop_pending_enable_cancels_cleanly() {
        let (owner,mut lease,monitor)=session([1;32],[2;32],Arc::new(FakeClock::default()));
        renew(&mut lease,1);owner.enable_live().unwrap();owner.complete_frame().unwrap();
        monitor.stop();lease.tick();assert_eq!(monitor.first_fault(),None);
        assert!(lease.enabled);assert!(lease.guards.cancelled);
        assert!(monitor.take_frame().is_none());assert!(owner.complete_frame().is_err());
    }
    #[test]
    fn early_stop_before_enable_rejects_producer_without_fault() {
        let (owner,mut lease,monitor)=session([1;32],[2;32],Arc::new(FakeClock::default()));
        renew(&mut lease,1);monitor.stop();
        assert!(owner.enable_live().is_err());lease.tick();
        assert!(!lease.enabled);assert_eq!(monitor.first_fault(),None);
        // The negative guard contract remains strict even after clean STOP.
        assert!(!lease.guards.enable_live(Duration::ZERO));
        assert_eq!(lease.guards.first_fault,Some(Fault::Protocol));
    }
    #[test]
    fn early_stop_enable_accepted_before_first_frame_and_after_first_frame() {
        for first_frame in [false,true] {
            let (owner,mut lease,monitor)=session([1;32],[2;32],Arc::new(FakeClock::default()));
            renew(&mut lease,1);owner.enable_live().unwrap();lease.tick();
            assert!(lease.enabled);
            if first_frame {owner.complete_frame().unwrap();lease.tick();}
            monitor.stop();lease.tick();assert_eq!(monitor.first_fault(),None);
            assert!(owner.complete_frame().is_err());assert!(monitor.take_frame().is_none());
        }
    }
    #[test]
    fn early_stop_preserves_sticky_fault_and_validates_pending_bad_event() {
        let (owner,mut lease,monitor)=session([1;32],[2;32],Arc::new(FakeClock::default()));
        renew(&mut lease,1);owner.enable_live().unwrap();
        lease.guards.fault(Fault::Lease);lease.publish();monitor.stop();lease.tick();
        assert_eq!(monitor.first_fault(),Some(Fault::Lease));
        let (owner,mut lease,monitor)=session([1;32],[2;32],Arc::new(FakeClock::default()));
        renew(&mut lease,1);owner.enable_live().unwrap();owner.complete_frame().unwrap();
        owner.0.reports.lock().unwrap().frame=Some((1,Duration::from_secs(1)));
        monitor.stop();lease.tick();assert_eq!(monitor.first_fault(),Some(Fault::Protocol));
    }
    #[test]
    fn early_stop_producer_waiting_on_enable_mutex_rechecks_cancellation() {
        let (owner,mut lease,monitor)=session([1;32],[2;32],Arc::new(FakeClock::default()));
        renew(&mut lease,1);
        let held=monitor.0.reports.lock().unwrap();
        let (tx,rx)=std::sync::mpsc::sync_channel(1);
        let thread=std::thread::spawn(move || {tx.send(()).unwrap();owner.enable_live()});
        rx.recv_timeout(Duration::from_secs(1)).unwrap();
        // Model cancellation becoming visible after the initial checkpoint but
        // before report-lock acquisition (also used by the fault side channel).
        monitor.0.cancelled.store(true,Ordering::Release);drop(held);
        assert!(thread.join().unwrap().is_err());lease.tick();
        assert!(!lease.enabled);assert_eq!(monitor.first_fault(),None);
    }
    #[test]
    fn frame_notifications_coalesce_and_stop_keeps_authority() {
        let (owner,mut lease,monitor)=session([1;32],[2;32],Arc::new(FakeClock::default()));
        renew(&mut lease,1);owner.enable_live().unwrap();
        let (entered,ready)=std::sync::mpsc::sync_channel(1);let waiting=monitor.clone();
        let thread=std::thread::spawn(move || {entered.send(()).unwrap();waiting.wait_frame(Duration::from_secs(2));});
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        for sequence in 1..=4 {
            owner.complete_frame().unwrap();
            let a=crate::sensor_contract::Acquisition::new(crate::controls::Settings::default(),[1;32],1,sequence).unwrap();
            owner.publish_frame(crate::Frame{raw:Arc::new(vec![1;1024*768]),metadata:crate::Metadata{acquisition:Some(a),source:"notification fixture".into(),simulated:true,sequence,measurement:None,unix_millis:0,width:1024,height:768,format:"BayerGRBG8".into()}}).unwrap();
        }
        assert!(monitor.take_frame().is_none());
        lease.tick();
        thread.join().unwrap();let frame=monitor.take_frame().unwrap();assert_eq!(frame.metadata.sequence,4);
        assert_eq!(frame.metadata.measurement.as_ref().unwrap().accepted_total,4);assert!(monitor.take_frame().is_none());
        owner.publish_frame(frame.clone()).unwrap();
        monitor.stop();assert!(monitor.take_frame().is_none());
        assert!(owner.complete_frame().is_err());assert!(owner.publish_frame(frame).is_err());
        lease.pipe_lost();assert_eq!(monitor.first_fault(),None);
    }
    #[test]
    fn supervisor_samples_report_clock_only_while_holding_report_serialization() {
        struct CountClock(std::sync::atomic::AtomicUsize);
        impl Clock for CountClock {
            fn now(&self) -> Duration {
                self.0.fetch_add(1, Ordering::AcqRel);
                Duration::ZERO
            }
        }
        let clock = Arc::new(CountClock(std::sync::atomic::AtomicUsize::new(0)));
        let (owner, mut lease, _) = session([1; 32], [2; 32], clock.clone());
        let held = owner.0.reports.lock().unwrap();
        let reads = clock.0.load(Ordering::Acquire);
        let (entered, ready) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            entered.send(()).unwrap();
            lease.tick();
        });
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let while_held = clock.0.load(Ordering::Acquire);
        drop(held);
        thread.join().unwrap();
        assert_eq!(
            while_held, reads,
            "report clock sampled before mailbox serialization; a newer valid publication can appear future-dated"
        );
        assert_eq!(clock.0.load(Ordering::Acquire), reads + 1);
    }
    #[derive(Default)]
    struct FakeClock(AtomicU64);
    impl Clock for FakeClock {
        fn now(&self) -> Duration {
            Duration::from_millis(self.0.load(Ordering::Acquire))
        }
    }
    impl FakeClock {
        fn set(&self, ms: u64) {
            self.0.store(ms, Ordering::Release);
        }
    }
    #[test]
    fn acknowledged_challenge_barriers_and_full_mailbox_fault_latch() {
        for delay in [31, 100, 2001] {
            let clock = Arc::new(FakeClock::default());
            let (_, mut lease, monitor) = session([1; 32], [2; 32], clock.clone());
            let (input, inbox) = ipc_mailbox();
            let (ack, accepted) = std::sync::mpsc::sync_channel(1);
            input
                .try_send(SupervisorCommand::AcceptChallenge([3; 32], ack))
                .unwrap();
            assert!(accepted.try_recv().is_err()); // cannot send wire challenge yet
            clock.set(delay); // supervisor has not run, no scheduler sleep
            lease.command(inbox.0.try_recv().unwrap());
            let accepted = accepted.try_recv().unwrap().unwrap();
            assert_eq!(accepted.accepted_at, Duration::from_millis(delay));
            assert_eq!(accepted.sequence, 1);
            let response = Record {
                kind: Kind::Response,
                session: [1; 32],
                binding: [2; 32],
                sequence: accepted.sequence,
                challenge: accepted.challenge,
            }
            .encode();
            input
                .try_send(SupervisorCommand::Response(response))
                .unwrap();
            lease.command(inbox.0.try_recv().unwrap());
            assert!(monitor.0.authorized.load(Ordering::Acquire));
            assert_eq!(monitor.first_fault(), None);
            // A full command queue cannot lose the adapter's failure.
            input
                .try_send(SupervisorCommand::Challenge([4; 32]))
                .unwrap();
            monitor.pipe_lost();
            assert_eq!(monitor.first_fault(), Some(Fault::Lease));
            assert!(monitor.cancelled());
            lease.command(inbox.0.try_recv().unwrap());
            lease.tick();
            assert_eq!(monitor.first_fault(), Some(Fault::Lease));
        }
    }
    #[test]
    fn acknowledged_challenge_post_acceptance_budget_is_not_shifted() {
        for delay in [31, 100, 2000, 8000] {
            let clock = Arc::new(FakeClock::default());
            let (owner, mut lease, monitor) = session([1; 32], [2; 32], clock.clone());
            let accepted = lease.accept_challenge([3; 32]).unwrap();
            clock.set(delay); // acceptance barrier held; fast wire reply queued
            let response = Record {
                kind: Kind::Response,
                session: [1; 32],
                binding: [2; 32],
                sequence: accepted.sequence,
                challenge: accepted.challenge,
            }
            .encode();
            assert_eq!(lease.response(&response), Ok(delay < 2000));
            assert_eq!(owner.authorized(), delay < 2000);
            assert_eq!(accepted.accepted_at, Duration::ZERO);
            if delay >= 2000 {
                clock.set(8000);
                lease.tick();
                assert_eq!(monitor.first_fault(), Some(Fault::Lease));
                assert!(monitor.cancelled());
            } else {
                assert_eq!(monitor.first_fault(), None);
            }
        }
    }
    #[test]
    fn acknowledged_response_barrier_prevents_next_challenge_queue_collision() {
        for delay in [31, 100, 1001, 2000] {
            let clock = Arc::new(FakeClock::default());
            let (_, mut lease, monitor) = session([1; 32], [2; 32], clock.clone());
            let first = lease.accept_challenge([3; 32]).unwrap();
            let response = Record {
                kind: Kind::Response,
                session: [1; 32],
                binding: [2; 32],
                sequence: first.sequence,
                challenge: first.challenge,
            }
            .encode();
            let (input, inbox) = ipc_mailbox();
            let (ack, consumed) = std::sync::mpsc::sync_channel(1);
            input
                .try_send(SupervisorCommand::AcceptResponse(response, ack))
                .unwrap();
            assert!(consumed.try_recv().is_err()); // adapter cannot issue next challenge
            clock.set(delay);
            lease.command(inbox.0.try_recv().unwrap());
            assert_eq!(consumed.try_recv().unwrap(), Ok(delay < 2000));
            if delay < 2000 {
                let (ack, next) = std::sync::mpsc::sync_channel(1);
                input
                    .try_send(SupervisorCommand::AcceptChallenge([4; 32], ack))
                    .unwrap();
                lease.command(inbox.0.try_recv().unwrap());
                assert_eq!(next.try_recv().unwrap().unwrap().sequence, 2);
                assert_eq!(monitor.first_fault(), None);
            } else {
                clock.set(8000);
                lease.tick();
                assert_eq!(monitor.first_fault(), Some(Fault::Lease));
            }
        }
    }
    fn renew(lease: &mut LeasePort, sequence: u64) {
        let mut challenge = [3; 32];
        challenge[..8].copy_from_slice(&sequence.to_le_bytes());
        lease.challenge(challenge).unwrap();
        assert_eq!(
            lease.response(
                &Record {
                    kind: Kind::Response,
                    session: [1; 32],
                    binding: [2; 32],
                    sequence,
                    challenge
                }
                .encode()
            ),
            Ok(true)
        );
    }
    #[test]
    fn shared_recipe_then_six_hours_live_single_init_exact_pattern_gates() {
        use crate::acquisition_recipe::{Backend, initialize_continuous};
        struct Virtual<'a> {
            clock: &'a FakeClock,
            owner: &'a OwnerPort,
            lease: &'a mut LeasePort,
            sequence: u64,
            gain: u16,
            patterns: Vec<Duration>,
            resets: usize,
        }
        impl Backend for Virtual<'_> {
            fn checkpoint(&mut self) -> Result<(), String> {
                self.owner.checkpoint()
            }
            fn write(&mut self, a: u16, v: u16) -> Result<(), String> {
                self.checkpoint()?;
                if a == 0x0d && v == 1 {
                    self.resets += 1;
                }
                if a == 0x35 {
                    self.gain = v;
                }
                Ok(())
            }
            fn checked(&mut self, a: u16, v: u16) -> Result<(), String> {
                self.write(a, v)
            }
            fn read(&mut self, _: u16) -> Result<u16, String> {
                self.checkpoint()?;
                Ok(self.gain)
            }
            fn bridge(&mut self, _: u16) -> Result<(), String> {
                self.checkpoint()
            }
            fn progress(&mut self, _: crate::progress::Stage) {}
            fn settle(&mut self, d: Duration) -> Result<(), String> {
                let mut left = d.as_millis() as u64;
                while left > 0 {
                    let step = left.min(1000);
                    let now = self.clock.now().as_millis() as u64;
                    self.clock.set(now + step);
                    self.sequence += 1;
                    renew(self.lease, self.sequence);
                    self.lease.tick();
                    self.checkpoint()?;
                    left -= step;
                }
                Ok(())
            }
            fn pattern(&mut self, i: usize, _: &str, d: Duration) -> Result<(), String> {
                assert_eq!(i, self.patterns.len());
                assert_eq!(d, Duration::from_secs(5));
                self.patterns.push(d);
                self.settle(d)
            }
        }
        for mode in [crate::controls::Mode::Full, crate::controls::Mode::Xga] {
            let clock = Arc::new(FakeClock::default());
            let (owner, mut lease, monitor) = session([1; 32], [2; 32], clock.clone());
            renew(&mut lease, 1);
            let mut backend = Virtual {
                clock: &clock,
                owner: &owner,
                lease: &mut lease,
                sequence: 1,
                gain: 8,
                patterns: Vec::new(),
                resets: 0,
            };
            initialize_continuous(
                &mut backend,
                crate::controls::Settings {
                    mode,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(backend.resets, 1);
            assert_eq!(backend.patterns, [Duration::from_secs(5); 3]);
            let mut sequence = backend.sequence;
            drop(backend);
            owner.enable_live().unwrap();
            lease.tick();
            for _ in 0..21600 {
                clock.set(clock.now().as_millis() as u64 + 1000);
                sequence += 1;
                renew(&mut lease, sequence);
                owner.nonempty_transfer().unwrap();
                owner.complete_frame().unwrap();
                lease.tick();
                assert!(!monitor.cancelled());
            }
            monitor.stop();
            assert!(owner.enable_live().is_err());
        }
    }
    #[test]
    fn owner_has_no_initial_lease_and_cannot_resurrect_after_stop() {
        let clock = Arc::new(FakeClock::default());
        let (owner, mut lease, monitor) = session([1; 32], [2; 32], clock);
        assert!(owner.checkpoint().is_err());
        lease.tick();
        assert!(monitor.cancelled());
        assert_eq!(monitor.first_fault(), Some(Fault::Protocol));
        assert!(lease.challenge([3; 32]).is_err());
    }
    #[test]
    fn stop_then_adapter_disconnect_cannot_reclassify_clean_cancellation() {
        let (_, mut lease, monitor) = session([1; 32], [2; 32], Arc::new(FakeClock::default()));
        renew(&mut lease, 1);
        monitor.stop();
        lease.tick();
        lease.pipe_lost();
        assert!(monitor.cancelled());
        assert_eq!(monitor.first_fault(), None);
        let (input, inbox) = ipc_mailbox();
        let thread = SupervisorThread::spawn(lease, inbox).unwrap();
        drop(input);
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(monitor.first_fault(), None);
        drop(thread);
    }
    #[test]
    fn production_mailbox_six_virtual_hours_one_enable_fixed_retention_then_stop() {
        let clock = Arc::new(FakeClock::default());
        let (owner, mut lease, monitor) = session([1; 32], [2; 32], clock.clone());
        renew(&mut lease, 1);
        owner.enable_live().unwrap();
        lease.tick();
        for n in 1..=10800 {
            clock.set(n * 2000);
            renew(&mut lease, n + 1);
            owner.nonempty_transfer().unwrap();
            owner.complete_frame().unwrap();
            lease.tick();
            assert!(!monitor.cancelled());
        }
        assert!(owner.enable_live().is_err()); // no second initialization/enable
        assert_eq!(monitor.first_fault(), Some(Fault::Protocol));
        monitor.stop();
        assert!(owner.nonempty_transfer().is_err());
    }
    #[test]
    fn real_independent_thread_expires_stalled_owner_and_pipe_loss_latches() {
        let clock = Arc::new(FakeClock::default());
        let (owner, mut lease, monitor) = session([1; 32], [2; 32], clock.clone());
        renew(&mut lease, 1);
        owner.enable_live().unwrap();
        lease.tick();
        // The producer performs no calls while the real supervisor runs.
        let (_input, inbox) = ipc_mailbox();
        let thread = SupervisorThread::spawn(lease, inbox).unwrap();
        clock.set(5000);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !monitor.cancelled() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(monitor.first_fault(), Some(Fault::NoTransfer));
        assert!(owner.checkpoint().is_err());
        drop(thread);
        let (_, mut lease, monitor) = session([1; 32], [2; 32], Arc::new(FakeClock::default()));
        lease.pipe_lost();
        assert_eq!(monitor.first_fault(), Some(Fault::Lease));
    }
    #[test]
    fn actual_mailbox_independent_deadlines_boundaries_and_stale_reports() {
        for fragments in [false, true] {
            let clock = Arc::new(FakeClock::default());
            let (owner, mut lease, monitor) = session([1; 32], [2; 32], clock.clone());
            renew(&mut lease, 1);
            owner.enable_live().unwrap();
            owner.complete_frame().unwrap();
            lease.tick();
            clock.set(4000);
            renew(&mut lease, 2);
            if fragments {
                owner.nonempty_transfer().unwrap();
            }
            lease.tick();
            clock.set(4999);
            lease.tick();
            assert!(!monitor.cancelled());
            clock.set(5000);
            lease.tick();
            assert_eq!(
                monitor.first_fault(),
                Some(if fragments {
                    Fault::NoFrame
                } else {
                    Fault::NoTransfer
                })
            );
        }
        let clock = Arc::new(FakeClock::default());
        let (owner, mut lease, monitor) = session([1; 32], [2; 32], clock.clone());
        renew(&mut lease, 1);
        owner.enable_live().unwrap();
        lease.tick();
        owner.nonempty_transfer().unwrap();
        clock.set(101);
        lease.tick();
        assert_eq!(monitor.first_fault(), Some(Fault::Protocol));
    }
}
