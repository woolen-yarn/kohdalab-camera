//! Independently derived MT9T001 experiment. Citations: PHYSICAL_REPORT.md.
use crate::acquisition_telemetry::{Binding, Event, Recorder, Session};
use nusb::{
    Interface, MaybeFuture,
    transfer::{Bulk, ControlIn, ControlType, In, Recipient},
};
use std::{
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
thread_local! { static QUIET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
// Optional logging cannot panic or precede required continuous cleanup.
macro_rules! println { ($($arg:tt)*) => { if !QUIET.get() { let _ = writeln!(std::io::stdout(), $($arg)*); } }; }
macro_rules! eprintln { ($($arg:tt)*) => { if !QUIET.get() { let _ = writeln!(std::io::stderr(), $($arg)*); } }; }
pub const CANCELLED: &str = "Requested cancellation";
pub const QUIESCENCE_FAILED: &str =
    "Transfer quiescence unproved; evidence review and physical recovery required";
/// Physical backend receipt construction and any optional persistence must live
/// outside this ownership scope. Both success and error drop interface first.
pub(crate) fn with_resource_owners<I, D, T>(
    interface: I,
    device: D,
    run: impl FnOnce(&I) -> T,
) -> T {
    let result = run(&interface);
    drop(interface);
    drop(device);
    result
}
// Explicit drain status survives recipe's legacy Result<(), String> propagation.
// Error text must never authorize further cleanup after an unproved drain.
pub(crate) fn quiescence_unproved(
    streams: &[Option<crate::acquisition_telemetry::Telemetry>],
) -> bool {
    streams
        .iter()
        .flatten()
        .any(|t| t.drain_returned_ok == Some(false))
}
pub fn cancellation(root: &Path) -> Result<(), String> {
    if root.ancestors().take(3).any(|p| p.join("STOP").exists()) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}
/// Each bounded preparation stage finishes its readback; a new stage checks STOP.
pub fn stage<T>(root: &Path, operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    cancellation(root)?;
    operation()
}
pub fn enable_stream(
    root: &Path,
    bridge: &impl Fn(u16) -> Result<(), String>,
) -> Result<(), String> {
    stage(root, || bridge(3))
}
/// Only pending-to-zero proves quiescence; cancellation completions may carry errors.
pub fn drain(
    mut step: impl FnMut(Duration) -> Result<usize, String>,
    budget: Duration,
) -> Result<(), String> {
    let start = Instant::now();
    loop {
        let pending = step(Duration::from_millis(50).min(budget.saturating_sub(start.elapsed())))
            .map_err(|_| QUIESCENCE_FAILED.to_owned())?;
        if pending == 0 {
            return Ok(());
        }
        if start.elapsed() >= budget {
            return Err(QUIESCENCE_FAILED.into());
        }
    }
}
use crate::diagnostic::SensorFailure as SF;
const TIMEOUT: Duration = Duration::from_millis(500);
// Pure response validation shared by the physical read and no-device tests.
pub(crate) fn validate_sensor_read(bytes:&[u8])->Result<u16,String> {
 if bytes.len()!=3 {return Err(SF::ReadLength.text().into());}
 if bytes[2]!=8 {return Err(SF::ReadStatus.text().into());}
 Ok(u16::from_be_bytes([bytes[0],bytes[1]]))
}
pub fn read(interface: &Interface, address: u16) -> Result<u16, String> {
    if !matches!(address, 0..=0x0a | 0x0d | 0x1e | 0x20..=0x23 | 0x2b..=0x2e | 0x32 | 0x35) {
        return Err("Read outside documented allowlist".into());
    }
    let bytes = interface
        .control_in(
            ControlIn {
                control_type: ControlType::Vendor,
                recipient: Recipient::Device,
                request: 0x0a,
                value: 0,
                index: address,
                length: 3,
            },
            TIMEOUT,
        )
        .wait()
        .map_err(|_| SF::ReadTransport.text().to_owned())?;
    println!("read {address:02x} -> {bytes:02x?}");
    validate_sensor_read(&bytes)
}
fn write(interface: &Interface, address: u16, value: u16) -> Result<(), String> {
    if !matches!(address, 1..=0x0a | 0x0d | 0x1e | 0x20..=0x23 | 0x32 | 0x35) {
        return Err("Write outside documented allowlist".into());
    }
    println!("write {address:02x} <- {value:04x}");
    if !QUIET.get() {
        let _ = std::io::stdout().flush();
    }
    let reply = interface
        .control_in(
            ControlIn {
                control_type: ControlType::Vendor,
                recipient: Recipient::Device,
                request: 0x0b,
                value,
                index: address,
                length: 1,
            },
            TIMEOUT,
        )
        .wait()
        .map_err(|_| SF::WriteTransport.text().to_owned())?;
    if reply != [8] {
        return Err(SF::WriteStatus.text().into());
    }
    Ok(())
}
fn checked(interface: &Interface, address: u16, value: u16) -> Result<(), String> {
    write(interface, address, value)?;
    let actual = read(interface, address)?;
    if actual != value {
        return Err(SF::ReadbackMismatch.text().into());
    }
    Ok(())
}
pub fn trial(
    interface: &Interface,
    width: u16,
    height: u16,
    root: &Path,
    settings: crate::controls::Settings,
    bridge: impl Fn(u16) -> Result<(), String>,
    session: &mut Session,
    preparation: crate::acquisition_recipe::CallerPreparation,
) -> Result<(), String> {
    acquire(
        interface, width, height, root, settings, bridge, session, None, None, preparation,
    )
}

/// Continuous acquisition requires a live independent supervisor capability.
/// It does not use the legacy duration as a natural end. The caller remains
/// responsible for startup IPC, lock ownership, handle release, receipt and reap.
pub(crate) fn continuous_acquire(
    interface: &Interface,
    width: u16,
    height: u16,
    root: &Path,
    settings: crate::controls::Settings,
    bridge: impl Fn(u16) -> Result<(), String>,
    session: &mut Session,
    control: &crate::continuous_runtime::OwnerPort,
    startup: crate::startup::Binding,
) -> Result<(), String> {
    control.checkpoint()?;
    QUIET.set(true);
    acquire(
        interface,
        width,
        height,
        root,
        settings,
        bridge,
        session,
        Some(control),
        Some(startup),
        Default::default(),
    )
}

fn acquire(
    interface: &Interface,
    width: u16,
    height: u16,
    root: &Path,
    settings: crate::controls::Settings,
    bridge: impl Fn(u16) -> Result<(), String>,
    session: &mut Session,
    control: Option<&crate::continuous_runtime::OwnerPort>,
    startup: Option<crate::startup::Binding>,
    preparation: crate::acquisition_recipe::CallerPreparation,
) -> Result<(), String> {
    settings.validate()?;
    if (width, height) != settings.dimensions() {
        return Err("Unsupported experiment mode".into());
    }
    if control.is_none() {
        std::fs::create_dir(root).map_err(|e| e.to_string())?;
    }
    let (streams, cleanup_slot) = session.records.split_at_mut(4);
    let mut backend = TrialRecipe { interface, root, bridge: &bridge, settings, streams, control };
    let result = orchestrate(&mut backend, settings, control.is_some(), startup.map(|s| s.intent),
        &preparation, &mut cleanup_slot[0], |b| {
            stream(interface, &root.join("live"), &bridge, false, 30, settings,
                &mut b.streams[3], control, startup)
        });
    // Finite filesystem receipts require the actual lifecycle cleanup observation.
    if cleanup_slot[0].as_ref().and_then(|t| t.cleanup_returned_ok) != Some(true) {
        return result;
    }
    if control.is_none() {
        std::fs::write(
            root.join("CLEANUP_OK"),
            b"Bridge stopped; output disabled/readback; chip ID valid\n",
        )
        .map_err(|e| e.to_string())?;
    }
    if control.is_none() && result.as_ref().err().map(String::as_str) == Some(CANCELLED) {
        std::fs::write(
            root.join("CANCELLED"),
            b"Requested cancellation after verified cleanup\n",
        )
        .map_err(|e| e.to_string())?;
    }
    result
}
/// Minimal injectable production seam. Ownership/drop and finite persistence stay outside.
pub(crate) trait AcquisitionBackend: crate::acquisition_recipe::Backend {
    fn quiescence_unproved(&self) -> bool;
    // Mandatory cleanup bypasses cancellation checkpoints, exactly as before.
    fn cleanup_disable(&mut self) -> Result<(), String>;
}
pub(crate) fn orchestrate<B: AcquisitionBackend>(
    backend: &mut B,
    settings: crate::controls::Settings,
    continuous: bool,
    intent: Option<crate::startup::Intent>,
    preparation: &crate::acquisition_recipe::CallerPreparation,
    cleanup_slot: &mut Option<crate::acquisition_telemetry::Telemetry>,
    live: impl FnOnce(&mut B) -> Result<(), String>,
) -> Result<(), String> {
    let mut binding = Binding::new(settings, false);
    binding.phase = "trial_cleanup_settings_not_observed";
    binding.clock_register = None;
    binding.exposure_rows = None;
    binding.gain_eighths = None;
    let mut lifecycle = Recorder::new(cleanup_slot, binding);
    let mut identity = crate::acquisition_recipe::IdentityGate::default();
    let result = (|| {
        identity.start(backend, continuous, preparation)?;
        if continuous {
            crate::acquisition_recipe::initialize_intent(backend, settings,
                intent.ok_or("Missing continuous startup binding")?)
                .map_err(|e| crate::diagnostic::backend_text("@IN", e))?;
        } else {
            crate::acquisition_recipe::initialize(backend, settings)?;
        }
        live(backend).map_err(|e| crate::diagnostic::backend_text("@ST", e))?;
        if !continuous { backend.checkpoint()?; }
        if backend.read(7)? != 2 { return Err(SF::ReadbackMismatch.text().into()); }
        Ok(())
    })();
    if !identity.verified { return identity.finish_unverified(backend, result); }
    if backend.quiescence_unproved() { return result; }
    backend.progress(crate::progress::Stage::Cleanup);
    lifecycle.event(Event::CleanupStart);
    let cleanup: Result<(), String> = (|| {
        backend.bridge(1)?;
        backend.cleanup_disable()?;
        if backend.read(0)? != 0x1621 { return Err(SF::IdentityMismatch.text().into()); }
        Ok(())
    })();
    lifecycle.data.cleanup_returned_ok = Some(cleanup.is_ok());
    if cleanup.is_ok() { lifecycle.event(Event::CleanupEnd); }
    if let Err(error) = cleanup {
        return Err(result.err().filter(|e| e != CANCELLED).unwrap_or_else(|| {
            if crate::diagnostic::backend_code(&error).origin == crate::diagnostic::Origin::Backend {error}
            else {format!("@CU Cleanup failed: {error}; STOP ALL WRITES; physical reconnection required")}
        }));
    }
    result
}
struct TrialRecipe<'a, B> {
    interface: &'a Interface,
    root: &'a Path,
    bridge: &'a B,
    settings: crate::controls::Settings,
    streams: &'a mut [Option<crate::acquisition_telemetry::Telemetry>],
    control: Option<&'a crate::continuous_runtime::OwnerPort>,
}
impl<B: Fn(u16) -> Result<(), String>> crate::acquisition_recipe::Backend for TrialRecipe<'_, B> {
    fn checkpoint(&mut self) -> Result<(), String> {
        if let Some(control) = self.control {
            control.checkpoint()?;
        }
        if self.control.is_none() {
            cancellation(self.root)
        } else {
            Ok(())
        }
    }
    fn write(&mut self, a: u16, v: u16) -> Result<(), String> {
        self.checkpoint()?;
        write(self.interface, a, v)
    }
    fn checked(&mut self, a: u16, v: u16) -> Result<(), String> {
        self.checkpoint()?;
        checked(self.interface, a, v)
    }
    fn read(&mut self, a: u16) -> Result<u16, String> {
        read(self.interface, a)
    }
    fn bridge(&mut self, v: u16) -> Result<(), String> {
        (self.bridge)(v)
    }
    fn settle(&mut self, delay: Duration) -> Result<(), String> {
        std::thread::sleep(delay);
        Ok(())
    }
    fn progress(&mut self, stage: crate::progress::Stage) {
        if self.control.is_none() {
            crate::progress::publish(stage);
        }
    }
    fn pattern(&mut self, i: usize, name: &str, duration: Duration) -> Result<(), String> {
        stream(
            self.interface,
            &self.root.join(name),
            self.bridge,
            true,
            duration.as_secs(),
            self.settings,
            &mut self.streams[i],
            self.control,
            None,
        )
    }
}
impl<B: Fn(u16) -> Result<(), String>> AcquisitionBackend for TrialRecipe<'_, B> {
    fn quiescence_unproved(&self) -> bool { quiescence_unproved(self.streams) }
    fn cleanup_disable(&mut self) -> Result<(), String> { checked(self.interface, 7, 0) }
}
/// Exactly the production boundary rule: unsynchronized input is discarded;
/// an exact frame must terminate with a short completion (including ZLP).
fn assemble(
    candidate: &mut Vec<u8>,
    synced: &mut bool,
    size: usize,
    data: &[u8],
    discarded: &mut u64,
    shorts: &mut u64,
) -> bool {
    if *synced && candidate.len() + data.len() <= size {
        candidate.extend_from_slice(data);
    } else if *synced {
        *synced = false;
        *discarded += 1;
        candidate.clear();
    }
    if data.len() < 16384 {
        *shorts += 1;
        if *synced && candidate.len() == size {
            return true;
        }
        *discarded += 1;
        candidate.clear();
        *synced = true;
    }
    false
}

