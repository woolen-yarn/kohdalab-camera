//! Fixed diagnostic observations. Never ownership or cleanup authority.
use crate::acquisition_telemetry::{Session, Telemetry};
use serde::Serialize;

/// Five slots: three patterns, live, cleanup. Times are recorder-local ns.
/// Zero status is an explicitly absent record (early cancellation is possible).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    words: [[u64; 28]; 5],
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PhaseSummary {
    pub diagnostic_version: u8,
    pub tail_provenance: Option<crate::tail_provenance::TailProvenance>,
    pub counters_saturated: bool,
    pub frames_saturated: bool,
    pub time_saturated: bool,
    pub relative_ns: [Option<u64>; 12],
    pub drain_returned_ok: Option<bool>,
    pub cleanup_returned_ok: Option<bool>,
    pub accepted_frames: u64,
    pub successful_transfers: u64,
    pub nonempty_bytes: u64,
    pub short_completions: u64,
    pub zlp_completions: u64,
    pub boundary_fragment_events: u64,
    pub overflow_events: u64,
    pub transfer_errors: u64,
    pub wait_timeouts: u64,
    pub timestamp_regressions: u64,
    /// 0 unavailable, 1 cancelled, 2 duration complete, 3 error.
    pub acquisition_exit: u64,
    pub completion_gap_count: u64,
    pub completion_gap_cumulative_ns: u64,
    pub completion_gap_max_ns: u64,
    pub completion_gap_saturated: bool,
}
impl Diagnostic {
    pub fn phases(&self) -> [Option<PhaseSummary>; 5] {
        std::array::from_fn(|i| self.phase(i))
    }
    pub fn phase(&self, slot: usize) -> Option<PhaseSummary> {
        let w = self.words.get(slot)?;
        if w[0] & 1 == 0 {
            return None;
        }
        let decode_tri = |n| match n {
            1 => Some(false),
            2 => Some(true),
            _ => None,
        };
        Some(PhaseSummary {
            diagnostic_version: ((w[0] >> 20) & 15) as u8,
            tail_provenance: decode_tail(w),
            counters_saturated: w[0] & 2 != 0,
            frames_saturated: w[0] & 4 != 0,
            time_saturated: w[0] & 8 != 0,
            relative_ns: std::array::from_fn(|i| (w[0] & (1 << (i + 4)) != 0).then_some(w[i + 1])),
            drain_returned_ok: decode_tri((w[0] >> 16) & 3),
            cleanup_returned_ok: decode_tri((w[0] >> 18) & 3),
            accepted_frames: w[13],
            successful_transfers: w[14],
            nonempty_bytes: w[15],
            short_completions: w[16],
            zlp_completions: w[17],
            boundary_fragment_events: w[18],
            overflow_events: w[19],
            transfer_errors: w[20],
            wait_timeouts: w[21],
            timestamp_regressions: w[22],
            acquisition_exit: w[23],
            completion_gap_count: w[24],
            completion_gap_cumulative_ns: w[25],
            completion_gap_max_ns: w[26],
            completion_gap_saturated: w[27] & 1 != 0,
        })
    }
    pub fn capture(session: &Session) -> Self {
        let mut result = Self::default();
        for (slot, record) in result.words.iter_mut().zip(&session.records) {
            if let Some(t) = record {
                *slot = capture(t);
            }
        }
        result
    }
    pub fn accepted_fps(&self, slot: usize) -> Option<f64> {
        let w = self.words.get(slot)?;
        if w[0] & 15 != 1 || w[0] & (1 << 6) == 0 || w[0] & (1 << 7) == 0 || w[22] != 0 {
            return None;
        }
        let ns = w[4].checked_sub(w[3]).filter(|&n| n > 0)?;
        (w[13] > 0).then(|| w[13] as f64 * 1e9 / ns as f64)
    }
}
fn tri(v: Option<bool>) -> u64 {
    match v {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    }
}
fn capture(t: &Telemetry) -> [u64; 28] {
    let mut w = [0; 28];
    w[0] = 1
        | (t.counters_saturated as u64) << 1
        | (t.frame_count_saturated as u64) << 2
        | (t.time_saturated.get() as u64) << 3;
    for (i, time) in t.relative_ns.iter().enumerate() {
        if let Some(ns) = time {
            w[0] |= 1 << (i + 4);
            w[i + 1] = *ns;
        }
    }
    w[0] |= tri(t.drain_returned_ok) << 16 | tri(t.cleanup_returned_ok) << 18;
    w[13..23].copy_from_slice(&[
        t.accepted_frames,
        t.successful_transfers,
        t.nonempty_bytes,
        t.short_completions,
        t.zlp_completions,
        t.discarded_fragment_events,
        t.overflow_events,
        t.transfer_errors,
        t.wait_timeouts,
        t.timestamp_regressions,
    ]);
    // Exit classification is descriptive, not parsed to decide any lifecycle action.
    w[23] = match t.acquisition_exit {
        None => 0,
        Some("cancelled") => 1,
        Some("duration_complete") => 2,
        Some(_) => 3,
    };
    w[24..28].copy_from_slice(&[
        t.completion_gap.count,
        t.completion_gap.cumulative_ns,
        t.completion_gap.max_ns,
        t.completion_gap.saturated as u64,
    ]);
    w[0] |= 2 << 20;
    if let Some(t) = t.tail_provenance.filter(|t| t.valid()) {
        w[0] |= u64::from(t.initial_sync) << 24 | u64::from(t.intervening_boundary) << 40;
        w[27] |= u64::from(t.intervening_overflow) << 1 | u64::from(t.tail_bytes) << 17
            | u64::from(t.tail_state) << 39 | u64::from(t.saturated) << 42 | 1 << 43;
    }
    w
}

