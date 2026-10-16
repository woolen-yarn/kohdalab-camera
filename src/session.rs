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
    #[cfg(all(test,not(windows)))]
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
pub enum FailureCode { Backend, Parent, Unknown, StopRequest, FrameContract, FrameValidation, CleanupUnproved, Supervisor, TransportPoll, CleanupDeadline, OpenGeneration, GuiFailure, WorkerPanic }
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
pub(crate) fn continuous_failure_message(error:&str,label:&str)->String {
    format!("Continuous acquisition failed: {error}. Ownership retained until collection. Evidence: {label}")
}
pub trait Transport: Send {
    fn set_frame_notify(&self, _t:std::thread::Thread) {}
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
    fn settings_digest(&mut self, settings:Settings)->Option<String> {
        #[cfg(windows)] {settings_digest_with(settings, |bytes|capability_platform::digest(bytes).map_err(|_|()))}
        #[cfg(not(windows))] {let _=settings;None}
    }
    fn open(&mut self, request: &Request) -> Result<Box<dyn Transport>, String>;
    fn open_generation(&mut self, request: &Request, _generation:u64) -> Result<Box<dyn Transport>,String> {
        self.open(request)
    }
}
#[cfg(any(windows,test))]
fn settings_digest_with(settings:Settings, hash:impl FnOnce(&[u8])->Result<[u8;32],()>)->Option<String> {
    hash(format!("settings-v2-geometry1:{}:{}:{}",settings.mode.profile().id,settings.exposure_rows,settings.gain_eighths).as_bytes()).ok().map(|v|v.iter().map(|b|format!("{b:02x}")).collect())
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
        self.snapshot.settings_digest=None;
        self.snapshot.startup_intent=None;
        if let Some(r)=request.as_ref() {
            self.snapshot.startup_intent=Some(r.startup_intent);
            self.snapshot.settings_digest=self.factory.settings_digest(r.settings);
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
        if self.snapshot.first_failure.is_none() { self.snapshot.first_cause=Some(self.snapshot.lifecycle.as_ref().and_then(|l|l.first_cause).unwrap_or(crate::diagnostic::Cause::unknown())); self.snapshot.first_failure_code=Some(code); self.snapshot.first_failure_original_bytes=Some(message.len()); self.snapshot.first_failure = Some(crate::failure_record::bounded(&message)); }
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
                    self.fail(FailureCode::StopRequest, format!("Stop request failed: {e}"));
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
                                    if contract.settings().ok() != Some(a.request.settings) || contract.settings_generation != a.id || contract.startup_intent != Some(a.request.startup_intent) {
                                        self.fail(FailureCode::FrameContract, "Frame settings contract mismatch".into());
                                        return;
                                    }
                                    contract.request_revision = Some(a.id);
                                } else if physical {
                                    self.fail(FailureCode::FrameContract, "Current physical frame lacks typed acquisition contract".into());
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
                        self.fail(FailureCode::CleanupUnproved, format!(
                            "Cleanup unproved; restart disabled. {}",
                            self.snapshot.message
                        ));
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
                        let code=match self.snapshot.lifecycle.as_ref().and_then(|l|l.first_cause).map(|c|c.origin) {Some(crate::diagnostic::Origin::Backend)=>FailureCode::Backend,Some(crate::diagnostic::Origin::Parent)=>FailureCode::Parent,Some(crate::diagnostic::Origin::Helper)=>FailureCode::Supervisor,_=>FailureCode::Unknown};
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
                self.fail(FailureCode::CleanupDeadline, 
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
            self.snapshot.owner_revision=Some(id);
            self.snapshot.owner_settings=Some(request.settings);
            self.snapshot.owner_startup_intent=Some(request.startup_intent);
            self.snapshot.owner_settings_digest=self.snapshot.settings_digest.clone();
            match self.factory.open_generation(&request,id) {
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
    fn set_frame_notify(&self,t:std::thread::Thread) {self.session.set_frame_notify(t);}
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
        self.lifecycle=crate::diagnostic::lifecycle(&facts, &s.causes);
        if let Some(event)=crate::diagnostic::transport_event(&self.lifecycle,&mut self.error_reported,&self.label) {return Ok(event);}
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
        self.open_generation(request,1)
    }
    fn open_generation(&mut self, request: &Request, generation:u64) -> Result<Box<dyn Transport>,String> {
        request.settings.validate()?;
        match &request.source {
            Source::Physical(info) => {
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
            #[cfg(all(test,not(windows)))]
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
fn publish_failed(shared:&mut Shared, failed:&Snapshot, seen:u64) {
    let old=shared.snapshot.frame.take();
    let mut snapshot=failed.clone();
    if shared.revision != seen {
        snapshot.frame=None;
        snapshot.requested=shared.snapshot.requested;
        snapshot.requested_revision=shared.snapshot.requested_revision;
        snapshot.settings_digest=shared.snapshot.settings_digest.clone();
        snapshot.startup_intent=shared.snapshot.startup_intent;
    }
    else if snapshot.frame.is_none() {snapshot.frame=old;}
    shared.snapshot=snapshot;
}
impl Worker {
    pub fn new() -> Self {
        Self::with_factory(RealFactory)
    }
    pub fn with_factory(factory: impl Factory + 'static) -> Self {
        Self::with_factory_recorder(factory, crate::failure_record::Recorder::new())
    }
    pub fn with_factory_recorder(factory: impl Factory + 'static, mut recorder: crate::failure_record::Recorder) -> Self {
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
                        engine.fail(FailureCode::GuiFailure, "GUI failed; restart disabled; actual collection required".into());
                        engine.snapshot.state = State::Error;
                        engine.snapshot.message =
                            "GUI failed; restart disabled; actual collection required".into();
                    }
                    recorder.observe(&mut engine.snapshot);
                    let done = closing && !engine.snapshot.active;
                    if done { recorder.finish(&mut engine.snapshot); }
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
                    let wake = if engine.snapshot.frame.is_some() { s.wake_gui.clone() } else { None };
                    engine.snapshot.frame = None;
                    s.closed = done;
                    drop(s);
                    if let Some(wake) = wake { wake(); }
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
                let mut s=state.lock().unwrap_or_else(|e|e.into_inner());
                publish_failed(&mut s, &engine.snapshot, seen);
                s.closed=true;
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
        s.snapshot.settings_digest=None;
        s.snapshot.startup_intent=request.as_ref().map(|r|r.startup_intent);
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

#[cfg(all(test, windows))]
mod continuous_worker_tests {
    use super::*;
    #[test]
    fn actual_worker_stall_faults_lease_and_collects_without_painter() {
        use crate::continuous_session::{ContinuousSession, Fixture};
        struct FactoryOnce(std::path::PathBuf);
        struct Stall {
            inner: ContinuousTransport,
            stalled: bool,
        }
        impl Factory for FactoryOnce {
            fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
                Ok(Box::new(Stall {
                    inner: ContinuousTransport::new(
                        ContinuousSession::start_fixture(&self.0, Fixture::Normal).unwrap(),
                        format!("SIMULATION evidence={}", self.0.display()),
                    ),
                    stalled: false,
                }))
            }
        }
        impl Transport for Stall {
            fn lifecycle(&self) -> Option<Lifecycle> {
                self.inner.lifecycle()
            }
            fn stop(&mut self) -> Result<(), String> {
                self.inner.stop()
            }
            fn poll(&mut self) -> Result<Event, String> {
                if self.inner.sequence >= 2 && !self.stalled {
                    self.stalled = true;
                    std::thread::sleep(Duration::from_secs(3));
                }
                self.inner.poll()
            }
        }
        let root = std::path::PathBuf::from(std::env::var_os("GUI_CONTINUOUS_FAKE_ROOT").unwrap())
            .join("worker-stall");
        std::fs::create_dir(&root).unwrap();
        let worker = Worker::with_factory(FactoryOnce(root.clone()));
        worker
            .request(Some(Request {
                startup_intent: crate::startup::Intent::Normal,
                source: Source::Simulation,
                settings: Settings::default(),
            }))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(35);
        let mut final_state = worker.snapshot();
        while !final_state.lifecycle.as_ref().is_some_and(|l| l.collected)
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(100));
            final_state = worker.snapshot();
        }
        assert!(final_state.lifecycle.as_ref().unwrap().collected);
        assert!(final_state.restart_disabled);
        assert!(final_state.message.contains("Lifecycle owner unresponsive"));
        assert!(
            worker
                .request(Some(Request {
                startup_intent: crate::startup::Intent::Normal,
                    source: Source::Simulation,
                    settings: Settings::default()
                }))
                .is_err()
        );
        worker.close();
        while !worker.closed() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(33));
        }
        assert!(worker.closed());
        println!(
            "SIMULATION actual Worker stall: {:?}; {}",
            final_state.lifecycle, final_state.message
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    #[test]
    fn gui_failure_during_active_poll_cannot_be_overwritten_by_old_snapshot() {
        use std::sync::mpsc;
        struct Make(Option<Held>);
        struct Held {
            entered: mpsc::SyncSender<()>,
            release: mpsc::Receiver<()>,
            count: u8,
        }
        impl Factory for Make {
            fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
                Ok(Box::new(self.0.take().unwrap()))
            }
        }
        impl Transport for Held {
            fn stop(&mut self) -> Result<(), String> {
                Ok(())
            }
            fn poll(&mut self) -> Result<Event, String> {
                self.count += 1;
                self.entered.send(()).unwrap();
                self.release.recv().unwrap();
                Ok(if self.count == 2 {
                    Event::Done { clean: true }
                } else {
                    Event::Waiting
                })
            }
        }
        let (entered, ready) = mpsc::sync_channel(1);
        let (release, released) = mpsc::sync_channel(1);
        let worker = Worker::with_factory(Make(Some(Held {
            entered,
            release: released,
            count: 0,
        })));
        let mut guardian = worker.guardian();
        worker
            .request(Some(Request {
                startup_intent: crate::startup::Intent::Normal,
                source: Source::Simulation,
                settings: Settings::default(),
            }))
            .unwrap();
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        guardian.gui_failed();
        release.send(()).unwrap();
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        let observed = worker.snapshot();
        release.send(()).unwrap();
        guardian.wait();
        assert_eq!(observed.state, State::Error);
        assert!(observed.restart_disabled);
    }
    #[test]
    fn held_poll_preserves_ui_origin_and_immediate_close_snapshot() {
        use std::sync::mpsc::{self, Receiver, SyncSender};
        struct HeldFactory(Option<Held>);
        struct Held {
            entered: SyncSender<()>,
            release: Receiver<()>,
            stopped: bool,
        }
        impl Factory for HeldFactory {
            fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
                Ok(Box::new(self.0.take().unwrap()))
            }
        }
        impl Transport for Held {
            fn stop(&mut self) -> Result<(), String> {
                self.stopped = true;
                Ok(())
            }
            fn poll(&mut self) -> Result<Event, String> {
                self.entered.send(()).unwrap();
                self.release.recv().unwrap();
                Ok(if self.stopped {
                    Event::Done { clean: true }
                } else {
                    Event::Progress(crate::progress::Stage::Controls)
                })
            }
        }
        let (entered, observed) = mpsc::sync_channel(1);
        let (release, held) = mpsc::sync_channel(1);
        let w = Worker::with_factory(HeldFactory(Some(Held {
            entered,
            release: held,
            stopped: false,
        })));
        let origin = Instant::now();
        w.request_at(Some(request(100)), origin).unwrap();
        observed.recv_timeout(Duration::from_secs(3)).unwrap(); // poll held
        let change = origin + Duration::from_secs(4);
        w.request_at(Some(request(50)), change).unwrap();
        assert_eq!(w.snapshot().started, Some(change));
        release.send(()).unwrap(); // obsolete poll cannot replace immediate snapshot
        observed.recv_timeout(Duration::from_secs(3)).unwrap(); // old owner stopping
        assert_eq!(w.snapshot().started, Some(change));
        // Inject an observed owner setting to verify close preserves it until completion.
        let applied = request(100).settings;
        {
            let mut s = w.shared.lock().unwrap();
            s.snapshot.applied = Some(applied);
            s.snapshot.progress = Some(crate::progress::Stage::Live);
            s.snapshot.stage_started = Some(origin);
        }
        let closing = origin + Duration::from_secs(9);
        w.close_at(closing);
        let s = w.snapshot();
        assert_eq!(s.state, State::Stopping);
        assert_eq!(s.started, Some(closing));
        assert_eq!(s.applied, Some(applied));
        assert!(s.progress.is_none());
        assert!(s.stage_started.is_none());
        assert!(s.requested.is_none());
        assert!(s.frame.is_none());
        w.close_at(closing + Duration::from_secs(1));
        assert_eq!(w.snapshot().started, Some(closing));
        release.send(()).unwrap();
        let limit = Instant::now() + Duration::from_secs(3);
        while !w.closed() && Instant::now() < limit {
            std::thread::yield_now();
        }
        assert!(w.closed());
        assert_eq!(w.snapshot().started, Some(closing));
    }
    #[test]
    fn injected_stop_deadline_exact_boundary_retains_owner() {
        let (mut e, opens) = engine(vec![vec![]]);
        let zero = Instant::now();
        e.request_at(1, Some(request(100)), zero);
        e.tick_at(zero);
        e.request_at(2, None, zero + Duration::from_secs(1));
        e.tick_at(zero + Duration::from_secs(2));
        assert_eq!(
            e.active.as_ref().unwrap().stop_at,
            Some(zero + Duration::from_secs(2))
        );
        e.tick_at(zero + Duration::from_secs(67) - Duration::from_nanos(1));
        assert!(!e.blocked);
        e.tick_at(zero + Duration::from_secs(67));
        assert!(e.blocked);
        assert!(e.snapshot.active);
        e.request_at(3, Some(request(50)), zero + Duration::from_secs(68));
        e.tick_at(zero + Duration::from_secs(200));
        assert_eq!(*opens.lock().unwrap(), vec![100]);
        assert_eq!(
            e.active.as_ref().unwrap().stop_at,
            Some(zero + Duration::from_secs(2))
        );
    }
    struct Script {
        opens: Arc<Mutex<Vec<u16>>>,
        live: Arc<Mutex<usize>>,
        events: VecDeque<VecDeque<Result<Event, String>>>,
    }
    struct Fake {
        events: VecDeque<Result<Event, String>>,
        live: Arc<Mutex<usize>>,
        stops: Arc<Mutex<usize>>,
    }
    impl Drop for Fake {
        fn drop(&mut self) {
            *self.live.lock().unwrap() -= 1;
        }
    }
    impl Transport for Fake {
        fn poll(&mut self) -> Result<Event, String> {
            self.events.pop_front().unwrap_or(Ok(Event::Waiting))
        }
        fn stop(&mut self) -> Result<(), String> {
            *self.stops.lock().unwrap() += 1;
            Ok(())
        }
    }
    impl Factory for Script {
        fn open(&mut self, r: &Request) -> Result<Box<dyn Transport>, String> {
            assert_eq!(
                *self.live.lock().unwrap(),
                0,
                "overlapping device ownership"
            );
            *self.live.lock().unwrap() += 1;
            self.opens.lock().unwrap().push(r.settings.exposure_rows);
            Ok(Box::new(Fake {
                events: self.events.pop_front().unwrap(),
                live: self.live.clone(),
                stops: Arc::new(Mutex::new(0)),
            }))
        }
    }
    fn request(rows: u16) -> Request {
        Request {
            startup_intent: crate::startup::Intent::Normal,
            source: Source::Simulation,
            settings: Settings {
                exposure_rows: rows,
                ..Default::default()
            },
        }
    }
    fn frame() -> Frame {
        Simulation { sequence: 0 }.next_frame().unwrap()
    }
    fn engine(events: Vec<Vec<Result<Event, String>>>) -> (Engine<Script>, Arc<Mutex<Vec<u16>>>) {
        let opens = Arc::new(Mutex::new(vec![]));
        (
            Engine::new(Script {
                opens: opens.clone(),
                live: Arc::new(Mutex::new(0)),
                events: events.into_iter().map(VecDeque::from).collect(),
            }),
            opens,
        )
    }
    #[test]
    fn progress_coalesces_and_stale_revision_cannot_certify_settings() {
        use crate::progress::Stage;
        let (mut e, opens) = engine(vec![
            vec![
                Ok(Event::Progress(Stage::Pattern155)),
                Ok(Event::Progress(Stage::Identity)),
                Ok(Event::Progress(Stage::Live)),
                Ok(Event::Done { clean: true }),
            ],
            vec![
                Ok(Event::Progress(Stage::Controls)),
                Ok(Event::Frame(frame())),
            ],
        ]);
        let zero = Instant::now();
        e.request_at(1, Some(request(100)), zero);
        e.tick_at(zero);
        e.tick_at(zero + Duration::from_secs(3));
        assert_eq!(e.snapshot.progress, Some(Stage::Pattern155));
        assert_eq!(
            e.snapshot.stage_started,
            Some(zero + Duration::from_secs(3))
        );
        e.tick_at(zero + Duration::from_secs(4));
        assert_eq!(e.snapshot.progress, Some(Stage::Pattern155)); // regression ignored
        for id in 2..=9 {
            e.request_at(id, Some(request(id as u16)), zero + Duration::from_secs(5));
        }
        e.tick_at(zero + Duration::from_secs(6)); // stale Live
        assert_eq!(e.snapshot.progress, None);
        assert_eq!(e.snapshot.requested, Some(request(9).settings));
        assert!(e.snapshot.applied.is_none());
        e.tick_at(zero + Duration::from_secs(7));
        e.tick_at(zero + Duration::from_secs(8));
        e.tick_at(zero + Duration::from_secs(10));
        assert_eq!(*opens.lock().unwrap(), vec![100, 9]);
        assert_eq!(e.snapshot.progress, Some(Stage::Controls));
        assert_eq!(e.snapshot.started, Some(zero + Duration::from_secs(5)));
        assert_eq!(
            e.snapshot.stage_started,
            Some(zero + Duration::from_secs(10))
        );
        assert!(e.snapshot.applied.is_none()); // protocol cannot certify application
        e.tick_at(zero + Duration::from_secs(11));
        assert_eq!(e.snapshot.state, State::Streaming);
    }
    #[test]
    fn terminal_presentation_distinguishes_end_cancel_and_error() {
        for (cancel, error, clean) in [
            (false, false, true),
            (true, false, true),
            (false, true, true),
            (false, false, false),
        ] {
            let (mut e, _) = engine(vec![vec![
                if error {
                    Err("fixture failure".into())
                } else {
                    Ok(Event::Waiting)
                },
                Ok(Event::Done { clean }),
            ]]);
            let now = Instant::now();
            e.request_at(1, Some(request(100)), now);
            e.tick_at(now);
            e.tick_at(now + Duration::from_secs(2));
            if cancel {
                e.request_at(2, None, now + Duration::from_secs(3));
                assert_eq!(e.snapshot.started, Some(now + Duration::from_secs(3)));
            }
            e.tick_at(now + Duration::from_secs(4));
            assert!(!e.snapshot.active);
            assert!(e.snapshot.applied.is_none());
            assert_eq!(
                e.snapshot.state,
                if error || !clean {
                    State::Error
                } else {
                    State::Disconnected
                }
            );
            assert!(e.snapshot.message.contains(if !clean {
                "Cleanup unproved"
            } else if error {
                "fixture failure"
            } else if cancel {
                "cancelled"
            } else {
                "Simulation ended"
            }));
            e.tick_at(now + Duration::from_secs(100));
            assert!(!e.snapshot.active); // no automatic trial loop
        }
    }
    #[test]
    fn repeated_requests_wait_for_cleanup_and_coalesce_stale_frames() {
        let (mut e, opens) = engine(vec![
            vec![
                Ok(Event::Frame(frame())),
                Ok(Event::Waiting),
                Ok(Event::Frame(frame())),
                Ok(Event::Done { clean: true }),
            ],
            vec![Ok(Event::Frame(frame()))],
        ]);
        e.request(1, Some(request(100)));
        e.tick();
        e.tick();
        assert_eq!(e.snapshot.state, State::Streaming);
        assert!(e.accepts(1));
        let captured = e.snapshot.frame.take().unwrap().1;
        let original = captured.metadata.clone();
        for id in 2..100 {
            let mut wanted = request(id as u16);
            wanted.settings.mode = if id % 2 == 0 {
                crate::controls::Mode::Full
            } else {
                crate::controls::Mode::FullFieldSkip
            };
            e.request(id, Some(wanted));
        }
        assert_eq!(e.snapshot.state, State::SettingsPending);
        assert!(!e.accepts(1));
        e.tick(); // Waiting; old child still owns device.
        assert_eq!(*opens.lock().unwrap(), vec![100]);
        e.tick(); // Stale old frame must not become visible.
        assert!(e.snapshot.frame.is_none());
        e.tick(); // Done; never open in same tick.
        assert_eq!(*opens.lock().unwrap(), vec![100]);
        e.tick();
        e.tick();
        assert_eq!(*opens.lock().unwrap(), vec![100, 99]);
        assert!(!e.accepts(1));
        assert!(e.accepts(99));
        assert_eq!(
            e.active.as_ref().unwrap().request.settings.exposure_rows,
            99
        );
        assert!(e.snapshot.applied.is_none()); // Simulation never claims hardware application.
        assert_eq!(
            e.active.as_ref().unwrap().request.settings.mode,
            crate::controls::Mode::FullFieldSkip
        );
        assert_eq!(captured.metadata, original); // Frozen capture remains original.
    }
    #[test]
    fn every_profile_replacement_rejects_stale_owner_frame() {
        for mode in crate::controls::Mode::ALL {
            let (mut e, opens) = engine(vec![
                vec![
                    Ok(Event::Frame(frame())),
                    Ok(Event::Frame(frame())),
                    Ok(Event::Done { clean: true }),
                ],
                vec![Ok(Event::Waiting)],
            ]);
            e.request(1, Some(request(100)));
            e.tick();
            e.tick();
            let retained = e.snapshot.frame.take().unwrap().1;
            let original = retained.metadata.clone();
            let mut next = request(101);
            next.settings.mode = mode;
            e.request(2, Some(next));
            e.tick();
            assert!(e.snapshot.frame.is_none());
            assert!(!e.accepts(1));
            assert_eq!(*opens.lock().unwrap(), vec![100]);
            e.tick();
            e.tick();
            assert_eq!(e.active.as_ref().unwrap().request.settings.mode, mode);
            assert_eq!(retained.metadata, original);
        }
    }
    #[test]
    fn disconnect_cancels_pending_open_and_can_reconnect_after_cleanup() {
        let (mut e, opens) = engine(vec![
            vec![Ok(Event::Waiting), Ok(Event::Done { clean: true })],
            vec![Ok(Event::Frame(frame()))],
        ]);
        e.request(1, Some(request(100)));
        e.tick();
        e.request(2, Some(request(50)));
        e.request(3, None);
        e.tick();
        e.tick();
        e.tick();
        assert_eq!(e.snapshot.state, State::Disconnected);
        assert_eq!(*opens.lock().unwrap(), vec![100]);
        e.request(4, Some(request(30)));
        e.tick();
        e.tick();
        assert!(e.accepts(4));
        assert_eq!(*opens.lock().unwrap(), vec![100, 30]);
    }
    #[test]
    fn process_failure_preserves_owner_until_reaped_and_blocks_unproved_cleanup() {
        for clean in [true, false] {
            let (mut e, opens) = engine(vec![
                vec![
                    Err("process/USB failure".into()),
                    Ok(Event::Waiting),
                    Ok(Event::Done { clean }),
                ],
                vec![],
            ]);
            e.request(1, Some(request(100)));
            e.tick();
            e.tick();
            assert_eq!(e.snapshot.state, State::Error);
            assert!(e.snapshot.active);
            e.request(2, Some(request(50)));
            e.tick();
            assert_eq!(*opens.lock().unwrap(), vec![100]);
            e.tick();
            e.tick();
            if clean {
                assert_eq!(*opens.lock().unwrap(), vec![100, 50]);
            } else {
                assert_eq!(e.snapshot.state, State::Error);
                e.request(3, Some(request(25)));
                e.tick();
                assert_eq!(*opens.lock().unwrap(), vec![100]);
            }
        }
    }
    #[test]
    fn stop_is_sent_once_and_worker_close_waits_without_blocking_caller() {
        struct SlowFactory {
            live: Arc<Mutex<usize>>,
            stops: Arc<Mutex<usize>>,
        }
        struct Slow {
            live: Arc<Mutex<usize>>,
            stops: Arc<Mutex<usize>>,
            remaining: u8,
        }
        impl Factory for SlowFactory {
            fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
                let mut live = self.live.lock().unwrap();
                assert_eq!(*live, 0);
                *live = 1;
                Ok(Box::new(Slow {
                    live: self.live.clone(),
                    stops: self.stops.clone(),
                    remaining: 4,
                }))
            }
        }
        impl Drop for Slow {
            fn drop(&mut self) {
                *self.live.lock().unwrap() = 0;
            }
        }
        impl Transport for Slow {
            fn stop(&mut self) -> Result<(), String> {
                *self.stops.lock().unwrap() += 1;
                Ok(())
            }
            fn poll(&mut self) -> Result<Event, String> {
                if *self.stops.lock().unwrap() == 0 {
                    return Ok(Event::Waiting);
                }
                if self.remaining > 0 {
                    self.remaining -= 1;
                    Ok(Event::Waiting)
                } else {
                    Ok(Event::Done { clean: true })
                }
            }
        }
        let live = Arc::new(Mutex::new(0));
        let stops = Arc::new(Mutex::new(0));
        let w = Worker::with_factory(SlowFactory {
            live: live.clone(),
            stops: stops.clone(),
        });
        w.request(Some(request(100))).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while *live.lock().unwrap() == 0 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(*live.lock().unwrap(), 1);
        w.close();
        assert!(!w.closed());
        assert!(w.request(Some(request(50))).is_err());
        while !w.closed() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(w.closed());
        assert_eq!(*live.lock().unwrap(), 0);
        assert_eq!(*stops.lock().unwrap(), 1);
    }
    #[test]
    fn real_adapter_cancel_exit_marker_reap_preserves_latest_or_blocks() {
        struct ChildFactory {
            root: std::path::PathBuf,
            opens: Arc<Mutex<usize>>,
            unclean: bool,
        }
        impl Factory for ChildFactory {
            fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
                let mut opens = self.opens.lock().unwrap();
                *opens += 1;
                if *opens > 1 {
                    return Ok(Box::new(Sim {
                        simulation: Simulation { sequence: 0 },
                        stopped: false,
                    }));
                }
                let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                command
                    .args([
                        "--exact",
                        "physical_preview::protocol_tests::child",
                        "--nocapture",
                    ])
                    .env("CAMERA_LAB_PROTOCOL_TEST", &self.root);
                if self.unclean {
                    command.env("CAMERA_LAB_NO_CLEANUP", "1");
                }
                Ok(Box::new(Physical {
                    preview: PhysicalPreview::test_child(
                        command.spawn().unwrap(),
                        self.root.clone(),
                    ),
                    error: None,
                    progress: None,
                }))
            }
        }
        for (index, unclean, disconnect) in [(0, false, false), (1, true, false), (2, false, true)]
        {
            let root =
                std::env::temp_dir().join(format!("adapter-cancel-{}-{index}", std::process::id()));
            std::fs::create_dir(&root).unwrap();
            let opens = Arc::new(Mutex::new(0));
            let mut e = Engine::new(ChildFactory {
                root: root.clone(),
                opens: opens.clone(),
                unclean,
            });
            e.request(1, Some(request(100)));
            e.tick();
            for id in 2..30 {
                e.request(id, Some(request(id as u16)));
            }
            if disconnect {
                e.request(30, None);
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            while e.snapshot.active && Instant::now() < deadline {
                e.tick();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(!e.snapshot.active);
            assert_eq!(*opens.lock().unwrap(), 1, "no replacement on reap tick");
            e.tick();
            assert_eq!(
                *opens.lock().unwrap(),
                if unclean || disconnect { 1 } else { 2 }
            );
            if unclean {
                assert!(e.blocked);
                assert_eq!(e.snapshot.state, State::Error);
            } else {
                assert_ne!(e.snapshot.state, State::Error);
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn panicked_worker_is_terminal_and_rejects_new_requests() {
        struct Panics;
        impl Factory for Panics {
            fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
                panic!("injected acquisition failure");
            }
        }
        let w = Worker::with_factory(Panics);
        w.request(Some(request(100))).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !w.closed() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(w.closed());
        assert_eq!(w.snapshot().state, State::Error);
        assert!(w.request(Some(request(50))).is_err());
    }
}

#[cfg(all(test,not(windows)))]
mod v034_tests {
 use super::*;
 struct Make;
 struct Fake {generation:u64,settings:Settings,stopped:bool,intent:crate::startup::Intent}
 impl Factory for Make {
  fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{unreachable!()}
  fn open_generation(&mut self,r:&Request,g:u64)->Result<Box<dyn Transport>,String>{
   match r.source {Source::Simulation=>Ok(Box::new(Sim{simulation:Simulation{sequence:0},stopped:false})),Source::HostFake=>{assert_eq!(g,3);Ok(Box::new(Fake{generation:g,settings:r.settings,stopped:false,intent:r.startup_intent}))},_=>panic!("no device access")}
  }
 }
 impl Transport for Fake {
  fn stop(&mut self)->Result<(),String>{self.stopped=true;Ok(())}
  fn poll(&mut self)->Result<Event,String>{if self.stopped{return Ok(Event::Done{clean:true});}
   let mut f=Simulation{sequence:0}.next_frame()?;
   let a=crate::sensor_contract::Acquisition::new(self.settings,[3;32],self.generation,1)?.with_startup(self.intent);
   let (w,h)=self.settings.dimensions();f.raw=Arc::new(vec![0;usize::from(w)*usize::from(h)]);f.metadata.width=w.into();f.metadata.height=h.into();f.metadata.format="BayerGRBG8".into();f.metadata.sequence=1;f.metadata.acquisition=Some(a);Ok(Event::Frame(f))}
 }
 #[test] fn worker_simulation_none_fake_generation3(){
  let w=Worker::with_factory(Make);let r=|source|Request{source,settings:Settings::default(),startup_intent:crate::startup::Intent::Normal};
  assert_eq!(w.request(Some(r(Source::Simulation))).unwrap(),1);
  let wait=|p:&dyn Fn(&Snapshot)->bool|{let end=Instant::now()+Duration::from_secs(5);loop{let s=w.snapshot();if p(&s){return s;}assert!(Instant::now()<end,"{}",s.message);thread_yield();}};
  wait(&|s|s.frame.is_some());assert_eq!(w.request(None).unwrap(),2);wait(&|s|!s.active);
  assert_eq!(w.request(Some(r(Source::HostFake))).unwrap(),3);let s=wait(&|s|s.frame.is_some());let (revision,f)=s.frame.unwrap();assert_eq!(revision,3);assert_eq!(f.metadata.acquisition.unwrap().settings_generation,3);w.close();wait(&|s|!s.active);
 }
 fn thread_yield(){std::thread::sleep(Duration::from_millis(2));}
 #[test] fn first_backend_error_survives_failed_cleanup(){
  struct F;struct T(u8);
  impl Factory for F{fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{Ok(Box::new(T(0)))}}
  impl Transport for T{fn stop(&mut self)->Result<(),String>{Err("later STOP failure".into())}fn poll(&mut self)->Result<Event,String>{self.0+=1;if self.0==1{Err("preopen backend failure".into())}else{Ok(Event::Done{clean:false})}}}
  let root=std::env::temp_dir().join(format!("v034-engine-record-{}",std::process::id()));let mut recorder=crate::failure_record::Recorder::at(root.clone());
  let mut e=Engine::new(F);e.request(3,Some(Request{source:Source::Simulation,settings:Settings::default(),startup_intent:crate::startup::Intent::Normal}));e.tick();e.tick();assert_eq!(e.snapshot.first_failure.as_deref(),Some("preopen backend failure"));assert_eq!(e.snapshot.owner_revision,Some(3));recorder.observe(&mut e.snapshot);e.request(4,None);e.tick();assert!(e.snapshot.restart_disabled);assert!(!e.snapshot.active);assert_eq!(e.snapshot.first_failure.as_deref(),Some("preopen backend failure"));recorder.finish(&mut e.snapshot);
  let read=|name:&str|->serde_json::Value{serde_json::from_slice(&std::fs::read(root.join("slot-00").join(name)).unwrap()).unwrap()};let first=read("first.json");let final_record=read("collection.json");assert_eq!(first["first_failure"]["code"],"TransportPoll");assert_eq!(first["first_failure"],final_record["first_failure"]);assert_eq!(final_record["owner_revision"],3);assert_eq!(final_record["latest_message"]["restart_disabled"],true);std::fs::remove_dir_all(root).unwrap();
 }
}

#[cfg(all(test,not(windows)))]
mod v034_binding_test {
 use super::*;
 #[test] fn stale_typed_generation_cannot_be_presented(){
  struct F;struct T;
  impl Factory for F{fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{Ok(Box::new(T))}}
  impl Transport for T{fn stop(&mut self)->Result<(),String>{Ok(())}fn poll(&mut self)->Result<Event,String>{
   let settings=Settings::default();let (w,h)=settings.dimensions();let a=crate::sensor_contract::Acquisition::new(settings,[2;32],2,1)?.with_startup(crate::startup::Intent::Normal);
   Ok(Event::Frame(Frame{raw:Arc::new(vec![0;usize::from(w)*usize::from(h)]),metadata:crate::Metadata{acquisition:Some(a),source:"host fake".into(),simulated:true,sequence:1,measurement:None,unix_millis:0,width:w.into(),height:h.into(),format:"BayerGRBG8".into()}}))}}
  let mut e=Engine::new(F);e.request(3,Some(Request{source:Source::HostFake,settings:Settings::default(),startup_intent:crate::startup::Intent::Normal}));e.tick();e.tick();assert!(e.snapshot.frame.is_none());assert_eq!(e.snapshot.first_failure.as_deref(),Some("Frame settings contract mismatch"));assert!(e.snapshot.active);
 }
}

#[cfg(test)] mod v035_tests {
 use super::*;
 struct Inject {panic:bool}
 impl Factory for Inject {
  fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{unreachable!()}
  fn open_generation(&mut self,_:&Request,_:u64)->Result<Box<dyn Transport>,String>{
   if self.panic {panic!("injected pre-owner panic");}
   Err("Continuous acquisition failed: backend preopen. Evidence: bus=SENTINEL_BUS ports=SENTINEL_PORT /Users/SENTINEL_USER/private".into())
  }
 }
 fn req()->Request{Request{source:Source::Simulation,settings:Settings::default(),startup_intent:crate::startup::Intent::Normal}}
 fn wait(w:&Worker,p:impl Fn(&Snapshot)->bool)->Snapshot {let end=Instant::now()+Duration::from_secs(5);loop{let s=w.snapshot();if p(&s){return s;}assert!(Instant::now()<end,"{}",s.message);std::thread::sleep(Duration::from_millis(2));}}
 #[test] fn worker_production_open_error_excludes_identity_both_records(){
  let root=std::env::temp_dir().join(format!("v035-identity-{}",std::process::id()));
  let w=Worker::with_factory_recorder(Inject{panic:false},crate::failure_record::Recorder::at(root.clone()));
  w.request(Some(req())).unwrap();let s=wait(&w,|s|s.first_failure.is_some());assert!(s.first_failure.unwrap().contains("SENTINEL_BUS"));
  w.close();wait(&w,|_|w.closed());let mut guardian=w.guardian();guardian.wait();
  for name in ["first.json","collection.json"] {let bytes=std::fs::read_to_string(root.join("slot-00").join(name)).unwrap();for sentinel in ["SENTINEL_BUS","SENTINEL_PORT","SENTINEL_USER","/Users/"]{assert!(!bytes.contains(sentinel));}let v:serde_json::Value=serde_json::from_str(&bytes).unwrap();assert_eq!(v["first_failure"]["code"],"OpenGeneration");}
  std::fs::remove_dir_all(root).unwrap();
 }
 #[test] fn worker_physical_style_lifecycle_message_safe_records(){
  struct F;struct T(bool);
  impl Factory for F{fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{Ok(Box::new(T(false)))}}
  impl Transport for T {
   fn stop(&mut self)->Result<(),String>{Ok(())}
   fn lifecycle(&self)->Option<Lifecycle>{Some(Lifecycle{helper_fault:Some(crate::continuous::Fault::Initialization),backend_error:Some("preopen SENTINEL_BUS /Users/SENTINEL_USER".into()),parent_error:Some("SENTINEL_PORT".into()),terminal_kind:Some(crate::diagnostic::Terminal::Unproved),..Default::default()})}
   fn poll(&mut self)->Result<Event,String>{if self.0{return Ok(Event::Done{clean:false});}self.0=true;Ok(Event::Lifecycle{message:continuous_failure_message("preopen IO timeout","bus=SENTINEL_BUS ports=SENTINEL_PORT /Users/SENTINEL_USER/private"),unproved:true})}
  }
  let root=std::env::temp_dir().join(format!("v035-lifecycle-{}",std::process::id()));let w=Worker::with_factory_recorder(F,crate::failure_record::Recorder::at(root.clone()));w.request(Some(req())).unwrap();let s=wait(&w,|s|s.first_failure.is_some());assert!(s.first_failure.unwrap().contains("SENTINEL_BUS"));w.close();wait(&w,|_|w.closed());let mut g=w.guardian();g.wait();
  for name in ["first.json","collection.json"]{let raw=std::fs::read_to_string(root.join("slot-00").join(name)).unwrap();for value in ["SENTINEL_BUS","SENTINEL_PORT","SENTINEL_USER","/Users/"]{assert!(!raw.contains(value));}let v:serde_json::Value=serde_json::from_str(&raw).unwrap();assert_eq!(v["first_failure"]["code"],"Unknown");assert_eq!(v["helper_fault"],"Initialization");assert_eq!(v["terminal_kind"],"Unproved");}
  std::fs::remove_dir_all(root).unwrap();
 }
 #[test] fn worker_open_panic_publishes_full_failure_and_failed_persistence(){
  let root=std::env::temp_dir().join(format!("v035-panic-{}",std::process::id()));std::fs::write(&root,b"blocked").unwrap();
  let w=Worker::with_factory_recorder(Inject{panic:true},crate::failure_record::Recorder::at(root.clone()));w.request(Some(req())).unwrap();
  let s=wait(&w,|_|w.closed());assert!(!s.active);assert!(s.restart_disabled);assert!(s.first_failure.as_deref().unwrap().contains("worker panicked"));assert!(s.first_failure_original_bytes.unwrap()>0);assert!(s.persistence.starts_with("FAILURE RECORD NOT PERSISTED"));assert!(s.frame.is_none());assert_eq!(s.owner_revision,Some(1));
  let mut guardian=w.guardian();guardian.wait();std::fs::remove_file(root).unwrap();
 }
 #[test] fn request_none_clears_facts_owner_remains_revision_bound(){
  struct F;struct T;impl Factory for F{fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{Ok(Box::new(T))}}
  impl Transport for T{fn stop(&mut self)->Result<(),String>{Ok(())}fn poll(&mut self)->Result<Event,String>{Ok(Event::Waiting)}}
  let mut e=Engine::new(F);e.request(1,Some(req()));e.snapshot.settings_digest=Some("digest-A".into());e.tick();
  let mut b=req();b.settings.exposure_rows+=1;e.request(2,Some(b.clone()));e.tick();assert_eq!(e.snapshot.owner_revision,Some(1));assert_eq!(e.snapshot.owner_settings,Some(req().settings));assert_eq!(e.snapshot.owner_settings_digest.as_deref(),Some("digest-A"));assert_eq!(e.snapshot.requested_revision,Some(2));assert_eq!(e.snapshot.requested,Some(b.settings));
  let b_json:serde_json::Value=serde_json::from_slice(&crate::failure_record::record(&e.snapshot,"test")).unwrap();assert_eq!(b_json["owner_revision"],1);assert_eq!(b_json["requested_revision"],2);assert_eq!(b_json["owner_settings_digest"],"digest-A");#[cfg(not(windows))] assert!(b_json["settings_digest"].is_null());
  #[cfg(windows)] {let canonical=format!("settings-v2-geometry1:{}:{}:{}",b.settings.mode.profile().id,b.settings.exposure_rows,b.settings.gain_eighths);let expected=capability_platform::digest(canonical.as_bytes()).expect("Windows digest success required in canonical-B test").iter().map(|v|format!("{v:02x}")).collect::<String>();assert_eq!(b_json["settings_digest"],expected);}
  e.request(3,None);assert!(e.snapshot.requested.is_none());assert!(e.snapshot.requested_revision.is_none());assert!(e.snapshot.settings_digest.is_none());assert!(e.snapshot.startup_intent.is_none());assert_eq!(e.snapshot.owner_revision,Some(1));assert_eq!(e.snapshot.owner_settings_digest.as_deref(),Some("digest-A"));
  let stopped:serde_json::Value=serde_json::from_slice(&crate::failure_record::record(&e.snapshot,"test")).unwrap();for field in ["requested_settings","requested_revision","settings_digest","startup_intent"]{assert!(stopped[field].is_null(),"{field}");}assert_eq!(stopped["owner_revision"],1);assert_eq!(stopped["owner_settings_digest"],"digest-A");
  assert_eq!(settings_digest_with(req().settings,|_|Err(())),None);assert_eq!(settings_digest_with(req().settings,|bytes|{assert!(std::str::from_utf8(bytes).unwrap().starts_with("settings-v2-geometry1:"));Ok([7;32])}),Some("07".repeat(32)));
  #[cfg(not(windows))] {let mut e=Engine::new(F);e.request(1,Some(req()));assert!(e.snapshot.settings_digest.is_none());}
 }
}

