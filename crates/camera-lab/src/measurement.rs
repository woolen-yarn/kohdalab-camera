//! Optional observations only: no acquisition, lease or cleanup authority.
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
const WINDOW: Duration = Duration::from_secs(2);
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    #[serde(skip)]
    pub received_at: Option<Instant>,
    pub version: u8,
    pub sequence: u64,
    pub generation: u64,
    /// UTC at host acceptance of a complete frame, never sensor exposure time.
    pub host_accepted_completion_unix_ms: Option<u64>,
    pub accepted_total: u64,
    pub source_sample_ns: u64,
    pub window_frames: u32,
    pub window_ns: u64,
}
impl Observation {
    pub fn valid_for(&self, m: &crate::Metadata) -> bool {
        self.version == 1
            && self.sequence == m.sequence
            && self.accepted_total == self.sequence
            && self.window_frames <= 256
            && self.window_ns == 2_000_000_000
            && m.acquisition.as_ref().is_some_and(|a| {
                a.sequence == self.sequence && a.settings_generation == self.generation
            })
    }
    pub fn fps(&self) -> f64 {
        f64::from(self.window_frames) * 1e9 / self.window_ns as f64
    }
}
#[derive(Default)]
pub struct Rolling {
    times: VecDeque<Duration>,
    previous: Option<Duration>,
    overflow_until: Option<Duration>,
}
impl Rolling {
    pub fn add(&mut self, now: Duration) {
        self.prune(now);
        if self.times.len() == 256 {
            self.times.pop_front();
            self.overflow_until = now.checked_add(WINDOW);
        }
        self.times.push_back(now);
    }
    pub fn prune(&mut self, now: Duration) {
        if self.previous.is_some_and(|p| now < p) {
            self.times.clear();
            self.overflow_until = now.checked_add(WINDOW);
        }
        self.previous = Some(now);
        while self
            .times
            .front()
            .is_some_and(|t| now.saturating_sub(*t) >= WINDOW)
        {
            self.times.pop_front();
        }
    }
    pub fn count(&mut self, now: Duration) -> Option<u32> {
        self.prune(now);
        if self.overflow_until.is_some_and(|t| now < t) {
            None
        } else {
            Some(self.times.len() as u32)
        }
    }
}
pub struct Source {
    origin: Instant,
    rolling: Rolling,
    total: u64,
    last: Option<Observation>,
}
impl Default for Source {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            rolling: Rolling::default(),
            total: 0,
            last: None,
        }
    }
}
impl Source {
    pub fn complete(&mut self) {
        let now = self.origin.elapsed();
        let Some(total) = self.total.checked_add(1) else {
            self.last = None;
            return;
        };
        self.total = total;
        self.rolling.add(now);
        self.last = self.rolling.count(now).and_then(|window_frames| {
            Some(Observation {
                received_at: None,
                version: 1,
                sequence: total,
                generation: 0,
                host_accepted_completion_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .and_then(|d| u64::try_from(d.as_millis()).ok())
                    .filter(|n| *n != 0),
                accepted_total: total,
                source_sample_ns: u64::try_from(now.as_nanos()).ok()?,
                window_frames,
                window_ns: 2_000_000_000,
            })
        });
    }
    pub fn bind(&self, frame: &mut crate::Frame) {
        let Some(a) = &frame.metadata.acquisition else {
            return;
        };
        if let Some(mut o) = self
            .last
            .clone()
            .filter(|o| o.sequence == frame.metadata.sequence)
        {
            o.generation = a.settings_generation;
            o.received_at = Some(Instant::now());
            frame.metadata.unix_millis =
                u128::from(o.host_accepted_completion_unix_ms.unwrap_or(0));
            frame.metadata.measurement = Some(o);
        }
    }
}
#[derive(Default)]
pub struct Live {
    origin: Option<Instant>,
    identity: Option<([u8; 32], u64, u64)>,
    last_source: Option<(Observation, Instant, bool)>,
    preview: Rolling,
    pub unique_total: u64,
    last_preview: Option<Instant>,
}
impl Live {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn receive(&mut self, f: &crate::Frame, now: Instant) {
        let o = f.metadata.measurement.as_ref().filter(|o| {
            o.valid_for(&f.metadata)
                && o.received_at
                    .is_some_and(|t| now.checked_duration_since(t).is_some())
        });
        if let Some(o) = o {
            let advancing = self.last_source.as_ref().is_none_or(|(p, _, _)| {
                o.sequence > p.sequence && o.source_sample_ns >= p.source_sample_ns
            });
            if advancing {
                self.last_source = Some((
                    o.clone(),
                    o.received_at.unwrap_or(now),
                    f.metadata.simulated,
                ));
            } else {
                self.last_source = None;
            }
        } else {
            self.last_source = None;
        }
    }
    pub fn submit(&mut self, f: &crate::Frame, now: Instant) {
        let Some(a) = &f.metadata.acquisition else {
            return;
        };
        let id = (a.session, a.settings_generation, f.metadata.sequence);
        if self
            .identity
            .is_some_and(|p| p.0 == id.0 && p.1 == id.1 && p.2 >= id.2)
        {
            return;
        }
        if self.identity.is_some_and(|p| p.0 != id.0 || p.1 != id.1) {
            self.preview = Rolling::default();
            self.origin = Some(now);
        }
        self.identity = Some(id);
        let origin = *self.origin.get_or_insert(now);
        self.preview.add(now.saturating_duration_since(origin));
        self.unique_total += 1;
        self.last_preview = Some(now);
    }
    pub fn preview(&mut self, now: Instant) -> (Option<f64>, Option<Duration>) {
        (
            self.origin
                .and_then(|o| self.preview.count(now.saturating_duration_since(o)))
                .map(|n| f64::from(n) / 2.),
            self.last_preview.map(|t| now.saturating_duration_since(t)),
        )
    }
    pub fn capture(&self, now: Instant) -> (Option<f64>, Option<Duration>, bool) {
        match &self.last_source {
            Some((o, t, sim)) => {
                let age = now.saturating_duration_since(*t);
                (
                    if age < WINDOW { Some(o.fps()) } else { None },
                    Some(age),
                    *sim,
                )
            }
            None => (None, None, false),
        }
    }
}
pub fn timestamp_semantics(m: &crate::Metadata) -> &'static str {
    if m.measurement.as_ref().is_some_and(|o| {
        o.valid_for(m)
            && o.host_accepted_completion_unix_ms
                .is_some_and(|t| u128::from(t) == m.unix_millis)
    }) {
        if m.simulated {
            "SIMULATED source host accepted-frame completion UTC; not sensor exposure time"
        } else {
            "HOST accepted-frame completion UTC; not sensor exposure time"
        }
    } else {
        "Host accepted-frame completion UTC unavailable; legacy unix_millis has unspecified semantics; sensor exposure time unavailable"
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rolling_decay_overflow_and_regression() {
        let mut r = Rolling::default();
        r.add(Duration::ZERO);
        assert_eq!(r.count(Duration::from_secs(1)), Some(1));
        assert_eq!(r.count(WINDOW), Some(0));
        for _ in 0..257 {
            r.add(WINDOW);
        }
        assert_eq!(r.count(WINDOW), None);
        assert_eq!(r.count(WINDOW * 2), Some(0));
        assert_eq!(r.count(Duration::ZERO), None);
    }
}