fn decode_tail(w: &[u64;28]) -> Option<crate::tail_provenance::TailProvenance> {
    if (w[0] >> 20) & 15 != 2 || w[27] & (1 << 43) == 0 { return None; }
    let t = crate::tail_provenance::TailProvenance {
        initial_sync: (w[0] >> 24) as u16, intervening_boundary: (w[0] >> 40) as u16,
        intervening_overflow: (w[27] >> 1) as u16, tail_bytes: ((w[27] >> 17) & 0x3fffff) as u32,
        tail_state: ((w[27] >> 39) & 7) as u8, saturated: w[27] & (1 << 42) != 0,
    };
    let aggregate = u64::from(t.initial_sync) + u64::from(t.intervening_boundary)
        + u64::from(t.intervening_overflow) + u64::from(t.tail_bytes != 0);
    (t.valid() && (t.saturated || (aggregate == w[18] && u64::from(t.intervening_overflow) == w[19]))
        && (t.tail_state != 2 || w[13] > 0)).then_some(t)
}

fn valid_words(w: &[u64;28]) -> bool {
    match (w[0] >> 20) & 15 {
        0 => w[0] >> 20 == 0 && w[27] <= 1,
        2 => w[0] >> 56 == 0 && w[27] >> 44 == 0 &&
            if w[27] & (1 << 43) != 0 { decode_tail(w).is_some() }
            else { w[0] >> 24 == 0 && w[27] <= 1 },
        _ => false,
    }
}

