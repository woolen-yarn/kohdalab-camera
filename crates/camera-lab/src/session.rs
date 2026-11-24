//! Single-owner lifecycle. Only the worker calls transport/process/file APIs.
#[cfg(test)]
use crate::physical_preview::PhysicalPreview;
use crate::{CameraAdapter, Frame, Simulation, controls::Settings};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub enum Source {
    Physical(nusb::DeviceInfo),
    Simulation,
    #[cfg(all(test, not(windows)))]
    HostFake,
    /// Explicit fake path: never enumerates devices or selects user_root.
    #[cfg(windows)]
    Fixture {
        root: std::path::PathBuf,
        fault: crate::continuous_session::Fixture,
    },
}
#[derive(Clone)]
pub struct Request {
    pub startup_intent: crate::startup::Intent,
    pub source: Source,
    pub settings: Settings,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Disconnected,
    Connecting,
    Streaming,
    SettingsPending,
    Stopping,
    Error,
}
#[derive(Clone, Debug, Default)]
pub struct Lifecycle {
    pub starting: bool,
    pub phase: u8,
    pub stop_observed: bool,
    pub release_committed: bool,
    pub marker_deleted: bool,
    pub collected: bool,
    pub exit: Option<u32>,
    pub proof_flags: u8,
    pub unproved: bool,
    pub helper_fault: Option<crate::continuous::Fault>,
    pub backend_error: Option<String>,
    pub parent_error: Option<String>,
    pub terminal_kind: Option<crate::diagnostic::Terminal>,
    pub first_cause: Option<crate::diagnostic::Cause>,
    pub backend_cause: Option<crate::diagnostic::Cause>,
}
#[derive(Clone, Copy, Debug)]
pub enum FailureCode {
    Backend,
    Parent,
    Unknown,
    StopRequest,
    FrameContract,
    FrameValidation,
    CleanupUnproved,
    Supervisor,
    TransportPoll,
    CleanupDeadline,
    OpenGeneration,
    GuiFailure,
    WorkerPanic,
}
#[derive(Clone)]
pub struct Snapshot {
    pub owner_revision: Option<u64>,
    pub owner_settings: Option<Settings>,
    pub owner_startup_intent: Option<crate::startup::Intent>,
    pub owner_settings_digest: Option<String>,
    pub settings_digest: Option<String>,
    pub startup_intent: Option<crate::startup::Intent>,
    pub first_failure_code: Option<FailureCode>,
    pub first_cause: Option<crate::diagnostic::Cause>,
    pub first_failure_original_bytes: Option<usize>,
    pub first_failure: Option<String>,
    pub persistence: String,
    pub diagnostic_invalid: bool,
    /// Optional immutable diagnostics for this exact active request revision.
    pub diagnostics: Option<(u64, crate::acceptance_summary::Diagnostic)>,
    pub state: State,
    pub lifecycle: Option<Lifecycle>,
    pub restart_disabled: bool,
    pub message: String,
    pub applied: Option<Settings>,
    pub requested: Option<Settings>,
    pub requested_revision: Option<u64>,
    pub applied_revision: Option<u64>,
    pub progress: Option<crate::progress::Stage>,
    pub started: Option<Instant>,
    pub stage_started: Option<Instant>,
    pub active: bool,
    pub frame: Option<(u64, Frame)>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            owner_revision: None,
            owner_settings: None,
            owner_startup_intent: None,
            owner_settings_digest: None,
            settings_digest: None,
            startup_intent: None,
            first_failure_code: None,
            first_cause: None,
            first_failure_original_bytes: None,
            first_failure: None,
            persistence: "No failure recorded".into(),
            state: State::Disconnected,
            diagnostics: None,
            diagnostic_invalid: false,
            lifecycle: None,
            restart_disabled: false,
            message: "Disconnected".into(),
            applied: None,
            requested: None,
            requested_revision: None,
            applied_revision: None,
            progress: None,
            started: None,
            stage_started: None,
            active: false,
            frame: None,
        }
    }
}
/// Done is reported only after the process exits and its handle has been reaped.
pub enum Event {
    Waiting,
    Lifecycle { message: String, unproved: bool },
    Progress(crate::progress::Stage),
    Frame(Frame),
    Done { clean: bool },
}
pub(crate) fn continuous_failure_message(error: &str, label: &str) -> String {
    format!(
        "Continuous acquisition failed: {error}. Ownership retained until collection. Evidence: {label}"
    )
}
pub trait Transport: Send {
    fn set_frame_notify(&self, _t: std::thread::Thread) {}
    fn diagnostic_invalid(&self) -> bool {
        false
    }
    fn diagnostics(&self) -> Option<crate::acceptance_summary::Diagnostic> {
        None
    }
    fn lifecycle(&self) -> Option<Lifecycle> {
        None
    }
    fn poll(&mut self) -> Result<Event, String>;
    fn stop(&mut self) -> Result<(), String>;
}
pub trait Factory: Send {
    fn settings_digest(&mut self, settings: Settings) -> Option<String> {
        #[cfg(windows)]
        {
            settings_digest_with(settings, |bytes| {
                capability_platform::digest(bytes).map_err(|_| ())
            })
        }
        #[cfg(not(windows))]
        {
            let _ = settings;
            None
        }
    }
    fn open(&mut self, request: &Request) -> Result<Box<dyn Transport>, String>;
    fn open_generation(
        &mut self,
        request: &Request,
        _generation: u64,
    ) -> Result<Box<dyn Transport>, String> {
        self.open(request)
    }
}
#[cfg(any(windows, test))]
fn settings_digest_with(
    settings: Settings,
    hash: impl FnOnce(&[u8]) -> Result<[u8; 32], ()>,
) -> Option<String> {
    hash(
        format!(
            "settings-v2-geometry1:{}:{}:{}",
            settings.mode.profile().id,
            settings.exposure_rows,
            settings.gain_eighths
        )
        .as_bytes(),
    )
    .ok()
    .map(|v| v.iter().map(|b| format!("{b:02x}")).collect())
}
struct Active {
    id: u64,
    request: Request,
    transport: Box<dyn Transport>,
    stopping: bool,
    stop_at: Option<Instant>,
}
pub struct Engine<F: Factory> {
    factory: F,
    desired: Option<(u64, Request)>,
    active: Option<Active>,
    pub snapshot: Snapshot,
    blocked: bool,
}
impl<F: Factory> Engine<F> {
    pub fn new(factory: F) -> Self {
        Self {
            factory,
            desired: None,
            active: None,
            snapshot: Snapshot::default(),
            blocked: false,
        }
    }
    pub fn request(&mut self, id: u64, request: Option<Request>) {
        self.request_at(id, request, Instant::now());
    }
    fn request_at(&mut self, id: u64, request: Option<Request>, now: Instant) {
        self.snapshot.frame = None;
        if self.blocked {
            return;
        }
        self.snapshot.settings_digest = None;
        self.snapshot.startup_intent = None;
        if let Some(r) = request.as_ref() {
            self.snapshot.startup_intent = Some(r.startup_intent);
            self.snapshot.settings_digest = self.factory.settings_digest(r.settings);
        }
        self.snapshot.requested = request.as_ref().map(|r| r.settings);
        self.snapshot.requested_revision = request.as_ref().map(|_| id);
        self.snapshot.progress = None;
        self.snapshot.started = Some(now);
        self.snapshot.stage_started = None;
        self.desired = request.map(|r| (id, r));
        self.snapshot.state = if self.active.is_some() {
            if self.desired.is_some() {
                State::SettingsPending
            } else {
                State::Stopping
            }
        } else if self.desired.is_some() {
            State::Connecting
        } else {
            State::Disconnected
        };
        self.snapshot.message = format!(
            "{:?}; latest valid image remains available for capture",
            self.snapshot.state
        );
    }
    fn fail(&mut self, code: FailureCode, message: String) {
        if self.snapshot.first_failure.is_none() {
            self.snapshot.first_cause = Some(
                self.snapshot
                    .lifecycle
                    .as_ref()
                    .and_then(|l| l.first_cause)
                    .unwrap_or(crate::diagnostic::Cause::unknown()),
            );
            self.snapshot.first_failure_code = Some(code);
            self.snapshot.first_failure_original_bytes = Some(message.len());
            self.snapshot.first_failure = Some(crate::failure_record::bounded(&message));
        }
        self.desired = None;
        self.snapshot.state = State::Error;
        self.snapshot.message = message;
    }
    pub fn accepts(&self, id: u64) -> bool {
        self.active
            .as_ref()
            .is_some_and(|a| a.id == id && !a.stopping)
            && self
                .desired
                .as_ref()
                .is_some_and(|(wanted, _)| *wanted == id)
    }
    pub fn tick(&mut self) {
        self.tick_at(Instant::now());
    }
    fn tick_at(&mut self, now: Instant) {
        if let Some(a) = self.active.as_mut() {
            let changed = self.desired.as_ref().is_none_or(|(id, _)| *id != a.id);
            if changed && !a.stopping {
                a.stopping = true;
                a.stop_at = Some(now);
                if let Err(e) = a.transport.stop() {
                    self.fail(
                        FailureCode::StopRequest,
                        format!("Stop request failed: {e}"),
                    );
                }
            }
            let result = self.active.as_mut().unwrap().transport.poll();
            self.snapshot.lifecycle = self.active.as_ref().unwrap().transport.lifecycle();
            self.snapshot.diagnostic_invalid =
                self.active.as_ref().unwrap().transport.diagnostic_invalid();
            if let Some(d) = self.active.as_ref().unwrap().transport.diagnostics() {
                self.snapshot.diagnostics = Some((self.active.as_ref().unwrap().id, d));
            }
            match result {
                Ok(Event::Frame(frame)) => {
                    let a = self.active.as_ref().unwrap();
                    if self.accepts(a.id) {
                        match frame.validate() {
                            Ok(()) => {
                                self.snapshot.state = State::Streaming;
                                let physical = !frame.metadata.simulated;
                                self.snapshot.applied = physical.then_some(a.request.settings);
                                self.snapshot.applied_revision = physical.then_some(a.id);
                                if physical
                                    && self
                                        .snapshot
                                        .progress
                                        .is_none_or(|p| p < crate::progress::Stage::Live)
                                {
                                    self.snapshot.progress = Some(crate::progress::Stage::Live);
                                    self.snapshot.stage_started = Some(now);
                                }
                                self.snapshot.message = if physical {
                                    "Live continuous session; checked settings applied"
                                } else {
                                    "Explicit simulation; fixed synthetic fixture, no hardware checks"
                                }.into();
                                let mut frame = frame;
                                if let Some(contract) = &mut frame.metadata.acquisition {
                                    if contract.settings().ok() != Some(a.request.settings)
                                        || contract.settings_generation != a.id
                                        || contract.startup_intent != Some(a.request.startup_intent)
                                    {
                                        self.fail(
                                            FailureCode::FrameContract,
                                            "Frame settings contract mismatch".into(),
                                        );
                                        return;
                                    }
                                    contract.request_revision = Some(a.id);
                                } else if physical {
                                    self.fail(
                                        FailureCode::FrameContract,
                                        "Current physical frame lacks typed acquisition contract"
                                            .into(),
                                    );
                                    return;
                                }
                                frame.metadata.source = format!(
                                    "{}; revision={}; profile={}; exposure_rows={}; gain_eighths={}",
                                    frame.metadata.source,
                                    a.id,
                                    a.request.settings.mode.profile().id,
                                    a.request.settings.exposure_rows,
                                    a.request.settings.gain_eighths
                                );
                                self.snapshot.frame = Some((a.id, frame));
                            }
                            Err(e) => self.fail(FailureCode::FrameValidation, e),
                        }
                    }
                }
                Ok(Event::Done { clean }) => {
                    let a = self.active.take().unwrap();
                    self.snapshot.active = false;
                    self.snapshot.applied = None;
                    self.snapshot.applied_revision = None;
                    if !clean {
                        self.blocked = true;
                        self.snapshot.restart_disabled = true;
                        self.fail(
                            FailureCode::CleanupUnproved,
                            format!(
                                "Cleanup unproved; restart disabled. {}",
                                self.snapshot.message
                            ),
                        );
                    } else if self.desired.as_ref().is_some_and(|(id, _)| *id == a.id) {
                        self.desired = None; // Bounded natural end never reconnects automatically.
                        self.snapshot.state = State::Disconnected;
                        self.snapshot.message =
                            if matches!(a.request.source, Source::Physical(_)) {
                                "Continuous session collected; verified cleanup; retained image is historical"
                            } else { "Simulation ended; retained image is historical" }.into();
                    } else if self.snapshot.state != State::Error {
                        self.snapshot.message = if self.desired.is_some() { "Previous session cancelled with verified cleanup; latest request pending" } else { "Session cancelled with verified cleanup; retained image is historical" }.into();
                        self.snapshot.state = if self.desired.is_some() {
                            State::SettingsPending
                        } else {
                            State::Disconnected
                        };
                    }
                }
                Ok(Event::Progress(stage)) => {
                    let a = self.active.as_ref().unwrap();
                    if self.accepts(a.id) && self.snapshot.progress.is_none_or(|old| stage >= old) {
                        if self.snapshot.progress != Some(stage) {
                            self.snapshot.stage_started = Some(now);
                        }
                        self.snapshot.progress = Some(stage);
                    }
                }
                Ok(Event::Lifecycle { message, unproved }) => {
                    if unproved {
                        self.blocked = true;
                        self.snapshot.restart_disabled = true;
                        let code = match self
                            .snapshot
                            .lifecycle
                            .as_ref()
                            .and_then(|l| l.first_cause)
                            .map(|c| c.origin)
                        {
                            Some(crate::diagnostic::Origin::Backend) => FailureCode::Backend,
                            Some(crate::diagnostic::Origin::Parent) => FailureCode::Parent,
                            Some(crate::diagnostic::Origin::Helper) => FailureCode::Supervisor,
                            _ => FailureCode::Unknown,
                        };
                        self.fail(code, message);
                    } else if self.snapshot.state != State::Error {
                        self.snapshot.message = message;
                    }
                }
                Ok(Event::Waiting) => {}
                Err(e) => self.fail(FailureCode::TransportPoll, e), // Keep ownership; next tick requests stop and reaps.
            }
            if self
                .active
                .as_ref()
                .and_then(|a| a.stop_at)
                .is_some_and(|t| now.saturating_duration_since(t) >= Duration::from_secs(65))
            {
                self.blocked = true;
                self.snapshot.restart_disabled = true;
                self.fail(
                    FailureCode::CleanupDeadline,
                    "Cleanup deadline exceeded; ownership retained, no replacement allowed".into(),
                );
            }
            return; // Never open a replacement in the same completion tick.
        }
        if self.blocked {
            return;
        }
        if let Some((id, request)) = self.desired.clone() {
            self.snapshot.diagnostics = None;
            self.snapshot.diagnostic_invalid = false;
            self.snapshot.lifecycle = None;
            self.snapshot.state = State::Connecting;
            self.snapshot.message =
                "Starting continuous owner; preparation and checked live data pending".into();
            self.snapshot.owner_revision = Some(id);
            self.snapshot.owner_settings = Some(request.settings);
            self.snapshot.owner_startup_intent = Some(request.startup_intent);
            self.snapshot.owner_settings_digest = self.snapshot.settings_digest.clone();
            match self.factory.open_generation(&request, id) {
                Ok(transport) => {
                    transport.set_frame_notify(std::thread::current());
                    self.active = Some(Active {
                        id,
                        request,
                        transport,
                        stopping: false,
                        stop_at: None,
                    });
                    self.snapshot.active = true;
                }
                Err(e) => self.fail(FailureCode::OpenGeneration, e),
            }
        }
    }
}