/// Fixed 60-byte optional record payload. Mandatory frame records stay unchanged.
pub fn encode(o: &Observation) -> [u8; 60] {
    let mut p = [0; 60];
    p[0] = o.version;
    for (offset, value) in [
        (4, o.sequence),
        (12, o.generation),
        (20, o.host_accepted_completion_unix_ms.unwrap_or(0)),
        (28, o.accepted_total),
        (36, o.source_sample_ns),
        (48, o.window_ns),
    ] {
        p[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    p[44..48].copy_from_slice(&o.window_frames.to_le_bytes());
    p
}
pub fn decode(p: &[u8; 60]) -> Option<Observation> {
    if p[0] != 1 || p[1..4].iter().chain(p[56..].iter()).any(|v| *v != 0) {
        return None;
    }
    let n = |i| u64::from_le_bytes(p[i..i + 8].try_into().unwrap());
    let utc = n(20);
    let o = Observation {
        received_at: None,
        version: 1,
        sequence: n(4),
        generation: n(12),
        host_accepted_completion_unix_ms: (utc != 0).then_some(utc),
        accepted_total: n(28),
        source_sample_ns: n(36),
        window_frames: u32::from_le_bytes(p[44..48].try_into().unwrap()),
        window_ns: n(48),
    };
    (o.sequence > 0
        && o.generation > 0
        && o.accepted_total == o.sequence
        && o.window_ns == 2_000_000_000
        && o.window_frames <= 256)
        .then_some(o)
}

#[cfg(test)]
mod boundaries {
    use super::*;
    fn frame() -> crate::Frame {
        crate::Frame {
            raw: std::sync::Arc::new(vec![1; 1024 * 768]),
            metadata: crate::Metadata {
                acquisition: Some(
                    crate::sensor_contract::Acquisition::new(
                        crate::controls::Settings::default(),
                        [7; 32],
                        1,
                        1,
                    )
                    .unwrap(),
                ),
                source: "test".into(),
                simulated: false,
                sequence: 1,
                measurement: None,
                unix_millis: 0,
                width: 1024,
                height: 768,
                format: "BayerGRBG8".into(),
            },
        }
    }
    #[test]
    fn source_wire_frame_binding_and_optional_failures() {
        let mut source = Source::default();
        source.complete();
        let mut f = frame();
        source.bind(&mut f);
        let o = f.metadata.measurement.as_ref().unwrap();
        assert!(o.valid_for(&f.metadata));
        assert_eq!(o.accepted_total, 1);
        assert_eq!(o.window_frames, 1);
        let bytes = encode(o);
        let decoded = decode(&bytes).unwrap();
        let mut wire = o.clone();
        wire.received_at = None;
        assert_eq!(decoded, wire);
        for i in [0, 1, 2, 3, 56, 57, 58, 59] {
            let mut bad = bytes;
            bad[i] = 255;
            assert!(decode(&bad).is_none());
        }
        let mut bad = f.clone();
        bad.metadata.sequence += 1;
        assert!(!o.valid_for(&bad.metadata));
        bad = f.clone();
        bad.metadata
            .acquisition
            .as_mut()
            .unwrap()
            .settings_generation += 1;
        assert!(!o.valid_for(&bad.metadata));
        let mut missing_clock = o.clone();
        missing_clock.host_accepted_completion_unix_ms = None;
        assert!(missing_clock.valid_for(&f.metadata));
        assert_eq!(
            decode(&encode(&missing_clock))
                .unwrap()
                .host_accepted_completion_unix_ms,
            None
        );
        source.complete();
        let mut wrong = frame();
        source.bind(&mut wrong);
        assert!(wrong.metadata.measurement.is_none());
    }
    #[test]
    fn unique_preview_recipe_repaint_missing_stale_and_reset() {
        let now = Instant::now();
        let mut f = frame();
        let mut source = Source::default();
        source.complete();
        source.bind(&mut f);
        f.metadata.measurement.as_mut().unwrap().received_at = Some(now);
        let mut live = Live::default();
        assert_eq!(live.capture(now), (None, None, false));
        live.receive(&f, now);
        assert_eq!(live.capture(now).0, Some(0.5));
        live.submit(&f, now);
        live.submit(&f, now + Duration::from_millis(20));
        assert_eq!(live.unique_total, 1);
        assert_eq!(live.preview(now).0, Some(0.5));
        assert_eq!(live.preview(now + WINDOW).0, Some(0.));
        assert_eq!(live.capture(now + WINDOW).0, None);
        assert_eq!(live.capture(now + WINDOW).1, Some(WINDOW));
        live.reset();
        assert_eq!(live.capture(now).0, None);
        assert_eq!(live.preview(now).0, None);
        assert_eq!(live.unique_total, 0);
        f.metadata.simulated = true;
        live.receive(&f, now);
        assert!(live.capture(now).2);
        f.metadata.measurement = None;
        live.receive(&f, now);
        assert_eq!(live.capture(now).0, None);
        source.bind(&mut f);
        f.metadata.measurement.as_mut().unwrap().received_at = None;
        live.receive(&f, now);
        assert_eq!(live.capture(now).0, None);
        source.bind(&mut f);
        f.metadata.measurement.as_mut().unwrap().received_at = Some(now + WINDOW);
        live.receive(&f, now);
        assert_eq!(live.capture(now).0, None);
        // A backlogged receipt cannot become fresh just because GUI finally consumes it.
        source.bind(&mut f);
        f.metadata.measurement.as_mut().unwrap().received_at = Some(now);
        live.receive(&f, now + WINDOW);
        assert_eq!(live.capture(now + WINDOW).0, None);
    }
    #[test]
    fn immutable_frozen_timing_and_clock_unavailable_semantics() {
        let mut source = Source::default();
        source.complete();
        let mut f = frame();
        source.bind(&mut f);
        let held = f.clone();
        source.complete();
        f.metadata.sequence = 2;
        f.metadata.acquisition.as_mut().unwrap().sequence = 2;
        source.bind(&mut f);
        assert_eq!(held.metadata.measurement.as_ref().unwrap().sequence, 1);
        assert_eq!(f.metadata.measurement.as_ref().unwrap().sequence, 2);
        assert!(timestamp_semantics(&held.metadata).starts_with("HOST accepted"));
        f.metadata
            .measurement
            .as_mut()
            .unwrap()
            .host_accepted_completion_unix_ms = None;
        f.metadata.unix_millis = 0;
        assert!(timestamp_semantics(&f.metadata).contains("unavailable"));
        let mut legacy = held.metadata.clone();
        legacy.measurement = None;
        assert!(timestamp_semantics(&legacy).contains("unspecified"));
    }
}