#[cfg(test)]
mod v036_tests {
 use super::*;
 use crate::diagnostic::{self as d,Accumulator,Cause,Stage,Reason,Terminal};
 struct Inject {parent:Option<Cause>,text:Option<&'static str>,terminal:Terminal,malformed:bool,flags:u8,exit:u32,release:bool,deleted:bool}
 struct BytesTransport {input:Inject,causes:Accumulator,facts:d::Facts,step:u8,reported:bool}
 impl Transport for BytesTransport {
  fn stop(&mut self)->Result<(),String>{Ok(())}
  fn lifecycle(&self)->Option<Lifecycle>{Some(d::lifecycle(&self.facts,&self.causes))}
  fn poll(&mut self)->Result<Event,String>{
   let mut raw=[0u8;128];raw[68]=self.input.flags;
   if self.step==0 {
    if let Some(c)=self.input.parent {self.causes.retain(c);}
    d::write_terminal_error(&mut raw,self.input.text);
    if self.input.malformed {raw[70]=58;}
    let _=self.causes.terminal(&raw);
    self.facts.parent_error=Some(self.input.text.unwrap_or("parent SENTINEL_BUS /Users/SENTINEL_USER").into());
   } else {
    // A later different helper fault must not rewrite first cause, including unknown.
    raw[68]=16;raw[69]=4;self.causes.terminal(&raw).unwrap();
    self.facts.collected=true;self.facts.exit=Some(self.input.exit);
    self.facts.release_committed=self.input.release;self.facts.marker_deleted=self.input.deleted;
    self.facts.terminal=Some(self.input.terminal);
   }
   let lifecycle=d::lifecycle(&self.facts,&self.causes);self.step+=1;
   Ok(d::transport_event(&lifecycle,&mut self.reported,"SENTINEL_PORT").unwrap_or(Event::Waiting))
  }
 }
 struct InjectFactory(Option<Inject>);
 impl Factory for InjectFactory {
  fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{Ok(Box::new(BytesTransport{input:self.0.take().unwrap(),causes:Accumulator::default(),facts:d::Facts::default(),step:0,reported:false}))}
 }
 fn request()->Request {Request{source:Source::Simulation,settings:Settings::default(),startup_intent:crate::startup::Intent::Normal}}
 fn wait(w:&Worker,p:impl Fn(&Snapshot)->bool)->Snapshot {let deadline=Instant::now()+Duration::from_secs(5);loop{let s=w.snapshot();if p(&s){return s;}assert!(Instant::now()<deadline,"worker timeout");std::thread::sleep(Duration::from_millis(2));}}
 #[test] fn v036_production_terminal_dispatch_worker_recorder_first_cause() {
  let cases=[
   (None,Some("@OP SENTINEL_BUS /Users/SENTINEL_USER"),d::backend_code("@OP x")),
   (None,Some("@DS SENTINEL_BUS /Users/SENTINEL_USER"),d::backend_code("@DS x")),
   (Some(Cause::parent(Stage::Initialization,Reason::Deadline)),None,Cause::parent(Stage::Initialization,Reason::Deadline)),
   (Some(Cause::parent(Stage::Owner,Reason::Unresponsive)),None,Cause::parent(Stage::Owner,Reason::Unresponsive)),
   (Some(Cause::parent(Stage::Channel,Reason::Lost)),None,Cause::parent(Stage::Channel,Reason::Lost)),
   (None,Some("unknown SENTINEL_BUS /Users/SENTINEL_USER"),Cause::unknown()),
   (None,None,Cause::parent(Stage::Terminal,Reason::Malformed))];
  for (index,(parent,text,expected)) in cases.into_iter().enumerate() {
   let root=std::env::temp_dir().join(format!("v036-path-{}-{index}",std::process::id()));
   let w=Worker::with_factory_recorder(InjectFactory(Some(Inject{parent,text,terminal:Terminal::FailedClean,malformed:index==6,flags:16,exit:2,release:true,deleted:false})),crate::failure_record::Recorder::at(root.clone()));
   w.request(Some(request())).unwrap();wait(&w,|s|s.first_failure.is_some());w.close();wait(&w,|_|w.closed());let mut g=w.guardian();g.wait();
   let mut first=None;
   for name in ["first.json","collection.json"] {
    let raw=std::fs::read_to_string(root.join("slot-00").join(name)).unwrap();
    for sentinel in ["SENTINEL_BUS","SENTINEL_PORT","SENTINEL_USER","/Users/"] {assert!(!raw.contains(sentinel));}
    let v:serde_json::Value=serde_json::from_str(&raw).unwrap();assert_eq!(v["first_failure"]["cause"],serde_json::json!(expected));
    if name=="first.json" {assert_eq!(v["proof_flags"],if index==6 {0}else{16});assert!(v["terminal_kind"].is_null());assert!(v["exit"].is_null());assert_eq!(v["collected"],false);assert_eq!(v["release_committed"],false);assert_eq!(v["marker_deleted"],false);first=Some(v["first_failure"].clone());}
    else {assert_eq!(v["first_failure"],first.clone().unwrap());assert_eq!(v["helper_fault"],"Initialization");assert_eq!(v["terminal_kind"],"FailedClean");assert_eq!(v["exit"],2);assert_eq!(v["proof_flags"],16);assert_eq!(v["release_committed"],true);assert_eq!(v["marker_deleted"],false);assert_eq!(v["collected"],true);}
   }
   std::fs::remove_dir_all(root).unwrap();
  }
 }
 #[test] fn v036_all_terminal_facts_to_collection() {
  for (index,(terminal,exit,proof,release,deleted)) in [(Terminal::CleanCancellation,0,15,true,true),(Terminal::FailedClean,2,23,true,false),(Terminal::Unproved,3,16,false,false)].into_iter().enumerate() {
   let mut acc=Accumulator::default();let mut packet=[0u8;128];packet[68]=proof;acc.terminal(&packet).unwrap();
   let l=d::lifecycle(&d::Facts{terminal:Some(terminal),exit:Some(exit),release_committed:release,marker_deleted:deleted,collected:true,..Default::default()},&acc);
   let mut reported=true;assert!(matches!(d::transport_event(&l,&mut reported,"inert"),Some(Event::Done{clean}) if clean==(terminal==Terminal::CleanCancellation)));
   let root=std::env::temp_dir().join(format!("v036-facts-{}-{index}",std::process::id()));let mut recorder=crate::failure_record::Recorder::at(root.clone());
   let mut snapshot=Snapshot::default();snapshot.first_failure=Some("inert collection observation".into());snapshot.lifecycle=Some(l);recorder.observe(&mut snapshot);recorder.finish(&mut snapshot);
   let v:serde_json::Value=serde_json::from_slice(&std::fs::read(root.join("slot-00/collection.json")).unwrap()).unwrap();
   assert_eq!(v["terminal_kind"],serde_json::json!(terminal));assert_eq!(v["exit"],exit);assert_eq!(v["proof_flags"],proof);assert_eq!(v["release_committed"],release);assert_eq!(v["marker_deleted"],deleted);assert_eq!(v["collected"],true);assert!(v["marker_present"].is_null());std::fs::remove_dir_all(root).unwrap();
  }
 }
 #[test] fn v036_request_hash_dependency_revision_owner_binding() {
  struct Idle;impl Transport for Idle{fn poll(&mut self)->Result<Event,String>{Ok(Event::Waiting)}fn stop(&mut self)->Result<(),String>{Ok(())}}
  struct HashFactory(bool);
  impl Factory for HashFactory {
   fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{Ok(Box::new(Idle))}
   fn settings_digest(&mut self,s:Settings)->Option<String>{settings_digest_with(s,|bytes|{assert!(std::str::from_utf8(bytes).unwrap().starts_with("settings-v2-geometry1:"));if self.0{Ok([9;32])}else{Err(())}})}
  }
  let mut e=Engine::new(HashFactory(true));e.request(1,Some(request()));e.tick();assert_eq!(e.snapshot.owner_settings_digest,Some("09".repeat(32)));
  e.factory.0=false;let mut b=request();b.settings.exposure_rows+=1;e.request(2,Some(b.clone()));assert_eq!(e.snapshot.requested,Some(b.settings));assert_eq!(e.snapshot.requested_revision,Some(2));assert!(e.snapshot.settings_digest.is_none());assert_eq!(e.snapshot.owner_revision,Some(1));assert_eq!(e.snapshot.owner_settings_digest,Some("09".repeat(32)));
  e.request(3,None);assert!(e.snapshot.settings_digest.is_none());assert!(e.snapshot.requested_revision.is_none());assert_eq!(e.snapshot.owner_settings_digest,Some("09".repeat(32)));
 }
}