/// The acquisition Worker, not the UI/render thread, renews the backend owner
/// lease. Modal dialogs and export never participate. A Worker stall therefore
/// conservatively faults the actual session, while a paused painter does not.
#[cfg(windows)]
struct ContinuousTransport {
    session: crate::continuous_session::ContinuousSession,
    sequence: u64,
    phase: u8,
    error_reported: bool,
    label: String,
    lifecycle: Lifecycle,
}
#[cfg(windows)]
impl ContinuousTransport {
    fn new(session: crate::continuous_session::ContinuousSession, label: String) -> Self {
        Self {
            session,
            sequence: 0,
            phase: 0,
            error_reported: false,
            label,
            lifecycle: Lifecycle::default(),
        }
    }
}
#[cfg(windows)]
impl Transport for ContinuousTransport {
    fn set_frame_notify(&self, t: std::thread::Thread) {
        self.session.set_frame_notify(t);
    }
    fn diagnostic_invalid(&self) -> bool {
        self.session.poll().diagnostic_invalid
    }
    fn diagnostics(&self) -> Option<crate::acceptance_summary::Diagnostic> {
        self.session.poll().diagnostics
    }
    fn lifecycle(&self) -> Option<Lifecycle> {
        Some(self.lifecycle.clone())
    }
    fn stop(&mut self) -> Result<(), String> {
        self.session.stop();
        Ok(())
    }
    fn poll(&mut self) -> Result<Event, String> {
        let s = self.session.poll();
        let facts = crate::diagnostic::Facts {
            starting: s.starting,
            phase: s.helper_phase,
            stop_observed: s.stop_observed,
            release_committed: s.release_committed,
            marker_deleted: s.marker_deleted,
            collected: s.collected,
            exit: s.exit,
            proof_flags: s.proof_flags,
            helper_fault: s.helper_fault,
            backend_error: s.backend_error.clone(),
            parent_error: s.first_error.clone(),
            terminal: s.terminal,
        };
        self.lifecycle = crate::diagnostic::lifecycle(&facts, &s.causes);
        if let Some(event) = crate::diagnostic::transport_event(
            &self.lifecycle,
            &mut self.error_reported,
            &self.label,
        ) {
            return Ok(event);
        }
        if let Some(mut f) = s.frame
            && f.metadata.sequence > self.sequence
        {
            self.sequence = f.metadata.sequence;
            f.metadata.source = format!("{}; {}", f.metadata.source, self.label);
            return Ok(Event::Frame(f));
        }
        if s.helper_phase > self.phase {
            self.phase = s.helper_phase;
            return Ok(Event::Progress(match self.phase {
                1 => crate::progress::Stage::Identity,
                2 => crate::progress::Stage::Configuration,
                3 => crate::progress::Stage::Live,
                _ => {
                    return Ok(Event::Lifecycle {
                        message:
                            "STOP observed; collecting exact helper, exit and lock release pending"
                                .into(),
                        unproved: false,
                    });
                }
            }));
        }
        Ok(Event::Waiting)
    }
}