#[cfg(test)]
mod tail_v020_tests {
    use super::assemble;
    use crate::tail_provenance::TailProvenance;
    struct Harness { bytes: Vec<u8>, synced: bool, discarded: u64, shorts: u64, accepted: u64, marked: bool, p: TailProvenance }
    impl Harness {
        fn new() -> Self { Self { bytes: vec![], synced:false, discarded:0, shorts:0, accepted:0, marked:false, p:Default::default() } }
        fn input(&mut self, data:&[u8], cancel:u8) {
            // 1 before assembly, 2 before acceptance, 3 after accepted telemetry,
            // 4 after publication before clear. These are observation cut points.
            if cancel == 1 { return; }
            let before=self.discarded; let synced=self.synced;
            let overflow=synced && self.bytes.len()+data.len()>32768;
            let complete=assemble(&mut self.bytes,&mut self.synced,32768,data,&mut self.discarded,&mut self.shorts);
            self.p.assembly(synced,overflow,self.discarded-before); self.marked=false;
            if complete {
                if cancel == 2 { return; }
                self.accepted+=1; self.marked=true;
                if cancel == 3 || cancel == 4 { return; }
                self.bytes.clear(); self.marked=false;
            }
        }
        fn tail(&mut self)->TailProvenance { self.p.finish(self.bytes.len(),32768,self.marked); self.p }
        fn sync(&mut self) { self.input(&[1],0); }
        fn prefix(&mut self) { self.input(&[0;16384],0);self.input(&[0;16384],0); }
    }
    #[test]
    fn cancellation_checkpoints_v020_tail_attribution() {
        for cancel in 1..=4 {
            let mut h=Harness::new();h.sync();h.prefix();h.input(&[],cancel);
            let p=h.tail();assert_eq!(p.initial_sync,1);assert_eq!(p.tail_bytes,32768);
            assert_eq!(p.tail_state,if cancel <= 2 {3}else{2});
            assert_eq!(h.accepted,if cancel <=2 {0}else{1});
            assert_eq!(p.intervening_boundary,0);assert_eq!(p.intervening_overflow,0);
        }
        let mut h=Harness::new(); assert_eq!(h.tail().tail_state,0);
        h.sync();h.input(&[0;16384],0);assert_eq!(h.tail().tail_state,1);
        h.input(&[0;16384],0);h.input(&[],0);assert_eq!(h.accepted,1);
        assert_eq!(h.tail().tail_state,0);assert_eq!(h.p.initial_sync,1);
    }
    #[test]
    fn intervening_bad_short_overflow_v020_preserves_aggregate() {
        let mut h=Harness::new();h.sync();h.prefix();h.input(&[],0);
        h.input(&[1],0);assert_eq!(h.p.intervening_boundary,1);
        h.prefix();h.input(&[0;16384],0); // overflow, awaiting resynchronization
        h.input(&[],0);h.prefix();h.input(&[],0);
        assert_eq!(h.accepted,2);assert_eq!(h.p.initial_sync,1);
        assert_eq!(h.p.intervening_overflow,1);assert_eq!(h.p.intervening_boundary,2);
        assert_eq!(h.discarded,4);assert_eq!(h.tail().tail_state,0);
    }
}
fn stream(
    interface: &Interface,
    root: &Path,
    bridge: &impl Fn(u16) -> Result<(), String>,
    pattern: bool,
    seconds: u64,
    settings: crate::controls::Settings,
    slot: &mut Option<crate::acquisition_telemetry::Telemetry>,
    control: Option<&crate::continuous_runtime::OwnerPort>,
    startup: Option<crate::startup::Binding>,
) -> Result<(), String> {
    if control.is_none() {
        std::fs::create_dir(root).map_err(|e| e.to_string())?;
    }
    let ep = interface
        .endpoint::<Bulk, In>(0x82)
        .map_err(|e| crate::diagnostic::backend_text("@ST",e.to_string()))?;
    stream_endpoint(ep, root, bridge, pattern, seconds, settings, slot, control, startup).map_err(|e|crate::diagnostic::backend_text("@ST",e))
}

