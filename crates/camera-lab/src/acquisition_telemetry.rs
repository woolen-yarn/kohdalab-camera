//! Observations only: never a lease, guard, ownership or cleanup authority.
//! All timestamps are nanoseconds relative to this recorder's local monotonic origin.
use serde::Serialize;
use std::{cell::Cell, io::Write, path::Path, time::Instant};

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Summary {
    pub saturated: bool,
    pub count: u64,
    pub cumulative_ns: u64,
    pub max_ns: u64,
    /// Upper inclusive bounds: 1us, 10us, 100us, 1ms, 10ms, 100ms, 1s, infinity.
    pub histogram: [u64; 8],
}
impl Summary {
    pub fn observe(&mut self, ns: u64) {
        add(&mut self.count, 1, &mut self.saturated);
        add(&mut self.cumulative_ns, ns, &mut self.saturated);
        self.max_ns = self.max_ns.max(ns);
        let i = [
            1_000,
            10_000,
            100_000,
            1_000_000,
            10_000_000,
            100_000_000,
            1_000_000_000,
        ]
        .iter()
        .position(|&limit| ns <= limit)
        .unwrap_or(7);
        add(&mut self.histogram[i], 1, &mut self.saturated);
    }
}
#[derive(Clone, Copy, Debug, Serialize)]
pub enum Event {
    EnableStart,
    EnableAck,
    AcquisitionStart,
    AcquisitionEnd,
    FirstFrame,
    LastFrame,
    CancelStart,
    CancelEnd,
    DrainStart,
    DrainEnd,
    CleanupStart,
    CleanupEnd,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Binding {
    pub geometry_contract_version: u32,
    pub source_window: [u16;4],
    pub sensor_skip: [u8;2],
    pub profile: &'static str,
    pub width: u16,
    pub height: u16,
    pub clock_register: Option<u16>,
    pub exposure_rows: Option<u16>,
    pub gain_eighths: Option<u16>,
    pub phase: &'static str,
    pub exposure_seconds: Option<f64>,
    pub exposure_note: &'static str,
}
impl Binding {
    pub fn new(settings: crate::controls::Settings, pattern: bool) -> Self {
        let p = settings.mode.profile();
        Self {
            geometry_contract_version: 1,
            source_window: [32,20,2048,1536],
            sensor_skip: if settings.mode == crate::controls::Mode::FullFieldSkip {[2,2]} else {[1,1]},
            profile: p.id,
            width: p.width,
            height: p.height,
            clock_register: Some(p.clock),
            exposure_rows: Some(if pattern { 100 } else { settings.exposure_rows }),
            gain_eighths: Some(if pattern { 8 } else { settings.gain_eighths }),
            phase: if pattern { "pattern" } else { "live" },
            exposure_seconds: None,
            exposure_note: "Requires a measured clock; no clock or board inference",
        }
    }
}
#[derive(Debug, Serialize)]
pub struct Telemetry {
    pub schema: u32,
    pub tail_provenance: Option<crate::tail_provenance::TailProvenance>,
    pub counters_saturated: bool,
    pub frame_count_saturated: bool,
    pub time_saturated: Cell<bool>,
    pub acquisition_exit: Option<&'static str>,
    pub drain_returned_ok: Option<bool>,
    pub cleanup_returned_ok: Option<bool>,
    pub binding: Binding,
    /// Fixed slots indexed by Event declaration order; absent means unobserved.
    pub relative_ns: [Option<u64>; 12],
    pub timestamp_regressions: u64,
    pub accepted_frames: u64,
    pub successful_transfers: u64,
    pub nonempty_transfers: u64,
    pub nonempty_bytes: u64,
    pub short_completions: u64,
    pub zlp_completions: u64,
    pub discarded_fragment_events: u64,
    pub overflow_events: u64,
    pub transfer_errors: u64,
    pub wait_timeouts: u64,
    pub wait: Summary,
    pub completion_gap: Summary,
    pub parse_and_assertions: Summary,
    pub diagnostic_copy: Summary,
    pub preview_publication: Summary,
    #[serde(skip)]
    last_timestamp: u64,
    #[serde(skip)]
    last_completion: Option<u64>,
}
#[derive(Debug, Serialize)]
pub struct Rate {
    pub inputs_saturated: bool,
    pub numerator_frames: u64,
    pub denominator_ns: Option<u64>,
    pub frames_per_second: Option<f64>,
}
impl Telemetry {
    pub fn new(binding: Binding) -> Self {
        Self {
            schema: 3,
            tail_provenance: None,
            counters_saturated: false,
            frame_count_saturated: false,
            time_saturated: Cell::new(false),
            acquisition_exit: None,
            drain_returned_ok: None,
            cleanup_returned_ok: None,
            binding,
            relative_ns: [None; 12],
            timestamp_regressions: 0,
            accepted_frames: 0,
            successful_transfers: 0,
            nonempty_transfers: 0,
            nonempty_bytes: 0,
            short_completions: 0,
            zlp_completions: 0,
            discarded_fragment_events: 0,
            overflow_events: 0,
            transfer_errors: 0,
            wait_timeouts: 0,
            wait: Summary::default(),
            completion_gap: Summary::default(),
            parse_and_assertions: Summary::default(),
            diagnostic_copy: Summary::default(),
            preview_publication: Summary::default(),
            last_timestamp: 0,
            last_completion: None,
        }
    }
    pub fn increment_wait_timeouts(&mut self) {
        add(&mut self.wait_timeouts, 1, &mut self.counters_saturated);
    }
    pub fn increment_transfer_errors(&mut self) {
        add(&mut self.transfer_errors, 1, &mut self.counters_saturated);
    }
    pub fn increment_overflow_events(&mut self) {
        add(&mut self.overflow_events, 1, &mut self.counters_saturated);
    }
    fn accept_time(&mut self, ns: u64) -> bool {
        if ns < self.last_timestamp {
            add(
                &mut self.timestamp_regressions,
                1,
                &mut self.counters_saturated,
            );
            return false;
        }
        self.last_timestamp = ns;
        true
    }
    pub fn event(&mut self, event: Event, ns: u64) {
        if self.accept_time(ns) {
            let slot = &mut self.relative_ns[event as usize];
            if slot.is_none() || matches!(event, Event::LastFrame) {
                *slot = Some(ns);
            }
        }
    }
    pub fn frame(&mut self, ns: u64) {
        add(
            &mut self.accepted_frames,
            1,
            &mut self.frame_count_saturated,
        );
        self.event(Event::FirstFrame, ns);
        self.event(Event::LastFrame, ns);
    }
    pub fn completion(&mut self, ns: u64, bytes: usize) {
        if self.accept_time(ns) {
            if let Some(previous) = self.last_completion {
                self.completion_gap.observe(ns - previous);
            }
            self.last_completion = Some(ns);
        }
        add(
            &mut self.successful_transfers,
            1,
            &mut self.counters_saturated,
        );
        add(
            &mut self.nonempty_bytes,
            bytes as u64,
            &mut self.counters_saturated,
        );
        if bytes > 0 {
            add(
                &mut self.nonempty_transfers,
                1,
                &mut self.counters_saturated,
            );
        }
        if bytes < 16384 {
            add(&mut self.short_completions, 1, &mut self.counters_saturated);
        }
        if bytes == 0 {
            add(&mut self.zlp_completions, 1, &mut self.counters_saturated);
        }
    }
    pub fn rate(&self, inter_frame: bool) -> Rate {
        let (a, b, n) = if inter_frame {
            (
                Event::FirstFrame,
                Event::LastFrame,
                self.accepted_frames.saturating_sub(1),
            )
        } else {
            (
                Event::AcquisitionStart,
                Event::AcquisitionEnd,
                self.accepted_frames,
            )
        };
        let denominator_ns = self.relative_ns[a as usize]
            .zip(self.relative_ns[b as usize])
            .and_then(|(a, b)| b.checked_sub(a))
            .filter(|&d| d > 0);
        let inputs_saturated = self.frame_count_saturated || self.time_saturated.get();
        Rate {
            inputs_saturated,
            numerator_frames: n,
            denominator_ns,
            frames_per_second: denominator_ns
                .filter(|_| n > 0 && self.timestamp_regressions == 0 && !inputs_saturated)
                .map(|d| n as f64 * 1e9 / d as f64),
        }
    }
}
/// Fixed storage: three patterns, Live, and trial cleanup.
#[derive(Default)]
pub struct Session {
    pub records: [Option<Telemetry>; 5],
}
impl Session {
    /// Optional observations only, after cleanup attempts and handle release.
    pub fn persist(&self, root: &Path, diagnostic: &mut impl Write) -> [bool; 5] {
        let mut failed = [false; 5];
        for (i, (record, directory)) in self
            .records
            .iter()
            .zip(["pattern-155", "pattern-123", "pattern-zero", "live", ""])
            .enumerate()
        {
            if let Some(data) = record {
                let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    let record = serde_json::json!({"telemetry": data,
                        "accepted_frame_acquisition_rate": data.rate(false),
                        "first_to_last_inter_frame_rate": data.rate(true),
                        "events": ["enable_start", "enable_ack", "acquisition_start", "acquisition_end", "first_frame", "last_frame", "cancel_start", "cancel_end", "drain_start", "drain_end", "cleanup_start", "cleanup_end"],
                        "scope": "Local monotonic observations only; never cleanup, ownership or lease proof. Drain completions excluded."});
                    std::fs::write(
                        root.join(directory).join("acquisition-telemetry.json"),
                        serde_json::to_vec_pretty(&record)?,
                    )?;
                    Ok(())
                })();
                if let Err(error) = result {
                    failed[i] = true;
                    let _ = writeln!(
                        diagnostic,
                        "acquisition telemetry persistence failed: {error}"
                    );
                }
            }
        }
        failed
    }
}
/// Production boundary shared with no-USB fixtures.
pub fn release_and_persist<T, I, D>(
    result: T,
    interface: I,
    device: D,
    session: &Session,
    root: Option<&Path>,
    diagnostic: &mut impl Write,
) -> (T, [bool; 5]) {
    drop(interface);
    drop(device);
    let failed = root
        .map(|root| session.persist(root, diagnostic))
        .unwrap_or([false; 5]);
    (result, failed)
}
fn add(value: &mut u64, amount: u64, saturated: &mut bool) {
    let (sum, overflow) = value.overflowing_add(amount);
    *value = if overflow { u64::MAX } else { sum };
    *saturated |= overflow;
}
pub(crate) struct Recorder<'a> {
    pub data: Telemetry,
    origin: Instant,
    slot: &'a mut Option<Telemetry>,
}
impl<'a> Recorder<'a> {
    pub fn new(slot: &'a mut Option<Telemetry>, binding: Binding) -> Self {
        Self {
            data: Telemetry::new(binding),
            origin: Instant::now(),
            slot,
        }
    }
    pub fn now(&self) -> u64 {
        self.clamp_time(self.origin.elapsed().as_nanos())
    }
    fn clamp_time(&self, ns: u128) -> u64 {
        self.data
            .time_saturated
            .set(self.data.time_saturated.get() || ns > u64::MAX as u128);
        ns.min(u64::MAX as u128) as u64
    }
    pub fn event(&mut self, event: Event) {
        self.data.event(event, self.now());
    }
}
impl Drop for Recorder<'_> {
    fn drop(&mut self) {
        // Only fixed-size memory; no allocation, formatting, logging or I/O.
        let empty = Telemetry::new(self.data.binding);
        *self.slot = Some(std::mem::replace(&mut self.data, empty));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saturated_inputs_are_explicit_and_rates_invalid() {
        let mut slot = None;
        {
            let recorder = Recorder::new(&mut slot, Binding::new(Default::default(), false));
            assert_eq!(recorder.clamp_time(u64::MAX as u128 + 1), u64::MAX);
            assert!(recorder.data.time_saturated.get());
            assert_eq!(recorder.clamp_time(1), 1);
            assert!(recorder.data.time_saturated.get());
        }
        assert!(slot.as_ref().unwrap().time_saturated.get());
        let mut t = fixture();
        t.event(Event::AcquisitionStart, 0);
        t.accepted_frames = u64::MAX;
        t.frame(1);
        t.event(Event::AcquisitionEnd, 2);
        assert!(t.frame_count_saturated);
        for rate in [t.rate(false), t.rate(true)] {
            assert!(rate.inputs_saturated);
            assert!(rate.frames_per_second.is_none());
        }
        let mut t = fixture();
        t.event(Event::AcquisitionStart, 0);
        t.frame(1);
        t.frame(2);
        t.event(Event::AcquisitionEnd, 3);
        t.time_saturated.set(true);
        assert!(t.rate(false).frames_per_second.is_none());
        assert!(t.rate(true).frames_per_second.is_none());
        t.successful_transfers = u64::MAX;
        t.completion(4, 1);
        assert!(t.counters_saturated);
        let mut s = Summary::default();
        s.observe(u64::MAX);
        s.observe(1);
        assert!(s.saturated);
        let mut s = Summary::default();
        s.count = u64::MAX;
        s.histogram[0] = u64::MAX;
        s.observe(1);
        assert!(s.saturated);
    }

    /// Child uses the production Recorder, save implementation and helper boundary.
    /// Fake handles only: this proves no USB lifecycle behavior.
    #[test]
    fn persistence_child() {
        let Some(root) = std::env::var_os("TELEMETRY_FIXTURE_ROOT") else {
            return;
        };
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap(); // parent closes stderr first
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir(&root).unwrap();
        struct Handle<'a>(&'a Path, &'static str);
        impl Drop for Handle<'_> {
            fn drop(&mut self) {
                assert!(!self.0.join("acquisition-telemetry.json").exists());
                std::fs::write(self.0.join(self.1), b"fake handle released").unwrap();
            }
        }
        for mode in ["normal", "cancel", "error", "unwind"] {
            let mut session = Session::default();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                for slot in &mut session.records {
                    let mut recorder = Recorder::new(slot, Binding::new(Default::default(), false));
                    recorder.data.frame(1);
                }
                if mode == "unwind" {
                    let _recorder = Recorder::new(
                        &mut session.records[0],
                        Binding::new(Default::default(), false),
                    );
                    panic!("injected acquisition unwind");
                }
                match mode {
                    "normal" => Ok(()),
                    "cancel" => Err("Requested cancellation"),
                    _ => Err("original acquisition failure"),
                }
            }));
            assert!(session.records.iter().all(Option::is_some));
            assert!(!root.join("acquisition-telemetry.json").exists());
            // Modeled cleanup sentinel after the exact former destructor failure boundary.
            std::fs::write(root.join("required-cleanup-attempted"), mode).unwrap();
            let interface = Handle(&root, "interface-released");
            let device = Handle(&root, "device-released");
            // Missing parent forces every production save to fail, including cleanup slot.
            let (preserved, failures) = release_and_persist(
                outcome,
                interface,
                device,
                &session,
                Some(&root.join("absent-parent")),
                &mut std::io::stderr(),
            );
            assert_eq!(failures, [true; 5]);
            assert!(root.join("required-cleanup-attempted").is_file());
            assert!(root.join("interface-released").is_file());
            assert!(root.join("device-released").is_file());
            match mode {
                "unwind" => assert!(preserved.is_err()),
                "normal" => assert_eq!(preserved.unwrap(), Ok(())),
                "cancel" => assert_eq!(preserved.unwrap(), Err("Requested cancellation")),
                _ => assert_eq!(preserved.unwrap(), Err("original acquisition failure")),
            }
        }
        // Successful production persistence also happens only after fake release.
        let mut session = Session::default();
        {
            let _recorder = Recorder::new(
                &mut session.records[4],
                Binding::new(Default::default(), false),
            );
        }
        let (_, failures) = release_and_persist(
            (),
            Handle(&root, "interface-released"),
            Handle(&root, "device-released"),
            &session,
            Some(&root),
            &mut std::io::stderr(),
        );
        assert_eq!(failures, [false; 5]);
        assert!(root.join("acquisition-telemetry.json").is_file());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn production_failed_save_closed_stderr_preserves_result_and_release() {
        use std::process::{Command, Stdio};
        for broken in [false, true] {
            let root = std::env::temp_dir()
                .join(format!("telemetry-v013-{}-{broken}", std::process::id()));
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "acquisition_telemetry::tests::persistence_child",
                    "--nocapture",
                ])
                .env("TELEMETRY_FIXTURE_ROOT", &root)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            if broken {
                drop(child.stderr.take());
            }
            child.stdin.take().unwrap().write_all(b"go\n").unwrap();
            let deadline = Instant::now() + std::time::Duration::from_secs(30);
            loop {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    panic!("child numerical timeout");
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success(), "child failed: {output:?}");
            assert!(!root.exists());
        }
    }
    fn fixture() -> Telemetry {
        Telemetry::new(Binding::new(Default::default(), false))
    }
    #[test]
    fn injected_intervals_stop_error_and_drain_exclusion() {
        for end in [100, 120] {
            // same observations, different STOP/error exit times
            let mut t = fixture();
            t.event(Event::EnableStart, 1);
            t.event(Event::EnableAck, 10);
            t.event(Event::AcquisitionStart, 20);
            t.frame(40);
            t.frame(80);
            t.event(Event::AcquisitionEnd, end);
            t.event(Event::CancelStart, end);
            t.event(Event::CancelEnd, end + 1);
            t.event(Event::DrainStart, end + 1);
            t.event(Event::DrainEnd, 9000);
            t.event(Event::CleanupStart, 10000);
            t.event(Event::CleanupEnd, 20000);
            assert_eq!(t.rate(false).denominator_ns, Some(end - 20));
            assert_eq!(t.rate(false).numerator_frames, 2);
            assert_eq!(t.rate(true).denominator_ns, Some(40));
            assert_eq!(t.rate(true).frames_per_second, Some(25_000_000.0));
        }
    }
    #[test]
    fn undefined_regression_and_saturation() {
        let mut t = fixture();
        assert!(t.rate(false).frames_per_second.is_none());
        t.event(Event::AcquisitionStart, 5);
        t.event(Event::AcquisitionEnd, 5);
        assert!(t.rate(false).denominator_ns.is_none());
        t.frame(6);
        assert!(t.rate(true).frames_per_second.is_none());
        t.frame(6);
        assert!(t.rate(true).frames_per_second.is_none());
        t.event(Event::AcquisitionEnd, 4);
        assert_eq!(t.timestamp_regressions, 1);
        assert_eq!(t.relative_ns[Event::AcquisitionEnd as usize], Some(5));
        let mut s = Summary::default();
        s.observe(u64::MAX);
        s.observe(1);
        assert_eq!(s.cumulative_ns, u64::MAX);
        t.accepted_frames = u64::MAX;
        t.frame(10);
        assert_eq!(t.accepted_frames, u64::MAX);
    }
    #[test]
    fn fixed_storage_millions_of_events_and_histogram_boundaries() {
        let mut t = fixture();
        let size = std::mem::size_of_val(&t);
        for ns in 0..2_000_000 {
            t.completion(ns, [0, 1, 16384][ns as usize % 3]);
            t.frame(ns);
            t.parse_and_assertions.observe(ns);
        }
        assert_eq!(std::mem::size_of_val(&t), size);
        assert!(!std::mem::needs_drop::<Telemetry>()); // no heap-owned fields
        assert_eq!(t.accepted_frames, 2_000_000);
        assert_eq!(t.completion_gap.count, 1_999_999);
        assert_eq!(t.successful_transfers, 2_000_000);
        assert_eq!(t.zlp_completions, 666_667);
        assert_eq!(t.short_completions, 1_333_334);
        let mut s = Summary::default();
        for ns in [1000, 1001, 10000, 10001, u64::MAX] {
            s.observe(ns);
        }
        assert_eq!(s.histogram, [1, 2, 1, 0, 0, 0, 0, 1]);
    }
    #[test]
    fn owner_profile_binding() {
        for mode in crate::controls::Mode::ALL {
            let settings = crate::controls::Settings {
                mode,
                exposure_rows: 42,
                gain_eighths: 16,
            };
            let b = Binding::new(settings, false);
            assert_eq!((b.width, b.height), settings.dimensions());
            assert_eq!(b.clock_register, Some(settings.clock()));
            assert_eq!((b.exposure_rows, b.gain_eighths), (Some(42), Some(16)));
            let b = Binding::new(settings, true);
            assert_eq!((b.exposure_rows, b.gain_eighths), (Some(100), Some(8)));
            assert!(b.exposure_seconds.is_none());
        }
    }
}