struct RealFactory;
#[cfg(test)]
struct Physical {
    preview: PhysicalPreview,
    error: Option<String>,
    progress: Option<crate::progress::Stage>,
}
#[cfg(test)]
impl Transport for Physical {
    fn stop(&mut self) -> Result<(), String> {
        self.preview.stop()
    }
    fn poll(&mut self) -> Result<Event, String> {
        // Even malformed IPC must not abandon a running child.
        if self.error.is_none() {
            match self.preview.poll() {
                Ok((Some(frame), _)) => return Ok(Event::Frame(frame)),
                Ok(_) => {}
                Err(e) => {
                    self.error = Some(e.clone());
                    return Err(e);
                }
            }
        }
        match self.preview.completion()? {
            Some(clean) => Ok(Event::Done { clean }),
            None => {
                let next = self.preview.progress();
                if let Some(stage) = next
                    && self.progress != next
                {
                    self.progress = next;
                    Ok(Event::Progress(stage))
                } else {
                    Ok(Event::Waiting)
                }
            }
        }
    }
}
struct Sim {
    simulation: Simulation,
    stopped: bool,
}
impl Transport for Sim {
    fn stop(&mut self) -> Result<(), String> {
        self.stopped = true;
        Ok(())
    }
    fn poll(&mut self) -> Result<Event, String> {
        if self.stopped {
            Ok(Event::Done { clean: true })
        } else {
            self.simulation.next_frame().map(Event::Frame)
        }
    }
}
impl Factory for RealFactory {
    fn open(&mut self, request: &Request) -> Result<Box<dyn Transport>, String> {
        self.open_generation(request, 1)
    }
    fn open_generation(
        &mut self,
        request: &Request,
        generation: u64,
    ) -> Result<Box<dyn Transport>, String> {
        request.settings.validate()?;
        match &request.source {
            Source::Physical(info) => {
                if let Some(error) = crate::driver_policy::connection_error(info) {
                    return Err(error.into());
                }
                #[cfg(windows)]
                {
                    let identity = crate::continuous_session::Identity::from_device(info)
                        .map_err(|e| e.to_string())?;
                    let label = format!(
                        "bus={} ports={:?}; evidence={}",
                        identity.bus,
                        identity.ports,
                        crate::coordination::user_root()?.display()
                    );
                    Ok(Box::new(ContinuousTransport::new(
                        crate::continuous_session::ContinuousSession::start_generation_intent(
                            identity,
                            request.settings,
                            generation,
                            request.startup_intent,
                        )
                        .map_err(|e| e.to_string())?,
                        label,
                    )))
                }
                #[cfg(not(windows))]
                {
                    let _ = info;
                    Err(
                        "Unsupported physical transport: continuous acquisition requires Windows"
                            .into(),
                    )
                }
            }
            #[cfg(windows)]
            Source::Fixture { root, fault } => Ok(Box::new(ContinuousTransport::new(
                crate::continuous_session::ContinuousSession::start_fixture_intent(
                    root,
                    *fault,
                    request.settings,
                    generation,
                    request.startup_intent,
                )
                .map_err(|e| e.to_string())?,
                format!("SIMULATED fixture; evidence={}", root.display()),
            ))),
            #[cfg(all(test, not(windows)))]
            Source::HostFake => Err("host FAKE requires injected factory".into()),
            Source::Simulation => Ok(Box::new(Sim {
                simulation: Simulation { sequence: 0 },
                stopped: false,
            })),
        }
    }
}
struct Shared {
    wake_gui: Option<Arc<dyn Fn() + Send + Sync>>,
    revision: u64,
    requested_at: Instant,
    desired: Option<Request>,
    closing: bool,
    closed: bool,
    gui_failed: bool,
    snapshot: Snapshot,
}
pub struct Worker {
    shared: Arc<Mutex<Shared>>,
    completion: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}