#[cfg(test)] mod v037_tests {
 use super::*;
 use crate::diagnostic::{self as d, Cause, ParentReport as P};
 struct F(usize); struct T{case:usize,step:u8,a:d::Accumulator,f:d::Facts,reported:bool}
 impl Factory for F {fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{Ok(Box::new(T{case:self.0,step:0,a:Default::default(),f:Default::default(),reported:false}))}}
 impl Transport for T {
 fn stop(&mut self)->Result<(),String>{Ok(())}
 fn lifecycle(&self)->Option<Lifecycle>{Some(d::lifecycle(&self.f,&self.a))}
 fn poll(&mut self)->Result<Event,String>{
 if self.step==0 {
  if self.case>=6 {let reports=[P::OwnerUnresponsive,P::OwnerUnresponsive,P::ChallengeLost,P::ChallengeUnanswered,P::ChallengeStalled,P::ChallengeMalformed];
   let r=reports[self.case-6];d::report_parent(&mut self.a,&mut self.f.parent_error,r);
   // The evaluate path and supervisor both use this exact owner report mutation.
   if self.case==6 || self.case==7 {d::report_parent(&mut self.a,&mut self.f.parent_error,P::OwnerUnresponsive);}
  } else {
   if self.case==3 {d::report_parent(&mut self.a,&mut self.f.parent_error,P::OwnerUnresponsive);}
   let mut p=[0;128];p[68]=16;
   if [0,2,3,5].contains(&self.case){d::write_terminal_error(&mut p,Some(if self.case==5 {"raw SENTINEL_BUS /Users/SENTINEL_USER"}else{"@OP SENTINEL_BUS /Users/SENTINEL_USER"}));}
   if [1,2,3].contains(&self.case){p[69]=4;}
   if self.case==4 {p[70]=58;}
   let _=d::report_terminal(&mut self.a,&mut self.f.parent_error,&p);
  }
 } else {
  let mut p=[0;128];p[68]=16;p[69]=5;d::report_terminal(&mut self.a,&mut self.f.parent_error,&p).unwrap();
  self.f.collected=true;self.f.terminal=Some(d::Terminal::FailedClean);self.f.exit=Some(2);
 }
 self.step+=1;Ok(d::transport_event(&d::lifecycle(&self.f,&self.a),&mut self.reported,"SENTINEL_PORT").unwrap_or(Event::Waiting))
 }}
 #[test] fn v037_real_reports_worker_both_records() {
 let helper=Cause{origin:d::Origin::Helper,stage:d::Stage::Supervisor,reason:d::Reason::SupervisorFault,category:Some(4)};
 let expected=[d::backend_code("@OP x"),helper,d::backend_code("@OP x"),P::OwnerUnresponsive.cause(),Cause::parent(d::Stage::Terminal,d::Reason::Malformed),Cause::unknown(),P::OwnerUnresponsive.cause(),P::OwnerUnresponsive.cause(),P::ChallengeLost.cause(),P::ChallengeUnanswered.cause(),P::ChallengeStalled.cause(),P::ChallengeMalformed.cause()];
 for (case,cause) in expected.into_iter().enumerate(){
 let root=std::env::temp_dir().join(format!("v037-reports-{}-{case}",std::process::id()));
 let w=Worker::with_factory_recorder(F(case),crate::failure_record::Recorder::at(root.clone()));
 w.request(Some(Request{source:Source::Simulation,settings:Settings::default(),startup_intent:crate::startup::Intent::Normal})).unwrap();
 let end=Instant::now()+Duration::from_secs(5);while w.snapshot().first_failure.is_none(){assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(2));}
 assert_eq!(w.snapshot().first_cause,Some(cause));w.close();while !w.closed(){assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(2));}let mut g=w.guardian();g.wait();
 if [0,2,3].contains(&case){assert_eq!(w.snapshot().lifecycle.as_ref().unwrap().backend_cause,Some(d::backend_code("@OP x")));assert!(w.snapshot().lifecycle.as_ref().unwrap().backend_error.as_ref().unwrap().contains("SENTINEL_BUS"));}
 assert_eq!(w.snapshot().first_cause,Some(cause));assert_eq!(w.snapshot().lifecycle.unwrap().helper_fault,Some(crate::continuous::Fault::NoTransfer));
 let mut first=None;for name in ["first.json","collection.json"]{let raw=std::fs::read_to_string(root.join("slot-00").join(name)).unwrap();for text in ["SENTINEL_BUS","SENTINEL_PORT","SENTINEL_USER","/Users/"]{assert!(!raw.contains(text));}let v:serde_json::Value=serde_json::from_str(&raw).unwrap();assert_eq!(v["first_failure"]["cause"],serde_json::json!(cause));if let Some(ref f)=first{assert_eq!(&v["first_failure"],f);}else{first=Some(v["first_failure"].clone());}}
 std::fs::remove_dir_all(root).unwrap();
 }
 }
}