trait StreamEndpoint {
    fn allocate(&self, len: usize) -> nusb::transfer::Buffer;
    fn submit(&mut self, buffer: nusb::transfer::Buffer);
    fn wait_next_complete(&mut self, timeout: Duration) -> Option<nusb::transfer::Completion>;
    fn pending(&self) -> usize;
    fn cancel_all(&mut self);
}
impl StreamEndpoint for nusb::Endpoint<Bulk, In> {
    fn allocate(&self, len: usize) -> nusb::transfer::Buffer {
        self.allocate(len)
    }
    fn submit(&mut self, b: nusb::transfer::Buffer) {
        self.submit(b)
    }
    fn wait_next_complete(&mut self, d: Duration) -> Option<nusb::transfer::Completion> {
        self.wait_next_complete(d)
    }
    fn pending(&self) -> usize {
        self.pending()
    }
    fn cancel_all(&mut self) {
        self.cancel_all()
    }
}
fn stream_endpoint(
    mut ep: impl StreamEndpoint,
    root: &Path,
    bridge: &impl Fn(u16) -> Result<(), String>,
    pattern: bool,
    seconds: u64,
    settings: crate::controls::Settings,
    slot: &mut Option<crate::acquisition_telemetry::Telemetry>,
    control: Option<&crate::continuous_runtime::OwnerPort>,
    startup: Option<crate::startup::Binding>,
) -> Result<(), String> {
    let (width, height) = settings.dimensions();
    // 16 KiB multiple of descriptor's 512 B packet; eight in flight = 128 KiB.
    let submitted = (|| -> Result<(), String> {
        for _ in 0..8 {
            let b = ep.allocate(16384);
            if let Some(control) = control {
                control.checkpoint()?;
            }
            ep.submit(b);
        }
        Ok(())
    })();
    let mut telemetry = Recorder::new(slot, Binding::new(settings, pattern));
    telemetry.event(Event::EnableStart);
    let enable = (|| {
        submitted?;
        if let Some(control) = control {
            control.checkpoint()?;
            if !pattern {
                control.enable_live()?;
            }
        }
        if control.is_some() {
            bridge(3)
        } else {
            enable_stream(root, bridge)
        }
    })();
    if enable.is_ok() {
        telemetry.event(Event::EnableAck);
    }
    let start = Instant::now();
    let size = width as usize * height as usize;
    let mut candidate = Vec::with_capacity(size);
    let mut provenance = crate::tail_provenance::TailProvenance::default();
    let mut candidate_accepted = false;
    let mut synced = false;
    let (mut accepted, mut discarded, mut bytes, mut transfers, mut shorts) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut first = None;
    let mut last = None;
    let mut periodic_frames = 0u64;
    let mut expected_zero_frames = 0u64;
    let mut expected_155_frames = 0u64;
    let mut expected_123_frames = 0u64;
    let mut preview_last = Instant::now() - Duration::from_secs(1);
    let result: Result<(), String> = (|| {
        enable?;
        telemetry.event(Event::AcquisitionStart);
        while (!pattern && control.is_some()) || start.elapsed() < Duration::from_secs(seconds) {
            if let Some(control) = control {
                control.checkpoint()?;
            }
            if control.is_none() {
                cancellation(root)?;
            }
            let wait_start = telemetry.now();
            let completion = ep.wait_next_complete(TIMEOUT);
            let completion_time = telemetry.now();
            telemetry
                .data
                .wait
                .observe(completion_time.saturating_sub(wait_start));
            if completion.is_none() {
                telemetry.data.increment_wait_timeouts();
            }
            let c = completion.ok_or("Bulk completion timeout")?;
            if c.status.is_err() {
                telemetry.data.increment_transfer_errors();
            }
            c.status.map_err(|e| e.to_string())?;
            if let Some(control) = control {
                control.checkpoint()?;
            }
            let data = c.buffer.as_ref();
            if !pattern && !data.is_empty() {
                if let Some(control) = control {
                    control.nonempty_transfer()?;
                }
            }
            telemetry.data.completion(completion_time, data.len());
            let parse_start = telemetry.now();
            if synced && candidate.len() + data.len() > size {
                telemetry.data.increment_overflow_events();
            }
            transfers += 1;
            bytes += data.len() as u64;
            let was_synced = synced;
            let overflow = synced && candidate.len() + data.len() > size;
            let discarded_before = discarded;
            let complete = assemble(
                &mut candidate,
                &mut synced,
                size,
                data,
                &mut discarded,
                &mut shorts,
            );
            provenance.assembly(was_synced, overflow, discarded - discarded_before);
            candidate_accepted = false;
            if complete {
                accepted += 1;
                candidate_accepted = true;
                if candidate
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .all(|p| *p == [0x55, 0xaa])
                {
                    expected_155_frames += 1;
                }
                if candidate
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .all(|p| *p == [0x48, 0xb7])
                {
                    expected_123_frames += 1;
                }
                if candidate
                    .iter()
                    .enumerate()
                    .all(|(i, b)| *b == candidate[i % 2])
                {
                    periodic_frames += 1;
                }
                if candidate
                    .iter()
                    .enumerate()
                    .all(|(i, b)| *b == if i % 2 == 0 { 0 } else { 255 })
                    || candidate
                        .iter()
                        .enumerate()
                        .all(|(i, b)| *b == if i % 2 == 0 { 255 } else { 0 })
                {
                    expected_zero_frames += 1;
                }
                telemetry
                    .data
                    .parse_and_assertions
                    .observe(telemetry.now().saturating_sub(parse_start));
                telemetry.data.frame(telemetry.now());
                if !pattern {
                    if let Some(control) = control {
                        control.complete_frame()?;
                    }
                }
                if control.is_none() {
                    let copy_start = telemetry.now();
                    if first.is_none() {
                        first = Some(candidate.clone());
                    }
                    // Diagnostic ownership is confined to the legacy path.
                    last.get_or_insert_with(|| Vec::with_capacity(size))
                        .clone_from(&candidate);
                    telemetry
                        .data
                        .diagnostic_copy
                        .observe(telemetry.now().saturating_sub(copy_start));
                }
                if !pattern && control.is_some() {
                    control.unwrap().publish_frame(crate::Frame {
                        raw: std::sync::Arc::new(candidate.clone()),
                        metadata: crate::Metadata {
                            acquisition: Some(startup.ok_or("Missing frame startup binding")?.acquisition(settings,accepted)?),
source: "Continuous physical".into(),
                            simulated: false,
                            sequence: accepted,
                            measurement: None, unix_millis: 0,
                            width: width.into(),
                            height: height.into(),
                            format: "BayerGRBG8".into(),
                        },
                    })?;
                }
                if !pattern
                    && control.is_none()
                    && preview_last.elapsed() >= Duration::from_millis(66)
                {
                    let publication_start = telemetry.now();
                    let mut bytes = Vec::with_capacity(16);
                    bytes.extend_from_slice(&accepted.to_le_bytes());
                    bytes.extend_from_slice(
                        &(std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_err(|e| e.to_string())?
                            .as_millis() as u64)
                            .to_le_bytes(),
                    );
                    let tmp = root.join("preview.tmp");
                    let target = root.join("preview.frame");
                    {
                        let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
                        file.write_all(&bytes).map_err(|e| e.to_string())?;
                        file.write_all(&candidate).map_err(|e| e.to_string())?;
                    }
                    if target.exists() {
                        std::fs::remove_file(&target).map_err(|e| e.to_string())?;
                    }
                    std::fs::rename(tmp, target).map_err(|e| e.to_string())?;
                    preview_last = Instant::now();
                    telemetry
                        .data
                        .preview_publication
                        .observe(telemetry.now().saturating_sub(publication_start));
                }
                candidate.clear();
                candidate_accepted = false;
            } else {
                telemetry
                    .data
                    .parse_and_assertions
                    .observe(telemetry.now().saturating_sub(parse_start));
            }
            if let Some(control) = control {
                control.checkpoint()?;
            }
            ep.submit(c.buffer);
            if control.is_none() && root.ancestors().take(3).any(|p| p.join("STOP").exists()) {
                return Err(CANCELLED.into());
            }
        }
        Ok(())
    })();
    if telemetry.data.relative_ns[Event::AcquisitionStart as usize].is_some() {
        telemetry.event(Event::AcquisitionEnd);
    }
    telemetry.data.acquisition_exit = Some(match result.as_ref().err().map(String::as_str) {
        None => "duration_complete",
        Some(CANCELLED) => "cancelled",
        Some(_) => "error",
    });
    provenance.finish(candidate.len(), size, candidate_accepted);
    telemetry.data.tail_provenance = Some(provenance);
    telemetry.data.counters_saturated |= discarded == u64::MAX;
    telemetry.data.discarded_fragment_events = discarded;
    if !candidate.is_empty() {
        telemetry.data.counters_saturated |= discarded == u64::MAX;
        telemetry.data.discarded_fragment_events = discarded.saturating_add(1);
    }
    telemetry.event(Event::CancelStart);
    ep.cancel_all();
    telemetry.event(Event::CancelEnd);
    telemetry.event(Event::DrainStart);
    let drained = drain(
        |wait| {
            if ep.pending() > 0 {
                let _ = ep.wait_next_complete(wait);
            }
            Ok(ep.pending())
        },
        Duration::from_secs(1),
    );
    telemetry.event(Event::DrainEnd);
    telemetry.data.drain_returned_ok = Some(drained.is_ok());
    if drained.is_err() {
        let detail = format!(
            "{QUIESCENCE_FAILED}; pending={} after bounded cancellation drain",
            ep.pending()
        );
        eprintln!("{detail}");
        if control.is_none() {
            let _ = std::fs::write(root.join("QUIESCENCE_FAILED"), detail);
        }
        // Preserve a non-STOP acquisition fault verbatim, independently of
        // drain status. STOP/success retain the exact legacy drain sentinel.
        return Err(result
            .err()
            .filter(|e| e != CANCELLED)
            .unwrap_or_else(|| QUIESCENCE_FAILED.into()));
    }
    // Mandatory continuous checks are in memory, before outer cleanup. Never
    // enter legacy codecs, logging or filesystem diagnostics with owners live.
    if control.is_some() {
        result?;
        if accepted == 0 {
            return Err("No complete frames; no capture support asserted".into());
        }
        if pattern {
            let matched = match root.file_name().and_then(|n| n.to_str()) {
                Some("pattern-155") => expected_155_frames,
                Some("pattern-123") => expected_123_frames,
                Some("pattern-zero") => expected_zero_frames,
                _ => 0,
            };
            if matched != accepted {
                return Err("Known-pattern assertion failed; live preview disabled".into());
            }
        }
        return Ok(());
    }
    if !candidate.is_empty() {
        discarded += 1;
    }
    let metrics = serde_json::json!({"width":width,"height":height,"elapsed_ms":start.elapsed().as_millis(),"bytes":bytes,"transfers":transfers,"short_transfers":shorts,"accepted_complete_frames":accepted,"periodic_frames":periodic_frames,"pattern_155_frames":expected_155_frames,"pattern_123_frames":expected_123_frames,"zero_ff_frames":expected_zero_frames,"discarded_fragments":discarded,"pending_after_cancel":ep.pending(),"error":result.as_ref().err()});
    println!("metrics={metrics}");
    if control.is_none() {
        std::fs::write(
            root.join("metrics.json"),
            serde_json::to_vec_pretty(&metrics).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if let Some(raw) = last {
            std::fs::write(root.join("last-raw.bin"), raw).map_err(|e| e.to_string())?;
        }
    }
    if let Some(raw) = first {
        let mut histogram = vec![0u64; 256];
        for &b in &raw {
            histogram[b as usize] += 1;
        }
        let reference = [raw[0], raw[1]];
        let mismatches = raw
            .iter()
            .enumerate()
            .filter(|(i, b)| **b != reference[i % 2])
            .count();
        let integrity = serde_json::json!({"pattern_enabled":pattern,"first_pair":reference,"period_two_mismatches":mismatches,"histogram":histogram,"interpretation":"Raw USB bytes; pixel mapping and image integrity unvalidated"});
        println!("first_frame_integrity={integrity}");
        if control.is_none() {
            std::fs::write(
                root.join("integrity.json"),
                serde_json::to_vec_pretty(&integrity).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            std::fs::write(root.join("first-raw.bin"), &raw).map_err(|e| e.to_string())?;
        }
        // Lossless grayscale rendering of received bytes, not a color reconstruction.
        for (extension, format) in [
            ("png", image::ImageFormat::Png),
            ("tiff", image::ImageFormat::Tiff),
        ] {
            let path = root.join(format!("byte-plane.{extension}"));
            image::save_buffer_with_format(
                &path,
                &raw,
                width.into(),
                height.into(),
                image::ColorType::L8,
                format,
            )
            .map_err(|e| e.to_string())?;
            if image::open(&path)
                .map_err(|e| e.to_string())?
                .to_luma8()
                .as_raw()
                != &raw
            {
                return Err("Lossless output decode equality failed".into());
            }
        }
        println!("lossless_byte_plane=PNG/TIFF decode equality passed; original raw preserved");
    }
    result?;
    if accepted == 0 {
        return Err("No complete frames; no capture support asserted".into());
    }
    if pattern {
        let matched = match root.file_name().and_then(|n| n.to_str()) {
            Some("pattern-155") => expected_155_frames,
            Some("pattern-123") => expected_123_frames,
            Some("pattern-zero") => expected_zero_frames,
            _ => 0,
        };
        if matched != accepted {
            return Err("Known-pattern assertion failed; live preview disabled".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod repair_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    struct FakeResource(&'static str, Arc<Mutex<Vec<&'static str>>>);
    impl Drop for FakeResource {
        fn drop(&mut self) {
            self.1.lock().unwrap().push(self.0);
        }
    }
    struct PatternEndpoint {
        remaining: usize,
        synced: bool,
        terminated: bool,
        pending: usize,
        stop: bool,
        stuck: bool,
        draining: bool,
        monitor: crate::continuous_runtime::Monitor,
        order: Arc<Mutex<Vec<&'static str>>>,
    }
    impl StreamEndpoint for PatternEndpoint {
        fn allocate(&self, n: usize) -> nusb::transfer::Buffer {
            nusb::transfer::Buffer::new(n)
        }
        fn submit(&mut self, _: nusb::transfer::Buffer) {
            self.pending += 1;
        }
        fn pending(&self) -> usize {
            self.pending
        }
        fn cancel_all(&mut self) {
            self.order.lock().unwrap().push("cancel/drain");
            self.draining = true;
            if !self.stuck {
                self.pending = 0;
            }
        }
        fn wait_next_complete(&mut self, _: Duration) -> Option<nusb::transfer::Completion> {
            if self.draining && self.stuck {
                std::thread::sleep(Duration::from_millis(10));
                return None;
            }
            self.pending -= 1;
            let (data, status) = if !self.synced {
                self.synced = true;
                (vec![], Ok(()))
            } else if self.remaining > 0 {
                let n = self.remaining.min(16384);
                self.remaining -= n;
                (
                    (0..n)
                        .map(|i| if i % 2 == 0 { 0x55 } else { 0xaa })
                        .collect(),
                    Ok(()),
                )
            } else if !self.terminated {
                self.terminated = true;
                self.order.lock().unwrap().push("first pattern frame");
                (vec![], Ok(()))
            } else {
                if self.stop {
                    self.monitor.stop();
                }
                (
                    vec![],
                    if self.stop {
                        Ok(())
                    } else {
                        Err(nusb::transfer::TransferError::Disconnected)
                    },
                )
            };
            let actual_len = data.len();
            Some(nusb::transfer::Completion {
                buffer: data.into(),
                actual_len,
                status,
            })
        }
    }
    impl Drop for PatternEndpoint {
        fn drop(&mut self) {
            assert_eq!(self.pending == 0, !self.stuck);
            self.order.lock().unwrap().push("endpoint drop");
        }
    }
    #[test]
    fn continuous_first_pattern_then_stop_error_never_enters_optional_export() {
        use crate::{
            continuous::{Kind, Record},
            continuous_runtime as runtime,
        };
        QUIET.set(true);
        for (stop, stuck) in [(false, false), (true, false), (false, true), (true, true)] {
            let (owner, mut lease, monitor) =
                runtime::session([1; 32], [2; 32], Arc::new(runtime::Monotonic::default()));
            lease.challenge([3; 32]).unwrap();
            assert_eq!(
                lease.response(
                    &Record {
                        kind: Kind::Response,
                        session: [1; 32],
                        binding: [2; 32],
                        sequence: 1,
                        challenge: [3; 32]
                    }
                    .encode()
                ),
                Ok(true)
            );
            let settings = crate::controls::Settings::default();
            let (w, h) = settings.dimensions();
            let order = Arc::new(Mutex::new(Vec::new()));
            let ep = PatternEndpoint {
                remaining: w as usize * h as usize,
                synced: false,
                terminated: false,
                pending: 0,
                stop,
                stuck,
                draining: false,
                monitor,
                order: order.clone(),
            };
            let mut telemetry = None;
            // Deliberately unusable output: continuous stream must do no path IO.
            let result = with_resource_owners(
                FakeResource("interface drop", order.clone()),
                FakeResource("device drop", order.clone()),
                |_| {
                    let result = stream_endpoint(
                        ep,
                        Path::new("missing-parent/never-created/pattern-155"),
                        &|_| Ok(()),
                        true,
                        5,
                        settings,
                        &mut telemetry,
                        Some(&owner),
                        None,
                    );
                    if !quiescence_unproved(std::slice::from_ref(&telemetry)) {
                        order
                            .lock()
                            .unwrap()
                            .extend(["bridge stop", "disable/readbacks"]);
                    }
                    result
                },
            );
            assert_eq!(
                result.as_ref().map_err(String::as_str),
                Err(if stop && stuck {
                    QUIESCENCE_FAILED
                } else if stop {
                    CANCELLED
                } else {
                    "device disconnected"
                })
            );
            assert_eq!(quiescence_unproved(std::slice::from_ref(&telemetry)), stuck);
            #[cfg(windows)]
            crate::continuous_session::assert_dual_failure_mapping(
                result.clone(),
                &mut telemetry,
                stuck,
                stop,
            );
            let telemetry = telemetry.unwrap();
            assert_eq!(telemetry.drain_returned_ok, Some(!stuck));
            assert_eq!(telemetry.accepted_frames, 1);
            assert_eq!(telemetry.diagnostic_copy.count, 0);
            assert_eq!(
                *order.lock().unwrap(),
                if stuck {
                    vec![
                        "first pattern frame",
                        "cancel/drain",
                        "endpoint drop",
                        "interface drop",
                        "device drop",
                    ]
                } else {
                    vec![
                        "first pattern frame",
                        "cancel/drain",
                        "endpoint drop",
                        "bridge stop",
                        "disable/readbacks",
                        "interface drop",
                        "device drop",
                    ]
                }
            );
            // Even a failing or permanently held optional exporter cannot be
            // reached from this shared production stream. Exercise a delayed
            // optional consumer only after the same production ownership scope.
            let (entered, after_drop) = std::sync::mpsc::sync_channel(1);
            let (release, blocked) = std::sync::mpsc::sync_channel(1);
            let original = result.clone();
            let export_order = order.clone();
            let exporter = std::thread::spawn(move || {
                assert_eq!(export_order.lock().unwrap().last(), Some(&"device drop"));
                entered.send(()).unwrap();
                blocked.recv().unwrap(); // a held optional IO cannot hold resources
                Err::<(), _>("Injected export failure")
            });
            after_drop.recv().unwrap();
            assert_eq!(result, original);
            release.send(()).unwrap();
            assert!(exporter.join().unwrap().is_err());
            assert_eq!(result, original);
        }
    }
    #[test]
    fn telemetry_partial_overflow_short_zlp_fixture() {
        let mut t =
            crate::acquisition_telemetry::Telemetry::new(Binding::new(Default::default(), false));
        let (mut candidate, mut synced, mut discarded, mut shorts) = (Vec::new(), false, 0, 0);
        for (ns, data) in [&[9][..], &[1, 2], &[1, 2, 3, 4, 5], &[1, 2, 3, 4], &[]]
            .into_iter()
            .enumerate()
        {
            t.completion(ns as u64, data.len());
            if synced && candidate.len() + data.len() > 4 {
                t.overflow_events += 1;
            }
            if assemble(
                &mut candidate,
                &mut synced,
                4,
                data,
                &mut discarded,
                &mut shorts,
            ) {
                t.frame(ns as u64);
                candidate.clear();
            }
        }
        t.discarded_fragment_events = discarded;
        assert_eq!(
            (
                t.successful_transfers,
                t.nonempty_transfers,
                t.nonempty_bytes
            ),
            (5, 4, 12)
        );
        assert_eq!((t.short_completions, t.zlp_completions, shorts), (5, 1, 5));
        assert_eq!(
            (
                t.accepted_frames,
                t.overflow_events,
                t.discarded_fragment_events
            ),
            (1, 1, 5)
        );
    }
    #[test]
    #[ignore = "explicit bounded synthetic overhead benchmark; no USB"]
    fn telemetry_synthetic_overhead_benchmark() {
        use std::hint::black_box;
        for mode in crate::controls::Mode::ALL {
            let settings = crate::controls::Settings {
                mode,
                ..Default::default()
            };
            let p = mode.profile();
            let size = usize::from(p.width) * usize::from(p.height);
            let raw: Vec<u8> = (0..size).map(|i| [0x55, 0xaa][i % 2]).collect();
            let mut copy = vec![0; size];
            for enabled in [false, true] {
                let mut t =
                    crate::acquisition_telemetry::Telemetry::new(Binding::new(settings, false));
                let origin = Instant::now();
                let now = || origin.elapsed().as_nanos().min(u64::MAX as u128) as u64;
                for _ in 0..32 {
                    for chunk in raw.chunks(16384) {
                        if enabled {
                            let a = now();
                            t.completion(a, black_box(chunk.len()));
                            t.wait.observe(now().saturating_sub(a));
                            t.parse_and_assertions.observe(now().saturating_sub(a));
                        }
                    }
                    let a = now();
                    black_box(raw.as_chunks::<2>().0.iter().all(|p| *p == [0x55, 0xaa]));
                    black_box(raw.as_chunks::<2>().0.iter().all(|p| *p == [0x48, 0xb7]));
                    black_box(raw.iter().enumerate().all(|(i, b)| *b == raw[i % 2]));
                    black_box(
                        raw.iter()
                            .enumerate()
                            .all(|(i, b)| *b == if i % 2 == 0 { 0 } else { 255 })
                            || raw
                                .iter()
                                .enumerate()
                                .all(|(i, b)| *b == if i % 2 == 0 { 255 } else { 0 }),
                    );
                    if enabled {
                        t.parse_and_assertions.observe(now().saturating_sub(a));
                        t.frame(now());
                    }
                    let a = now();
                    copy.clone_from_slice(black_box(&raw));
                    black_box(&copy);
                    if enabled {
                        t.diagnostic_copy.observe(now().saturating_sub(a));
                    }
                }
                let ns = origin.elapsed().as_nanos();
                println!(
                    "synthetic profile={} enabled={} frames=32 bytes={} wall_ns={} payload_bytes_per_s={} target_bytes_per_s=28311552 implied_target_frame_rate={} telemetry_size={} hot_telemetry_heap_fields={} parse_ns={} copy_ns={}",
                    p.id,
                    enabled,
                    size * 32,
                    ns,
                    size as f64 * 32.0 * 1e9 / ns as f64,
                    28_311_552.0 / size as f64,
                    std::mem::size_of_val(&t),
                    std::mem::needs_drop::<crate::acquisition_telemetry::Telemetry>(),
                    t.parse_and_assertions.cumulative_ns,
                    t.diagnostic_copy.cumulative_ns
                );
            }
        }
    }
    #[test]
    fn resolution_three_pattern_generated_framing() {
        for mode in crate::controls::Mode::ALL {
            let p = mode.profile();
            let size = usize::from(p.width) * usize::from(p.height);
            for pair in [[0x55, 0xaa], [0x48, 0xb7], [0, 255]] {
                let raw: Vec<_> = (0..size).map(|i| pair[i % 2]).collect();
                let (mut candidate, mut synced, mut discarded, mut shorts) =
                    (Vec::with_capacity(size), false, 0, 0);
                assert!(!assemble(
                    &mut candidate,
                    &mut synced,
                    size,
                    &[9],
                    &mut discarded,
                    &mut shorts
                ));
                assert_eq!((discarded, shorts), (1, 1));
                let mut accepted = 0;
                for _ in 0..2 {
                    let mut complete = false;
                    for chunk in raw.chunks(16384) {
                        complete = assemble(
                            &mut candidate,
                            &mut synced,
                            size,
                            chunk,
                            &mut discarded,
                            &mut shorts,
                        );
                        assert_eq!(complete, chunk.len() < 16384);
                    }
                    if !complete {
                        assert!(assemble(
                            &mut candidate,
                            &mut synced,
                            size,
                            &[],
                            &mut discarded,
                            &mut shorts
                        ));
                    }
                    assert_eq!(candidate, raw);
                    assert!(candidate.as_chunks::<2>().0.iter().all(|v| *v == pair));
                    accepted += 1;
                    candidate.clear();
                }
                assert_eq!((accepted, discarded, shorts), (2, 1, 3));
                assert!(!assemble(
                    &mut candidate,
                    &mut synced,
                    size,
                    &[1, 2],
                    &mut discarded,
                    &mut shorts
                ));
                candidate.extend_from_slice(&raw);
                assert!(!assemble(
                    &mut candidate,
                    &mut synced,
                    size,
                    &[1],
                    &mut discarded,
                    &mut shorts
                ));
                assert_eq!((discarded, shorts), (4, 5));
                assert!(candidate.is_empty() && synced);
            }
        }
    }
    #[test]
    fn drain_timely_delayed_error_completions_and_deadline() {
        for pending in [vec![0], vec![3, 2, 1, 0], vec![2, 2, 1, 0]] {
            let mut queue = pending.into_iter();
            assert!(drain(|_| Ok(queue.next().unwrap()), Duration::from_secs(1)).is_ok());
        }
        assert_eq!(
            drain(
                |_| Err("completion API failure".into()),
                Duration::from_secs(1)
            )
            .unwrap_err(),
            QUIESCENCE_FAILED
        );
        let start = Instant::now();
        assert!(
            drain(
                |wait| {
                    std::thread::sleep(wait);
                    Ok(2)
                },
                Duration::from_millis(2)
            )
            .is_err()
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        assert_eq!(
            drain(|_| Ok(1), Duration::ZERO).unwrap_err(),
            QUIESCENCE_FAILED
        );
    }
    #[test]
    fn stop_between_preparation_stages_and_before_enable() {
        let root = std::env::temp_dir().join(format!("stage-stop-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        for stop_at in 0..30 {
            let mut performed = 0;
            for stage in 0..30 {
                if stage == stop_at {
                    std::fs::write(root.join("STOP"), b"stop").unwrap();
                }
                if super::stage(&root, || {
                    performed += 1;
                    Ok(())
                })
                .is_err()
                {
                    break;
                }
            }
            assert_eq!(performed, stop_at);
            let enabled = std::cell::Cell::new(false);
            assert_eq!(
                enable_stream(&root, &|value| {
                    assert_eq!(value, 3);
                    enabled.set(true);
                    Ok(())
                })
                .unwrap_err(),
                CANCELLED
            );
            assert!(!enabled.get());
            std::fs::remove_file(root.join("STOP")).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }
}

#[cfg(test)] mod v039_tests {
 use super::*;
 #[test] fn v039_production_read_validation() {
  for bytes in [&[][..], &[0,0][..], &[0,0,8,0][..]] {assert_eq!(validate_sensor_read(bytes).unwrap_err(),SF::ReadLength.text());}
  assert_eq!(validate_sensor_read(&[0x16,0x21,7]).unwrap_err(),SF::ReadStatus.text());
  assert_eq!(validate_sensor_read(&[0x16,0x21,8]).unwrap(),0x1621);
 }
}

#[cfg(test)]
pub(crate) mod v041_tests {
    use super::*;
    use crate::{acquisition_recipe::{Backend, CallerPreparation}, controls::Settings, startup::Intent};
    #[derive(Default)]
    pub(crate) struct Recording {
        pub ops: Vec<String>, checkpoints: usize, cancel_at: Option<usize>,
        bridge_calls: usize, fail_bridge: Option<usize>, reply: Option<Vec<u8>>,
        identity_error: Option<String>, gain: u16, cleanup_error: bool, cleanup_identity_error: bool, init_error: bool,
        live_error: Option<String>, unproved: bool,
        streams: [Option<crate::acquisition_telemetry::Telemetry>; 4],
    }
    impl Backend for Recording {
        fn checkpoint(&mut self)->Result<(),String> {
            let n=self.checkpoints;self.checkpoints+=1;
            if self.cancel_at==Some(n){Err(CANCELLED.into())}else{Ok(())}
        }
        fn bridge(&mut self,v:u16)->Result<(),String> {
            let n=self.bridge_calls;self.bridge_calls+=1;self.ops.push(format!("B{v}"));
            if self.fail_bridge==Some(n){Err("@IN preparation transport".into())}else{Ok(())}
        }
        fn read(&mut self,a:u16)->Result<u16,String> {
            self.ops.push(format!("R{a}"));
            if a==0 {
                if let Some(e)=&self.identity_error {return Err(e.clone())}
                return validate_sensor_read(self.reply.as_deref().unwrap_or(&[0x16,0x21,8]));
            }
            Ok(if a==7 {2}else{self.gain})
        }
        fn write(&mut self,a:u16,v:u16)->Result<(),String>{self.ops.push(format!("W{a}:{v}"));Ok(())}
        fn checked(&mut self,a:u16,v:u16)->Result<(),String>{if self.init_error {return Err(SF::WriteStatus.text().into())} if a==0x35{self.gain=v}self.write(a,v)}
        fn settle(&mut self,d:Duration)->Result<(),String>{assert_eq!(d,Duration::from_millis(20));self.ops.push("settle20".into());Ok(())}
        fn progress(&mut self,s:crate::progress::Stage){self.ops.push(format!("stage:{s:?}"));}
        fn pattern(&mut self,i:usize,_:&str,d:Duration)->Result<(),String>{assert_eq!(d,Duration::from_secs(5));self.ops.push(format!("P{i}"));Ok(())}
    }
    impl AcquisitionBackend for Recording {
        fn quiescence_unproved(&self)->bool {quiescence_unproved(&self.streams)}
        fn cleanup_disable(&mut self)->Result<(),String> {
            self.ops.push("disable7/readback".into());
            if self.cleanup_identity_error {self.identity_error=Some(SF::ReadTransport.text().into());}
            if self.cleanup_error {Err(SF::ReadbackMismatch.text().into())} else {Ok(())}
        }
    }
    pub(crate) struct Run { pub result: Result<(),String>, pub session: Session, pub ops: Vec<String> }
    fn run(mut b:Recording, continuous:bool, intent:Intent, caller:CallerPreparation)->Run {
        let settings=Settings{exposure_rows:73,gain_eighths:24,..Default::default()};
        let mut session=Session::default();
        let result=orchestrate(&mut b,settings,continuous,Some(intent),&caller,&mut session.records[4],|b| {
            b.ops.push("LIVE".into());
            let mut stream=Recorder::new(&mut b.streams[3],Binding::new(settings,false));
            stream.data.drain_returned_ok=Some(!b.unproved);
            b.live_error.clone().map_or(Ok(()),Err)
        });
        for (dst,src) in session.records[..4].iter_mut().zip(b.streams) {*dst=src;}
        Run{result,session,ops:b.ops}
    }
    fn unknown(run:&Run) {
        let t=run.session.records[4].as_ref().unwrap();
        assert_eq!(t.cleanup_returned_ok,None);
        for e in [Event::CleanupStart,Event::CleanupEnd] {assert_eq!(t.relative_ns[e as usize],None);}
        assert!(!run.ops.iter().any(|s|s.starts_with('W') || s=="disable7/readback" || s=="LIVE"));
    }
    /// Also used on Windows to project these actual orchestration results/records.
    pub(crate) fn unverified_runs()->Vec<Run> {
        let mut runs=Vec::new();
        for n in 0..4 {
            let r=run(Recording{cancel_at:Some(n),..Default::default()},true,Intent::Normal,Default::default());
            assert_eq!(r.result.as_ref().unwrap_err(),CANCELLED);
            let mut expected:Vec<String>=["B1","B0","B1"][..n].iter().map(|s|s.to_string()).collect();
            if n>0 {expected.push("B1".into())}assert_eq!(r.ops,expected);unknown(&r);runs.push(r);
        }
        for n in 0..3 {
            let r=run(Recording{fail_bridge:Some(n),..Default::default()},true,Intent::Normal,Default::default());
            assert_eq!(r.result.as_ref().unwrap_err(),"@IN preparation transport");
            let mut expected:Vec<String>=["B1","B0","B1"][..=n].iter().map(|s|s.to_string()).collect();expected.push("B1".into());
            assert_eq!(r.ops,expected);unknown(&r);runs.push(r);
        }
        for (reply,error) in [(None,SF::ReadTransport),(Some(vec![]),SF::ReadLength),(Some(vec![0x16,0x21]),SF::ReadLength),(Some(vec![0x16,0x21,8,0]),SF::ReadLength),(Some(vec![0x16,0x21,0]),SF::ReadStatus),(Some(vec![0,0,8]),SF::IdentityMismatch)] {
            let b=Recording{reply,identity_error:if error==SF::ReadTransport{Some(error.text().into())}else{None},fail_bridge:Some(3),..Default::default()};
            let r=run(b,true,Intent::Normal,Default::default());
            assert_eq!(r.result.as_ref().unwrap_err(),error.text());assert_eq!(r.ops,vec!["B1","B0","B1","R0","B1"]);unknown(&r);runs.push(r);
        }
        // Actual finite caller passes attempts/completions and external-read failure.
        for (attempts,completed,external) in [(0,0,false),(1,0,false),(1,1,false),(2,1,false),(3,3,false),(0,0,true),(3,3,true)] {
            let caller=CallerPreparation{bridge_attempts:attempts,bridge_completed:completed,identity_attempted:external,failure:None,..Default::default()};
            let r=run(Recording{cancel_at:Some(0),fail_bridge:Some(0),..Default::default()},false,Intent::Diagnostics,caller);
            assert_eq!(r.result.as_ref().unwrap_err(),CANCELLED);
            assert_eq!(r.ops,if attempts>0||external{vec!["B1"]}else{vec![]});unknown(&r);runs.push(r);
        }
        for (attempts,external,error) in [(1,false,"@IN preparation transport"),(3,true,SF::ReadTransport.text()),(3,true,SF::ReadLength.text()),(3,true,SF::ReadStatus.text()),(3,true,SF::IdentityMismatch.text())] {
            let caller=CallerPreparation{bridge_attempts:attempts,bridge_completed:0,identity_attempted:external,failure:Some(error.into()),..Default::default()};
            let r=run(Recording{fail_bridge:Some(0),..Default::default()},false,Intent::Diagnostics,caller);
            assert_eq!(r.result.as_ref().unwrap_err(),error);assert_eq!(r.ops,vec!["B1"]);unknown(&r);runs.push(r);
        }
        runs
    }
    #[test]
    fn v041_finite_dispatch_state_survives_failure_and_cancellation() {
        for completed in 0..=3 {
            let mut caller=CallerPreparation::default();
            for _ in 0..completed {caller.dispatch_bridge(|| Ok(())).unwrap();}
            assert_eq!(caller.bridge_attempts,completed);assert_eq!(caller.bridge_completed,completed);
            let r=run(Recording{cancel_at:Some(0),..Default::default()},false,Intent::Diagnostics,caller);
            assert_eq!(r.result.as_ref().unwrap_err(),CANCELLED);
            assert_eq!(r.ops,if completed==0{vec![]}else{vec!["B1"]});unknown(&r);
        }
        for completed in 0..3 {
            let mut caller=CallerPreparation::default();
            for _ in 0..completed {caller.dispatch_bridge(|| Ok(())).unwrap();}
            caller.dispatch_bridge(|| Err("bridge fault".into())).unwrap_err();
            assert_eq!(caller.bridge_attempts,completed+1);assert_eq!(caller.bridge_completed,completed);
            let r=run(Recording{fail_bridge:Some(0),..Default::default()},false,Intent::Diagnostics,caller);
            assert_eq!(r.result.as_ref().unwrap_err(),"@IN bridge fault");assert_eq!(r.ops,vec!["B1"]);unknown(&r);
        }
        let mut caller=CallerPreparation::default();
        caller.dispatch_identity(|| Err::<(),_>(SF::ReadStatus.text().into())).unwrap_err();
        assert!(caller.identity_attempted);assert_eq!(caller.bridge_attempts,0);
        let r=run(Recording{fail_bridge:Some(0),..Default::default()},false,Intent::Diagnostics,caller);
        assert_eq!(r.result.as_ref().unwrap_err(),SF::ReadStatus.text());assert_eq!(r.ops,vec!["B1"]);unknown(&r);
        let mut caller=CallerPreparation::default();
        caller.dispatch_identity(|| Ok(())).unwrap();
        caller.stop_attempted=true;
        caller.dispatch_bridge(|| Err("existing STOP fault".into())).unwrap_err();
        let r=run(Recording::default(),false,Intent::Diagnostics,caller);
        assert_eq!(r.result.as_ref().unwrap_err(),"@IN existing STOP fault");assert!(r.ops.is_empty());unknown(&r);
    }
    #[test] fn v041_unverified_orchestration_boundaries_and_first_cause() {assert_eq!(unverified_runs().len(),25);}
    pub(crate) fn verified_runs()->Vec<Run> {
        let mut runs=vec![];
        for continuous in [true,false] {for intent in [Intent::Normal,Intent::Diagnostics] {
            let r=run(Recording::default(),continuous,intent,CallerPreparation{bridge_attempts:3,bridge_completed:3,..Default::default()});
            r.result.as_ref().unwrap();
            let prefix:Vec<&str>=if continuous{vec!["B1","B0","B1","R0"]}else{vec!["R0"]};
            assert_eq!(&r.ops[..prefix.len()],prefix.as_slice());
            assert_eq!(r.ops.iter().filter(|s|s.as_str()=="B0").count(),if continuous{1}else{0});
            let expected_patterns=if !continuous||intent==Intent::Diagnostics{3}else{0};
            assert_eq!(r.ops.iter().filter(|s|s.starts_with('P')).count(),expected_patterns);
            assert!(r.ops.contains(&"W9:73".into())&&r.ops.contains(&"W53:24".into()));
            let live=r.ops.iter().position(|s|s=="LIVE").unwrap();assert_eq!(r.ops[live+1],"R7");
            assert!(r.ops.ends_with(&["B1".into(),"disable7/readback".into(),"R0".into()]));
            let t=r.session.records[4].as_ref().unwrap();assert_eq!(t.cleanup_returned_ok,Some(true));
            assert!(t.relative_ns[Event::CleanupStart as usize].is_some()&&t.relative_ns[Event::CleanupEnd as usize].is_some());runs.push(r);
        }}
        for first in [None,Some(CANCELLED),Some(SF::ReadStatus.text())] {
            let r=run(Recording{cleanup_error:true,live_error:first.map(str::to_string),..Default::default()},true,Intent::Normal,Default::default());
            assert_eq!(r.result.as_ref().unwrap_err(),if first==Some(SF::ReadStatus.text()){SF::ReadStatus.text()}else{SF::ReadbackMismatch.text()});
            let t=r.session.records[4].as_ref().unwrap();assert_eq!(t.cleanup_returned_ok,Some(false));assert!(t.relative_ns[Event::CleanupStart as usize].is_some());assert_eq!(t.relative_ns[Event::CleanupEnd as usize],None);
            assert!(r.ops.ends_with(&["B1".into(),"disable7/readback".into()]));runs.push(r);
        }
        let r=run(Recording{unproved:true,live_error:Some(QUIESCENCE_FAILED.into()),..Default::default()},true,Intent::Normal,Default::default());
        assert!(r.ops.ends_with(&["LIVE".into()]));assert_eq!(r.result.as_ref().unwrap_err(),"@ST Transfer quiescence unproved; evidence review and physical recovery required");
        let t=r.session.records[4].as_ref().unwrap();assert_eq!(t.cleanup_returned_ok,None);assert_eq!(t.relative_ns[Event::CleanupEnd as usize],None);runs.push(r);
        runs
    }
    #[test]
    fn v041_verified_cleanup_each_mandatory_failure_and_init_first_cause() {
        for (bridge,disable,identity,error) in [(Some(3),false,false,"@IN preparation transport"),(None,true,false,SF::ReadbackMismatch.text()),(None,false,true,SF::ReadTransport.text())] {
            let r=run(Recording{fail_bridge:bridge,cleanup_error:disable,cleanup_identity_error:identity,..Default::default()},true,Intent::Normal,Default::default());
            assert_eq!(r.result.as_ref().unwrap_err(),error);
            let t=r.session.records[4].as_ref().unwrap();assert_eq!(t.cleanup_returned_ok,Some(false));assert_eq!(t.relative_ns[Event::CleanupEnd as usize],None);
        }
        let r=run(Recording{init_error:true,cleanup_error:true,..Default::default()},true,Intent::Normal,Default::default());
        assert_eq!(r.result.as_ref().unwrap_err(),SF::WriteStatus.text());assert!(!r.ops.contains(&"LIVE".into()));
        assert_eq!(r.session.records[4].as_ref().unwrap().cleanup_returned_ok,Some(false));
    }
    #[test]
    fn v041_live_cancellation_preserved_after_full_cleanup() {
        let r=run(Recording{live_error:Some(CANCELLED.into()),..Default::default()},true,Intent::Normal,Default::default());
        assert_eq!(r.result.as_ref().unwrap_err(),CANCELLED);
        assert_eq!(r.session.records[4].as_ref().unwrap().cleanup_returned_ok,Some(true));
        assert!(r.ops.ends_with(&["B1".into(),"disable7/readback".into(),"R0".into()]));
    }
    #[test] fn v041_verified_orchestration_live_cleanup_and_quiescence() {assert_eq!(verified_runs().len(),8);}
}