/// Retain outside App before GUI construction; wait only after GUI teardown.
/// Abort, OOM and external termination cannot execute this barrier.
pub struct LifecycleGuardian {
    shared: Arc<Mutex<Shared>>,
    completion: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}
impl LifecycleGuardian {
    pub fn gui_failed(&self) {
        let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        s.gui_failed = true;
        s.snapshot.restart_disabled = true;
        s.snapshot.state = State::Error;
        s.snapshot.message = "GUI failed; restart disabled; actual collection required".into();
    }
    pub fn wait(&mut self) -> Snapshot {
        let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        s.closing = true;
        s.desired = None;
        s.revision += 1;
        drop(s);
        if let Some(thread) = self
            .completion
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = thread.join();
            // Unexpected worker death is never collection proof.
            while !self.shared.lock().unwrap_or_else(|e| e.into_inner()).closed {
                std::thread::park_timeout(Duration::from_secs(1));
            }
        }
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .clone()
    }
}
impl Drop for LifecycleGuardian {
    fn drop(&mut self) {
        self.wait();
    }
}
impl Default for Worker {
    fn default() -> Self {
        Self::new()
    }
}
fn publish_failed(shared: &mut Shared, failed: &Snapshot, seen: u64) {
    let old = shared.snapshot.frame.take();
    let mut snapshot = failed.clone();
    if shared.revision != seen {
        snapshot.frame = None;
        snapshot.requested = shared.snapshot.requested;
        snapshot.requested_revision = shared.snapshot.requested_revision;
        snapshot.settings_digest = shared.snapshot.settings_digest.clone();
        snapshot.startup_intent = shared.snapshot.startup_intent;
    } else if snapshot.frame.is_none() {
        snapshot.frame = old;
    }
    shared.snapshot = snapshot;
}
impl Worker {
    pub fn new() -> Self {
        Self::with_factory(RealFactory)
    }
    pub fn with_factory(factory: impl Factory + 'static) -> Self {
        Self::with_factory_recorder(factory, crate::failure_record::Recorder::new())
    }
    pub fn with_factory_recorder(
        factory: impl Factory + 'static,
        mut recorder: crate::failure_record::Recorder,
    ) -> Self {
        let shared = Arc::new(Mutex::new(Shared {
            wake_gui: None,
            revision: 0,
            requested_at: Instant::now(),
            desired: None,
            closing: false,
            closed: false,
            gui_failed: false,
            snapshot: Snapshot::default(),
        }));
        let state = shared.clone();
        let thread = std::thread::spawn(move || {
            let mut engine = Engine::new(factory);
            let mut seen = 0;
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                loop {
                    let (revision, desired, closing, requested_at, gui_failed) = {
                        let s = state.lock().unwrap();
                        (
                            s.revision,
                            s.desired.clone(),
                            s.closing,
                            s.requested_at,
                            s.gui_failed,
                        )
                    };
                    if revision != seen {
                        engine.request_at(revision, desired, requested_at);
                        seen = revision;
                    }
                    if gui_failed {
                        engine.blocked = true;
                        engine.desired = None;
                        engine.snapshot.restart_disabled = true;
                    }
                    engine.tick();
                    if gui_failed {
                        engine.fail(
                            FailureCode::GuiFailure,
                            "GUI failed; restart disabled; actual collection required".into(),
                        );
                        engine.snapshot.state = State::Error;
                        engine.snapshot.message =
                            "GUI failed; restart disabled; actual collection required".into();
                    }
                    recorder.observe(&mut engine.snapshot);
                    let done = closing && !engine.snapshot.active;
                    if done {
                        recorder.finish(&mut engine.snapshot);
                    }
                    let mut s = state.lock().unwrap();
                    // A newer UI request invalidates frames produced during this tick.
                    if s.revision == seen {
                        let old = s.snapshot.frame.take();
                        s.snapshot = engine.snapshot.clone();
                        if s.snapshot.frame.is_none() {
                            s.snapshot.frame = old;
                        }
                    }
                    // GUI failure may arrive during this tick. Publish it under
                    // the same lock so an older snapshot cannot clear failure.
                    if s.gui_failed {
                        s.snapshot.restart_disabled = true;
                        s.snapshot.state = State::Error;
                        s.snapshot.message =
                            "GUI failed; restart disabled; actual collection required".into();
                    }
                    let wake = if engine.snapshot.frame.is_some() {
                        s.wake_gui.clone()
                    } else {
                        None
                    };
                    engine.snapshot.frame = None;
                    s.closed = done;
                    drop(s);
                    if let Some(wake) = wake {
                        wake();
                    }
                    if done {
                        break;
                    }
                    std::thread::park_timeout(Duration::from_millis(5));
                }
            }));
            if outcome.is_err() {
                engine.fail(FailureCode::WorkerPanic, "Acquisition worker panicked; cleanup unproved; restart requires evidence review".into());
                recorder.observe(&mut engine.snapshot);
                engine.blocked = true;
                engine.snapshot.restart_disabled = true;
                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                s.closed = false;
                publish_failed(&mut s, &engine.snapshot, seen);
                s.desired = None;
                s.closing = true;
                drop(s);
                engine.desired = None;
                while engine.snapshot.active {
                    // Retain the active owner even if recovery polling panics.
                    // Repeated external panics may prevent progress indefinitely.
                    let _ =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| engine.tick()));
                    engine.snapshot.state = State::Error;
                    engine.snapshot.message =
                        "Worker panicked; restart disabled; ownership retained until collection"
                            .into();
                    recorder.observe(&mut engine.snapshot);
                    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                    publish_failed(&mut s, &engine.snapshot, seen);
                    s.closed = false;
                    drop(s);
                    std::thread::sleep(Duration::from_millis(33));
                }
                recorder.finish(&mut engine.snapshot);
                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                publish_failed(&mut s, &engine.snapshot, seen);
                s.closed = true;
            }
        });
        Self {
            shared,
            completion: Arc::new(Mutex::new(Some(thread))),
        }
    }
    pub fn guardian(&self) -> LifecycleGuardian {
        LifecycleGuardian {
            shared: self.shared.clone(),
            completion: self.completion.clone(),
        }
    }
    pub fn request(&self, request: Option<Request>) -> Result<u64, String> {
        self.request_at(request, Instant::now())
    }
    fn request_at(&self, request: Option<Request>, now: Instant) -> Result<u64, String> {
        if let Some(r) = &request {
            r.settings.validate()?;
        }
        let mut s = self.shared.lock().unwrap();
        if s.closing || s.snapshot.restart_disabled {
            return Err("Session restart disabled; inspect cleanup evidence".into());
        }
        s.revision += 1;
        s.requested_at = now;
        s.snapshot.settings_digest = None;
        s.snapshot.startup_intent = request.as_ref().map(|r| r.startup_intent);
        s.snapshot.requested = request.as_ref().map(|r| r.settings);
        s.snapshot.requested_revision = request.as_ref().map(|_| s.revision);
        s.snapshot.progress = None;
        s.snapshot.stage_started = None;
        s.snapshot.started = Some(now);
        s.desired = request;
        s.snapshot.frame = None;
        s.snapshot.state = if s.snapshot.active {
            if s.desired.is_some() {
                State::SettingsPending
            } else {
                State::Stopping
            }
        } else {
            State::Connecting
        };
        Ok(s.revision)
    }
    pub fn snapshot(&self) -> Snapshot {
        let mut s = self.shared.lock().unwrap();
        let mut result = s.snapshot.clone();
        result.frame = s.snapshot.frame.take();
        result
    }
    /// egui coalesces repeated repaint requests; no GUI image work runs here.
    pub fn set_wake(&self, wake: impl Fn() + Send + Sync + 'static) {
        self.shared.lock().unwrap().wake_gui = Some(Arc::new(wake));
    }
    pub fn close(&self) {
        self.close_at(Instant::now());
    }
    fn close_at(&self, now: Instant) {
        let mut s = self.shared.lock().unwrap();
        if !s.closing {
            s.revision += 1;
            s.requested_at = now;
            s.desired = None;
            s.closing = true;
            s.snapshot.frame = None;
            s.snapshot.requested = None;
            s.snapshot.requested_revision = None;
            s.snapshot.progress = None;
            s.snapshot.stage_started = None;
            s.snapshot.started = Some(now);
            s.snapshot.state = State::Stopping;
            s.snapshot.message = "Closing; cancellation requested; cleanup not yet proved".into();
        }
    }
    pub fn closed(&self) -> bool {
        self.shared.lock().unwrap().closed
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.close();
    }
}

// Test modules live in sibling files; each keeps its original `session::<name>` path.
#[cfg(all(test, windows))]
#[path = "session_tests/continuous_worker_tests.rs"]
mod continuous_worker_tests;

#[cfg(test)]
#[path = "session_tests/tests.rs"]
mod tests;

#[cfg(all(test, not(windows)))]
#[path = "session_tests/v034_tests.rs"]
mod v034_tests;

#[cfg(all(test, not(windows)))]
#[path = "session_tests/v034_binding_test.rs"]
mod v034_binding_test;

#[cfg(test)]
#[path = "session_tests/v035_tests.rs"]
mod v035_tests;

#[cfg(test)]
#[path = "session_tests/v036_tests.rs"]
mod v036_tests;

#[cfg(test)]
#[path = "session_tests/v037_tests.rs"]
mod v037_tests;