/// Strict ordered, one-shot decoder. Binding is checked by the enclosing pipe
/// parser before push. Invalid observations do not change terminal proof.
#[derive(Default)]
pub struct Decoder {
    next: usize,
    value: Diagnostic,
    pub invalid: bool,
}
impl Decoder {
    pub fn push(&mut self, payload: &[u8]) -> Result<(), &'static str> {
        let result = self.push_inner(payload);
        if result.is_err() {
            self.invalid = true;
        }
        result
    }
    fn push_inner(&mut self, p: &[u8]) -> Result<(), &'static str> {
        if self.invalid
            || p.len() != 60
            || self.next >= 20
            || p[58..] != [0, 0]
            || p[0] as usize != self.next / 4
            || p[1] as usize != self.next % 4
        {
            return Err("diagnostic order/size");
        }
        let slot = self.next / 4;
        let chunk = self.next % 4;
        for i in 0..7 {
            self.value.words[slot][chunk * 7 + i] =
                u64::from_le_bytes(p[2 + i * 8..10 + i * 8].try_into().unwrap());
        }
        self.next += 1;
        if chunk == 3 {
            let w = &self.value.words[slot];
            if !valid_words(w)
                || (w[0] >> 16) & 3 == 3
                || (w[0] >> 18) & 3 == 3
                || w[23] > 3
                || (w[0] & 1 == 0 && w.iter().any(|&n| n != 0))
            {
                return Err("diagnostic range");
            }
            for i in 0..12 {
                if w[0] & (1 << (i + 4)) == 0 && w[i + 1] != 0 {
                    return Err("absent time");
                }
            }
        }
        Ok(())
    }
    pub fn finish(&self) -> Option<Diagnostic> {
        (!self.invalid && self.next == 20).then_some(self.value)
    }
}
pub fn payload(d: &Diagnostic, index: usize) -> [u8; 60] {
    assert!(index < 20);
    let mut p = [0; 60];
    p[0] = (index / 4) as u8;
    p[1] = (index % 4) as u8;
    for i in 0..7 {
        p[2 + i * 8..10 + i * 8]
            .copy_from_slice(&d.words[index / 4][index % 4 * 7 + i].to_le_bytes());
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn v020_packed_provenance_legacy_unknown_and_bounded_roundtrip() {
        use crate::acquisition_telemetry::{Binding,Telemetry,Session};
        let mut t=Telemetry::new(Binding::new(Default::default(),false));
        t.tail_provenance=Some(crate::tail_provenance::TailProvenance {
            initial_sync:1,intervening_boundary:2,intervening_overflow:3,
            tail_bytes:3145728,tail_state:2,saturated:false,
        });
        t.discarded_fragment_events=7;
        t.overflow_events=3;
        t.accepted_frames=1;
        let mut session=Session::default();session.records[3]=Some(t);
        let d=Diagnostic::capture(&session);let mut decoder=Decoder::default();
        for i in 0..20 { decoder.push(&payload(&d,i)).unwrap(); }
        let phase=decoder.finish().unwrap().phase(3).unwrap();
        assert_eq!(phase.boundary_fragment_events,7);
        assert_eq!(phase.tail_provenance.unwrap().tail_state,2);
        assert_eq!(std::mem::size_of::<Diagnostic>(),1120);
        assert_eq!((0..20).map(|i|payload(&d,i).len()).sum::<usize>(),1200);
        for (word,bits) in [(0,1u64<<56),(27,1<<44),(27,7<<39)] {
            let mut bad=d;bad.words[3][word]|=bits;
            let mut decoder=Decoder::default();
            assert!((0..20).any(|i|decoder.push(&payload(&bad,i)).is_err()));
            assert!(decoder.finish().is_none());
        }
        let mut legacy=d; legacy.words[3][0]&=(1<<20)-1;legacy.words[3][27]&=1;
        assert!(legacy.phase(3).unwrap().tail_provenance.is_none());
        assert!(valid_words(&legacy.words[3]));
        let mut unknown=d;unknown.words[3][0]^=1<<20;
        assert!(!valid_words(&unknown.words[3]));
    }
    #[test]
    fn fixed_roundtrip_order_duplicates_truncation_ranges_and_partial() {
        let d = Diagnostic::default();
        let mut decoder = Decoder::default();
        for i in 0..20 {
            decoder.push(&payload(&d, i)).unwrap();
            if i < 19 {
                assert!(decoder.finish().is_none());
            }
        }
        assert_eq!(decoder.finish(), Some(d));
        assert!(decoder.push(&payload(&d, 19)).is_err());
        assert!(decoder.finish().is_none());
        for size in 0..60 {
            assert!(Decoder::default().push(&[0; 60][..size]).is_err());
        }
        let mut decoder = Decoder::default();
        assert!(decoder.push(&payload(&d, 1)).is_err());
        for (word, value) in [(0, 1 << 20), (0, 3 << 16), (23, 4), (27, 2), (1, 1)] {
            let mut d = d;
            d.words[0][word] = value;
            let mut decoder = Decoder::default();
            assert!((0..4).any(|i| decoder.push(&payload(&d, i)).is_err()));
        }
    }
    #[test]
    fn saturation_regression_and_absent_time_invalidate_fps() {
        let mut d = Diagnostic::default();
        d.words[3][0] = 1 | (1 << 6) | (1 << 7);
        d.words[3][4] = 1_000_000_000;
        d.words[3][13] = 40;
        assert_eq!(d.accepted_fps(3), Some(40.));
        for bit in [1, 2, 3] {
            let mut x = d;
            x.words[3][0] |= 1 << bit;
            assert_eq!(x.accepted_fps(3), None);
        }
        d.words[3][22] = 1;
        assert_eq!(d.accepted_fps(3), None);
    }
}
