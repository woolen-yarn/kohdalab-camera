//! Best-effort, fixed-size presentation protocol. Never certifies hardware checks.
use std::{
    path::Path,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Stage {
    Identity = 1,
    Configuration,
    Pattern155,
    Pattern123,
    PatternZero,
    Controls,
    Live,
    Cleanup,
}
impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Identity => "Identity / bridge preparation",
            Self::Configuration => "Configuration / readbacks",
            Self::Pattern155 => "Pattern 1 of 3 (5-second acquisition)",
            Self::Pattern123 => "Pattern 2 of 3 (5-second acquisition)",
            Self::PatternZero => "Pattern 3 of 3 (5-second acquisition)",
            Self::Controls => "Requested controls / readbacks",
            Self::Live => "Live trial (30-second acquisition limit)",
            Self::Cleanup => "Cleanup in progress; completion unproved",
        }
    }
    pub fn record(self) -> [u8; 8] {
        [b'C', b'L', b'P', 1, self as u8, 0, 0, 0]
    }
}
pub fn parse(bytes: &[u8]) -> Option<Stage> {
    if bytes.len() != 8 || bytes[..4] != [b'C', b'L', b'P', 1] || bytes[5..] != [0, 0, 0] {
        return None;
    }
    Some(match bytes[4] {
        1 => Stage::Identity,
        2 => Stage::Configuration,
        3 => Stage::Pattern155,
        4 => Stage::Pattern123,
        5 => Stage::PatternZero,
        6 => Stage::Controls,
        7 => Stage::Live,
        8 => Stage::Cleanup,
        _ => return None,
    })
}
pub fn read(path: &Path) -> Option<Stage> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::with_capacity(9);
    file.by_ref().take(9).read_to_end(&mut bytes).ok()?;
    parse(&bytes)
}
pub fn publish(stage: Stage) {
    // Optional channel: failures must never change acquisition or cleanup behavior.
    if let Some(path) = std::env::var_os("CAMERA_LAB_PROGRESS") {
        let _ = std::fs::write(path, stage.record());
    }
}
/// Receipt timing belongs to the displayed frame, independent of its wall metadata.
pub struct FrameTiming {
    pub captured_unix_millis: u128,
    received: Instant,
}
impl FrameTiming {
    pub fn received_at(frame: &crate::Frame, now: Instant) -> Self {
        Self {
            captured_unix_millis: frame.metadata.unix_millis,
            received: now,
        }
    }
    pub fn age_at(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.received)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_protocol_bounds_and_unknown_records() {
        for stage in [
            Stage::Identity,
            Stage::Configuration,
            Stage::Pattern155,
            Stage::Pattern123,
            Stage::PatternZero,
            Stage::Controls,
            Stage::Live,
            Stage::Cleanup,
        ] {
            let b = stage.record();
            assert_eq!(parse(&b), Some(stage));
            for n in 0..8 {
                assert_eq!(parse(&b[..n]), None);
            }
            let mut extra = b.to_vec();
            extra.push(0);
            assert_eq!(parse(&extra), None);
        }
        for index in [0, 1, 2, 3, 5, 6, 7] {
            let mut b = Stage::Live.record();
            b[index] = 255;
            assert_eq!(parse(&b), None);
        }
        for value in [0, 9, 255] {
            let mut b = Stage::Live.record();
            b[4] = value;
            assert_eq!(parse(&b), None);
        }
    }
    #[test]
    fn missing_partial_oversized_and_replaced_progress_are_best_effort() {
        let path = std::env::temp_dir().join(format!("progress-fixture-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert_eq!(read(&path), None);
        for bytes in [
            vec![],
            vec![0; 7],
            vec![0; 100_000],
            Stage::Controls.record().to_vec(),
        ] {
            std::fs::write(&path, &bytes).unwrap();
            assert_eq!(read(&path), parse(&bytes));
        }
        std::fs::write(&path, Stage::Cleanup.record()).unwrap();
        assert_eq!(read(&path), Some(Stage::Cleanup));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn retained_freshness_survives_pause_end_and_wall_clock_changes() {
        use crate::CameraAdapter;
        let mut frame = crate::Simulation { sequence: 0 }.next_frame().unwrap();
        frame.metadata.unix_millis = 5_000;
        let clock = Instant::now();
        let retained = FrameTiming::received_at(&frame, clock);
        // Pause / end: no replacement frame, so receipt age continues to increase.
        assert_eq!(
            retained.age_at(clock + Duration::from_secs(60)),
            Duration::from_secs(60)
        );
        assert_eq!(retained.captured_unix_millis, 5_000);
        // A new frame can have an earlier wall timestamp; receipt freshness still resets.
        frame.metadata.unix_millis = 1_000;
        let replacement = FrameTiming::received_at(&frame, clock + Duration::from_secs(61));
        assert_eq!(
            replacement.age_at(clock + Duration::from_secs(63)),
            Duration::from_secs(2)
        );
        assert_eq!(replacement.captured_unix_millis, 1_000);
        assert_eq!(
            retained.age_at(clock + Duration::from_secs(63)),
            Duration::from_secs(63)
        );
        assert_eq!(retained.captured_unix_millis, 5_000); // old capture metadata remains owned
    }
}
