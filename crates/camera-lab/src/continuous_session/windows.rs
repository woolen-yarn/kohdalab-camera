use crate::{
    Frame, Metadata,
    continuous::{Kind, Record},
    continuous_runtime as runtime,
    controls::{Mode, Settings},
};
use capability_platform as os;
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
const IO: Duration = Duration::from_secs(2);
const FRAME_CHUNK: usize = 65_536;
struct FixtureInitializationClock(runtime::Monotonic);
impl runtime::Clock for FixtureInitializationClock {
    fn now(&self) -> Duration {
        self.0.now() * 2
    }
}
fn err() -> io::Error {
    io::ErrorKind::InvalidData.into()
}
use crate::diagnostic::{decode_fault, write_terminal_error, Accumulator, Cause, Stage, Reason};
pub use crate::diagnostic::Terminal;
#[cfg(test)] use crate::diagnostic::terminal_error;
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub bus: String,
    pub ports: Vec<u8>,
}
impl Identity {
    /// Converts an already-selected descriptor. This does not enumerate/open.
    pub fn from_device(info: &nusb::DeviceInfo) -> io::Result<Self> {
        if info.vendor_id() != 0x0547
            || info.product_id() != 0x4d33
            || info.product_string() != Some("3.1M USB2.0 Camera")
            || info.device_version() != 0
            || info.serial_number().is_some()
        {
            return Err(err());
        }
        let identity = Self {
            bus: info.bus_id().to_owned(),
            ports: info.port_chain().to_vec(),
        };
        identity.validate()?;
        Ok(identity)
    }
    fn validate(&self) -> io::Result<()> {
        if self.bus.is_empty()
            || self.bus.len() > 64
            || !self
                .bus
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-{}#()_.".contains(&b))
            || self.ports.is_empty()
            || self.ports.len() > 8
            || self.ports.contains(&0)
        {
            return Err(err());
        }
        Ok(())
    }
    fn key(&self) -> String {
        format!(
            "0547:4d33:{}:{}",
            self.bus,
            self.ports
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(".")
        )
    }
}
/// Closed fixture faults. Only the dedicated fixture requests these; production
/// dispatch always selects Physical, independently of bootstrap contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fixture {
    Normal,
    FullFrames,
    XgaFrames,
    TypedFrames,
    NoTelemetry,
    BadTelemetry,
    ParentThreadFail,
    BootstrapStop,
    BootstrapStall,
    ResponseStop,
    LeaseStop,
    PrepStop,
    InitStall,
    NoTransfer,
    NoFrame,
    ProducerStall,
    DrainFail,
    CleanupFail,
    MissingReceipt,
    BadReceipt,
    NonzeroExit,
    ReleaseStall,
    Replay,
    StaleSettings,
    BadBootstrap,
    LeaseLoss,
    WorkerStall,
    ReapFail,
    UnlockFail,
    SettingsDigest,
    WaitTimeout,
    Crash,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bootstrap {
    version: u32,
    startup_intent: crate::startup::Intent,
    nonce: [u8; 32],
    identity: Identity,
    mode: String,
    geometry: crate::sensor_contract::Geometry,
    settings_generation: u64,
    exposure: u16,
    gain: u16,
    settings_digest: [u8; 32],
    output: PathBuf,
    fixture: Option<Fixture>,
}
impl Bootstrap {
    fn settings(&self) -> io::Result<Settings> {
        let settings = Settings {
            mode: Mode::parse(&self.mode).map_err(|_| err())?,
            exposure_rows: self.exposure,
            gain_eighths: self.gain,
        }
        .validate()
        .map_err(|_| err())?;
        if self.geometry != crate::sensor_contract::Geometry::for_mode(settings.mode).map_err(|_| err())?
            || self.settings_generation == 0 || self.settings_digest != settings_hash(settings)? {
            return Err(err());
        }
        Ok(settings)
    }
    fn bytes(&self) -> io::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|_| err())
    }
    fn validate(&self, fake: bool) -> io::Result<()> {
        self.identity.validate()?;
        self.settings()?;
        if self.version != 3
            || self.nonce == [0; 32]
            || !self.output.is_absolute()
            || self.output.as_os_str().len() > 300
            || self.fixture.is_some() != fake
        {
            return Err(err());
        }
        Ok(())
    }
}
fn frame_contract(b: &Bootstrap, sequence: u64) -> io::Result<crate::sensor_contract::Acquisition> {
    crate::sensor_contract::Acquisition::new(b.settings()?, b.nonce, b.settings_generation, sequence).map(|a| a.with_startup(b.startup_intent)).map_err(|_| err())
}
fn frame_contract_hash(b: &Bootstrap, sequence: u64) -> io::Result<[u8;32]> {
    os::digest(&serde_json::to_vec(&frame_contract(b,sequence)?).map_err(|_|err())?)
}
fn frame_binding_valid(raw: &[u8;128], b: &Bootstrap, sequence:u64) -> bool {
    Some(raw[80..112].try_into().unwrap()) == frame_contract_hash(b,sequence).ok()
        && u64::from_le_bytes(raw[112..120].try_into().unwrap()) == b.settings_generation
        && raw[120..].iter().all(|v|*v==0)
}
fn settings_hash(s: Settings) -> io::Result<[u8; 32]> {
    os::digest(
        format!(
            "settings-v2-geometry1:{}:{}:{}",
            s.mode.profile().id,
            s.exposure_rows,
            s.gain_eighths
        )
        .as_bytes(),
    )
}
fn bootstrap_send(w: &mut os::Writer, b: &Bootstrap) -> io::Result<[u8; 32]> {
    let mut wire_b = b.clone();
    if b.fixture == Some(Fixture::SettingsDigest) {
        wire_b.settings_digest[0] ^= 1;
    }
    let bytes = wire_b.bytes()?;
    if bytes.len() > 2012 {
        return Err(err());
    }
    let digest = os::digest(&bytes)?;
    let mut wire = [0u8; 2048];
    wire[..4].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
    wire[4..4 + bytes.len()].copy_from_slice(&bytes);
    wire[2016..].copy_from_slice(&digest);
    if b.fixture == Some(Fixture::BadBootstrap) {
        wire[2000] = 1;
    }
    if matches!(
        b.fixture,
        Some(Fixture::BootstrapStop | Fixture::BootstrapStall)
    ) {
        w.write(&wire[..128], Instant::now() + IO, &os::Cancel::default())?;
        thread::sleep(if b.fixture == Some(Fixture::BootstrapStall) {
            Duration::from_secs(10)
        } else {
            Duration::from_millis(500)
        });
        w.write(&wire[128..], Instant::now() + IO, &os::Cancel::default())?;
    } else {
        w.write(&wire, Instant::now() + IO, &os::Cancel::default())?;
    }
    Ok(digest)
}
fn bootstrap_receive(r: &mut os::Reader, fake: bool) -> io::Result<(Bootstrap, [u8; 32])> {
    let mut bytes = Vec::with_capacity(2048);
    let deadline = Instant::now() + IO;
    for _ in 0..16 {
        bytes.extend_from_slice(&r.record(deadline, &os::Cancel::default())?);
    }
    bootstrap_decode(&bytes, fake)
}
fn bootstrap_decode(bytes: &[u8], fake: bool) -> io::Result<(Bootstrap, [u8; 32])> {
    if bytes.len() != 2048 {
        return Err(err());
    }
    let n = u32::from_le_bytes(bytes[..4].try_into().map_err(|_| err())?) as usize;
    if n > 2012 || bytes[4 + n..2016].iter().any(|b| *b != 0) {
        return Err(err());
    }
    let digest = os::digest(&bytes[4..4 + n])?;
    if bytes[2016..] != digest {
        return Err(err());
    }
    let b: Bootstrap = serde_json::from_slice(&bytes[4..4 + n]).map_err(|_| err())?;
    b.validate(fake)?;
    if b.bytes()? != bytes[4..4 + n] {
        return Err(err());
    }
    Ok((b, digest))
}
fn packet(tag: u8, b: &Bootstrap, binding: [u8; 32]) -> [u8; 128] {
    let mut p = [0; 128];
    p[..4].copy_from_slice(&[b'C', b'O', 2, tag]);
    p[4..36].copy_from_slice(&b.nonce);
    p[36..68].copy_from_slice(&binding);
    p
}
fn valid(p: &[u8; 128], b: &Bootstrap, binding: [u8; 32]) -> bool {
    p[..3] == [b'C', b'O', 2] && p[4..36] == b.nonce && p[36..68] == binding
}

#[derive(Clone)]
pub struct Snapshot {
    pub startup_intent: Option<crate::startup::Intent>,
    pub diagnostics: Option<crate::acceptance_summary::Diagnostic>,
    pub diagnostic_invalid: bool,
    pub helper_phase: u8,
    pub starting: bool,
    /// Native deletion has been irrevocably authorized; retention is no longer promised.
    pub release_committed: bool,
    pub marker_deleted: bool,
    pub frame: Option<Frame>,
    pub frames: u64,
    pub heartbeats: u64,
    pub stop_observed: bool,
    pub terminal: Option<Terminal>,
    pub backend_error: Option<String>,
    pub causes: Accumulator,
    pub first_error: Option<String>,
    pub collected: bool,
    pub exit: Option<u32>,
    pub responding: bool,
    pub helper_fault: Option<crate::continuous::Fault>,
    pub proof_flags: u8,
    origin: Duration,
    owner_at: Duration,
    stop_at: Option<Duration>,
    previous: Duration,
    frozen: bool,
}
impl Snapshot {
    pub fn diagnostic_status(&self) -> Option<crate::startup::DiagnosticStatus> {
        self.startup_intent.map(|i| i.status(self.frames > 0, self.first_error.is_some() || self.helper_fault.is_some() || self.proof_flags & 16 != 0))
    }
    fn fault(&mut self, s: impl Into<String>) {
        self.fault_cause(Cause::unknown(), s);
    }
    fn fault_cause(&mut self, cause:Cause, s:impl Into<String>) {
        if self.frozen {
            return;
        }
        crate::diagnostic::report_fault(&mut self.causes,&mut self.first_error,cause,s);
    }
    fn parent_report(&mut self, report:crate::diagnostic::ParentReport) {
        if !self.frozen {crate::diagnostic::report_parent(&mut self.causes,&mut self.first_error,report);}
    }
    fn evaluate(&mut self, now: Duration) {
        if self.frozen {
            return;
        }
        if now < self.previous {
            self.fault_cause(Cause::parent(Stage::Clock,Reason::Malformed), "Parent monotonic clock regressed");
            if !self.release_committed {
                self.terminal = Some(Terminal::Unproved);
            }
        }
        self.previous = now;
        if now.saturating_sub(self.owner_at) >= IO {
            self.parent_report(crate::diagnostic::ParentReport::OwnerUnresponsive);
        }
        if self.frames == 0 && now.saturating_sub(self.origin) >= Duration::from_secs(60) {
            self.fault_cause(Cause::parent(Stage::Initialization,Reason::Deadline), "Parent initialization deadline");
        }
        if self.first_error.is_some() {
            self.stop_at.get_or_insert(now);
        }
        if self
            .stop_at
            .is_some_and(|at| now.saturating_sub(at) >= Duration::from_secs(8))
        {
            self.fault_cause(Cause::parent(Stage::Shutdown,Reason::Deadline), "Shutdown proof deadline");
            if !self.release_committed {
                self.terminal = Some(Terminal::Unproved);
            }
        }
    }
}
struct State {
    snapshot: Mutex<Snapshot>,
    stop: AtomicBool,
    clock: Arc<dyn runtime::Clock>,
    monitor_ready: AtomicBool,
    end: AtomicBool,
    owner_challenge: Mutex<Option<(u64, [u8; 32], Instant, bool)>>,
    frame_notify: Mutex<Option<thread::Thread>>,
}
impl State {
    fn new(clock: Arc<dyn runtime::Clock>) -> Arc<Self> {
        Arc::new(Self {
            snapshot: Mutex::new(Snapshot {
                startup_intent: None,
                diagnostics: None,
                diagnostic_invalid: false,
                helper_phase: 0,
                starting: true,
                release_committed: false,
                marker_deleted: false,
                frame: None,
                frames: 0,
                heartbeats: 0,
                stop_observed: false,
                terminal: None,
                backend_error: None,
                causes: Accumulator::default(),
                first_error: None,
                collected: false,
                exit: None,
                responding: false,
                helper_fault: None,
                proof_flags: 0,
                origin: clock.now(),
                owner_at: clock.now(),
                previous: clock.now(),
                stop_at: None,
                frozen: false,
            }),
            stop: AtomicBool::new(false),
            clock,
            monitor_ready: AtomicBool::new(false),
            end: AtomicBool::new(false),
            owner_challenge: Mutex::new(None),
            frame_notify: Mutex::new(None),
        })
    }
    fn owner_age(&self) -> Duration {
        let s = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        self.clock
            .now()
            .checked_sub(s.owner_at)
            .unwrap_or(Duration::MAX)
    }
}
/// One revision, one process. Drop requests STOP; it never waits, unlocks, starts
/// a replacement, or abandons the background collection owner. Call poll at
/// least once per two seconds on the lifecycle owner, including while stopping.
pub struct ContinuousSession {
    state: Arc<State>,
}
impl ContinuousSession {
    pub fn set_frame_notify(&self,t:thread::Thread) {*self.state.frame_notify.lock().unwrap_or_else(|e|e.into_inner())=Some(t);}

    pub fn start(identity: Identity, settings: Settings) -> io::Result<Self> {
        Self::start_generation(identity,settings,1)
    }
    pub fn start_generation(identity:Identity,settings:Settings,generation:u64) -> io::Result<Self> {
        let root = crate::coordination::user_root().map_err(io::Error::other)?;
        Self::start_inner(identity, settings, root, None,generation,crate::startup::Intent::Normal)
    }
    pub fn start_generation_intent(identity:Identity,settings:Settings,generation:u64,intent:crate::startup::Intent) -> io::Result<Self> {
        let root = crate::coordination::user_root().map_err(io::Error::other)?;
        Self::start_inner(identity,settings,root,None,generation,intent)
    }
    /// Fake roots are create-new private fixture directories, never user_root.
    pub fn start_fixture(root: &Path, fixture: Fixture) -> io::Result<Self> {
        Self::start_fixture_settings(root, fixture, Settings::default())
    }
    /// Explicit fake entry with real settings validation/digest and recipe.
    /// Does not select a physical backend or the real user interlock root.
    pub fn start_fixture_settings(
        root: &Path,
        fixture: Fixture,
        settings: Settings,
    ) -> io::Result<Self> {
        Self::start_fixture_generation(root,fixture,settings,1)
    }
    pub fn start_fixture_generation(root:&Path,fixture:Fixture,settings:Settings,generation:u64) -> io::Result<Self> {
        Self::start_fixture_intent(root,fixture,settings,generation,crate::startup::Intent::Diagnostics)
    }
    pub fn start_fixture_intent(root:&Path,fixture:Fixture,settings:Settings,generation:u64,intent:crate::startup::Intent) -> io::Result<Self> {
        Self::start_inner(
            Identity {
                bus: "PCIROOT(0)#PCI(FFFF)#USBROOT(0)".into(),
                ports: vec![1],
            },
            settings,
            root.to_owned(),
            Some(fixture),
            generation,
            intent,
        )
    }
    fn start_inner(
        identity: Identity,
        settings: Settings,
        root: PathBuf,
        fixture: Option<Fixture>,
        generation:u64,
        intent:crate::startup::Intent,
    ) -> io::Result<Self> {
        identity.validate()?;
        settings.validate().map_err(io::Error::other)?;
        if generation==0 {return Err(err());}
        let state = State::new(Arc::new(runtime::Monotonic::default()));
        state.snapshot.lock().unwrap().startup_intent = Some(intent);
        Self::start_background(state, move |owner, monitor| {
            startup(identity, settings, root, fixture, generation, intent, owner, monitor)
        })
    }
    fn start_background(
        state: Arc<State>,
        task: impl FnOnce(Arc<State>, thread::JoinHandle<()>) -> io::Result<()> + Send + 'static,
    ) -> io::Result<Self> {
        let owner = state.clone();
        spawn_startup(
            Box::new(move || {
                let monitor = match parent_monitor(owner.clone()) {
                    Ok(m) => m,
                    Err(e) => {
                        startup_failed(&owner, e);
                        return;
                    }
                };
                if let Err(e) = task(owner.clone(), monitor) {
                    startup_failed(&owner, e);
                }
            }),
            false,
        )?;
        Ok(Self { state })
    }
    pub fn poll(&self) -> Snapshot {
        // Record a missed interval BEFORE renewal, even if the monitor has not run.
        {
            let mut s = self
                .state
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let now = self.state.clock.now();
            s.evaluate(now);
            if !s.frozen {
                s.owner_at = now;
            }
            if s.stop_at.is_some() {
                self.state.stop.store(true, Ordering::Release);
            }
        }
        if let Some((_, _, issued, answered)) = self
            .state
            .owner_challenge
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            if issued.elapsed() < IO {
                *answered = true;
            }
        }
        self.state
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn stop(&self) {
        request_stop(&self.state);
    }
}
fn request_stop(state: &State) {
    let mut s = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
    if !s.frozen {
        s.stop_at.get_or_insert(state.clock.now());
    }
    state.stop.store(true, Ordering::Release);
}
fn spawn_startup(
    task: Box<dyn FnOnce() + Send>,
    inject_failure: bool,
) -> io::Result<thread::JoinHandle<()>> {
    if inject_failure {
        return Err(io::Error::other(
            "Injected startup thread creation failure; no child",
        ));
    }
    thread::Builder::new()
        .name("continuous-startup-owner".into())
        .spawn(task)
}

fn startup_failed(state: &State, error: io::Error) {
    state.end.store(true, Ordering::Release);
    let mut s = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
    s.fault(format!("Startup: {error}"));
    s.starting = false;
    s.terminal = Some(Terminal::Unproved);
    s.collected = true; // no child was launched on these Result error paths
    s.frozen = true;
}
fn startup(
    identity: Identity,
    settings: Settings,
    root: PathBuf,
    fixture: Option<Fixture>,
    generation:u64,
    intent: crate::startup::Intent,
    state: Arc<State>,
    monitor: thread::JoinHandle<()>,
) -> io::Result<()> {
    let result = (|| {
        let helper = std::env::current_exe()?.with_file_name("continuous-helper.exe");
        if !helper.is_file() {
            return Err(io::Error::new(io::ErrorKind::NotFound,
                "continuous-helper.exe is missing. Extract or reinstall the complete application before connecting."));
        }
        let legacy = helper.with_file_name("physical-camera.lock");
        if legacy.try_exists()? {
            return Err(io::Error::other(
                "Legacy interlock blocks continuous startup",
            ));
        }
        std::fs::create_dir_all(&root)?;
        let output = root.join(format!("session-{}", hex(&os::random()?[..12])));
        std::fs::create_dir(&output)?;
        let b = Bootstrap {
            version: 3,
            startup_intent: intent,
            nonce: os::random()?,
            identity,
            mode: settings.mode.profile().id.into(),
            geometry: crate::sensor_contract::Geometry::for_mode(settings.mode).map_err(|_| err())?,
            settings_generation: generation,
            exposure: settings.exposure_rows,
            gain: settings.gain_eighths,
            settings_digest: settings_hash(settings)?,
            output,
            fixture,
        };
        b.validate(fixture.is_some())?;
        // Same injective legacy device key: old files conservatively block.
        let path = root.join(format!("{}.lock", hex(b.identity.key().as_bytes())));
        let lock = os::RetainedInterlock::create_new(&path, &b.bytes()?)?;
        let child = os::launch_helper(&helper, fixture.is_some())?;
        state
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .starting = false;
        Ok((child, lock, b))
    })();
    match result {
        Ok((child, lock, b)) => {
            parent(child, lock, b, state, monitor);
            Ok(())
        }
        Err(e) => {
            state.end.store(true, Ordering::Release);
            let _ = monitor.join();
            Err(e)
        }
    }
}
impl Drop for ContinuousSession {
    fn drop(&mut self) {
        self.stop();
    }
}
fn fault_cause(state:&State,cause:Cause,message:impl Into<String>) {
 let mut s=state.snapshot.lock().unwrap_or_else(|e|e.into_inner());s.fault_cause(cause,message);
 s.stop_at.get_or_insert(state.clock.now());
 state.stop.store(true, Ordering::Release);
}
fn parent_report(state:&State, report:crate::diagnostic::ParentReport) {
 let mut s=state.snapshot.lock().unwrap_or_else(|e|e.into_inner());s.parent_report(report);
 s.stop_at.get_or_insert(state.clock.now());state.stop.store(true,Ordering::Release);
}
fn fault(state: &State, message: impl Into<String>) {
    let mut s = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
    s.fault(message);
    s.stop_at.get_or_insert(state.clock.now());
    state.stop.store(true, Ordering::Release);
}

fn parent_monitor(stop_state: Arc<State>) -> io::Result<thread::JoinHandle<()>> {
    thread::Builder::new()
        .name("continuous-parent-monitor".into())
        .spawn(move || {
            while !stop_state.end.load(Ordering::Acquire) {
                {
                    let mut s = stop_state
                        .snapshot
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    s.evaluate(stop_state.clock.now());
                    if s.stop_at.is_some() {
                        stop_state.stop.store(true, Ordering::Release);
                    }
                }
                stop_state.monitor_ready.store(true, Ordering::Release);
                thread::sleep(Duration::from_millis(20));
            }
        })
}

fn parent(
    mut p: os::Parent,
    mut lock: os::RetainedInterlock,
    b: Bootstrap,
    state: Arc<State>,
    monitor: thread::JoinHandle<()>,
) {
    let expected_binding = os::digest(&b.bytes().unwrap()).unwrap();
    let mut stop = p.stop;
    let stop_state = state.clone();
    let stop_nonce = b.nonce;
    let stop_writer = thread::Builder::new()
        .name("continuous-stop-writer".into())
        .spawn(move || {
            while !stop_state.end.load(Ordering::Acquire) {
                if stop_state.stop.load(Ordering::Acquire) {
                    let r = Record {
                        kind: Kind::Cancel,
                        session: stop_nonce,
                        binding: expected_binding,
                        sequence: 1,
                        challenge: [0; 32],
                    };
                    if let Err(error) =
                        stop.write(&r.encode(), Instant::now() + IO, &os::Cancel::default())
                    {
                        fault(&stop_state, format!("Priority STOP write: {error}"));
                    }
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            stop.close();
        });
    if let Err(e) = &stop_writer {
        fault(&state, format!("STOP writer spawn: {e}"));
    }
    let binding = match bootstrap_send(&mut p.bootstrap, &b) {
        Ok(v) => v,
        Err(e) => {
            fault(&state, format!("Bootstrap: {e}"));
            [0; 32]
        }
    };
    p.bootstrap.close();
    let mut challenge = p.challenge;
    let mut response = p.response;
    let mut receipt = p.receipt;
    let supervisor_state = state.clone();
    let nonce = b.nonce;
    let fixture = b.fixture;
    let supervisor_task = move || {
        let io_cancel = os::Cancel::default();
        let mut seq = 0u64;
        while !supervisor_state.end.load(Ordering::Acquire) {
            if supervisor_state.owner_age() >= IO {
                parent_report(&supervisor_state, crate::diagnostic::ParentReport::OwnerUnresponsive);
            }
            let raw = match challenge.record(Instant::now() + Duration::from_secs(3), &io_cancel) {
                Ok(v) => v,
                Err(_) => {
                    if !supervisor_state.stop.load(Ordering::Acquire)
                        && !supervisor_state.end.load(Ordering::Acquire)
                    {
                        parent_report(&supervisor_state, crate::diagnostic::ParentReport::ChallengeLost);
                    }
                    break;
                }
            };
            let r = match Record::decode(&raw) {
                Ok(r)
                    if r.kind == Kind::Response
                        && r.session == nonce
                        && r.binding == binding
                        && Some(r.sequence) == seq.checked_add(1) =>
                {
                    r
                }
                _ => {
                    parent_report(&supervisor_state, crate::diagnostic::ParentReport::ChallengeMalformed);
                    break;
                }
            };
            seq = r.sequence;
            let issued = Instant::now();
            *supervisor_state
                .owner_challenge
                .lock()
                .unwrap_or_else(|e| e.into_inner()) =
                Some((r.sequence, r.challenge, issued, false));
            if fixture == Some(Fixture::ResponseStop) {
                supervisor_state
                    .snapshot
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .responding = true;
                thread::sleep(Duration::from_millis(500));
            }
            let mut acknowledged = false;
            while issued.elapsed() < IO && !supervisor_state.end.load(Ordering::Acquire) {
                acknowledged = supervisor_state
                    .owner_challenge
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .as_ref()
                    .is_some_and(|(seq, random, at, answered)| {
                        *seq == r.sequence && *random == r.challenge && *at == issued && *answered
                    });
                if acknowledged {
                    break;
                }
                thread::sleep(Duration::from_millis(5));
            }
            supervisor_state
                .owner_challenge
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            if !acknowledged {
                parent_report(&supervisor_state, crate::diagnostic::ParentReport::ChallengeUnanswered);
                break;
            }
            // Response authorization is tied to the lifecycle owner, not producer
            // activity, file timestamps or this supervisor's own responsiveness.
            if supervisor_state.owner_age() >= IO {
                parent_report(&supervisor_state, crate::diagnostic::ParentReport::ChallengeStalled);
                break;
            }
            if fixture == Some(Fixture::LeaseLoss)
                && supervisor_state
                    .snapshot
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .helper_phase
                    >= 3
            {
                break;
            }
            if let Err(error) = response
                .write(&r.encode(), Instant::now() + IO, &io_cancel)
            {
                response_write_failed(&supervisor_state, &error);
                break;
            }
            supervisor_state
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .heartbeats = seq;
        }
        response.close();
        challenge.close();
    };
    let supervisor = if fixture == Some(Fixture::ParentThreadFail) {
        drop(supervisor_task);
        Err(io::Error::other(
            "Injected parent thread spawn failure after child creation",
        ))
    } else {
        thread::Builder::new()
            .name("continuous-parent-ipc".into())
            .spawn(supervisor_task)
    };
    if let Err(e) = &supervisor {
        fault(&state, format!("Parent IPC spawn: {e}"));
        state
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .terminal = Some(Terminal::Unproved);
    }
    let mut proof = None;
    let mut sequence = 0;
    let mut diagnostics = crate::acceptance_summary::Decoder::default();
    let mut measurement = None;
    loop {
        let raw = match receipt.record(
            Instant::now() + Duration::from_secs(70),
            &os::Cancel::default(),
        ) {
            Ok(v) => v,
            Err(_) => {
                fault_cause(&state, Cause::parent(Stage::Channel,Reason::Lost), "Missing terminal receipt");
                break;
            }
        };
        // Optional measurement never grants STOP/fault/cleanup authority. Ignore
        // unknown versions, stale session bindings and malformed optional data.
        if raw[3]==10 {
            measurement=if valid(&raw,&b,binding) {crate::measurement::decode(raw[68..].try_into().unwrap())} else {None};
            continue;
        }
        if raw[3]!=5 {measurement=None;}
        if !valid(&raw, &b, binding) {
            fault(&state, "Bad session receipt");
            break;
        }
        match raw[3] {
            9 => {
                // Diagnostics are separate from mandatory lifecycle authority.
                let invalid = diagnostics.push(&raw[68..]).is_err();
                let mut s = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
                s.diagnostic_invalid |= invalid;
            }
            8 => {
                let mut s = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
                if raw[68] <= s.helper_phase || raw[68] > 4 || raw[69..].iter().any(|b| *b != 0) {
                    s.fault("Invalid helper phase");
                } else {
                    s.helper_phase = raw[68];
                }
            }
            5 => {
                let seq = u64::from_le_bytes(raw[68..76].try_into().unwrap());
                let width = u16::from_le_bytes(raw[76..78].try_into().unwrap()) as u32;
                let height = u16::from_le_bytes(raw[78..80].try_into().unwrap()) as u32;
                let n = width as usize * height as usize;
                let expected = if b.fixture == Some(Fixture::TypedFrames) {
                    let (w,h)=b.settings().unwrap().dimensions(); (u32::from(w),u32::from(h))
                } else if matches!(
                    b.fixture,
                    Some(Fixture::FullFrames | Fixture::NoTelemetry | Fixture::BadTelemetry)
                ) {
                    (2048, 1536)
                } else if b.fixture == Some(Fixture::XgaFrames) {
                    (1024, 768)
                } else if b.fixture.is_some() {
                    (2, 2)
                } else {
                    let (w, h) = b.settings().unwrap().dimensions();
                    (w as u32, h as u32)
                };
                if (width, height) != expected
                    || seq <= sequence
                    || !frame_binding_valid(&raw,&b,seq)
                {
                    fault(&state, "Invalid frame header");
                    break;
                }
                let mut data = vec![0; n];
                let deadline = Instant::now() + IO;
                let mut failed = false;
                for chunk in data.chunks_mut(FRAME_CHUNK) {
                    if receipt
                        .read_exact(chunk, deadline, &os::Cancel::default())
                        .is_err()
                    {
                        failed = true;
                        break;
                    }
                }
                if failed {
                    fault(&state, "Incomplete frame");
                    break;
                }
                let mut f = Frame {
                    raw: Arc::new(data),
                    metadata: Metadata {
                        acquisition: if b.fixture.is_none() || b.fixture == Some(Fixture::TypedFrames) { Some(frame_contract(&b,seq).unwrap()) } else {None},
source: "Continuous session".into(),
                        simulated: b.fixture.is_some(),
                        sequence: seq,
                        measurement: None, unix_millis: 0,
                        width,
                        height,
                        format: "BayerGRBG8".into(),
                    },
                };
                if f.validate().is_err() {
                    fault(&state, "Invalid frame");
                    break;
                }
                // Bind only the immediately preceding optional observation to this exact frame.
                if let Some(mut o)=measurement.take().filter(|o|o.valid_for(&f.metadata)) {
                    o.received_at=Some(Instant::now());
                    f.metadata.unix_millis=u128::from(o.host_accepted_completion_unix_ms.unwrap_or(0));
                    f.metadata.measurement=Some(o);
                }
                sequence = seq;
                if b.fixture == Some(Fixture::WaitTimeout) && seq == 1 {
                    // The producer is demonstrably live and has received no
                    // STOP. Exercise an actual native WAIT_TIMEOUT; its process
                    // owner is retained for the later mandatory numeric wait.
                    let wait = p.process.wait(Duration::ZERO);
                    fault(
                        &state,
                        format!(
                            "Native zero-deadline child wait: {:?} (WAIT_TIMEOUT=258)",
                            wait.as_ref().err().map(io::Error::kind)
                        ),
                    );
                    state
                        .snapshot
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .terminal = Some(Terminal::Unproved);
                }
                let mut s = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
                s.frame = Some(f);
                s.frames = seq;
                drop(s);
                if let Some(t)=state.frame_notify.lock().unwrap_or_else(|e|e.into_inner()).as_ref() {t.unpark();}
            }
            7 => {
                if raw[68..].iter().any(|v| *v != 0) {
                    fault(&state, "Bad StopObserved");
                    break;
                }
                state
                    .snapshot
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .stop_observed = true;
            }
            6 => {
                {
                    let mut s = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
                    s.diagnostics = diagnostics.finish();
                    s.diagnostic_invalid |= diagnostics.invalid;
                }
                let mut s=state.snapshot.lock().unwrap_or_else(|e|e.into_inner());
                let Snapshot{causes,first_error,..}=&mut *s;
                if crate::diagnostic::report_terminal(causes,first_error,&raw).is_ok() {
                    proof=Some(raw[68]);s.helper_fault=decode_fault(raw[69]);
                    s.proof_flags=raw[68];s.backend_error=s.causes.text.clone();
                } else {
                    drop(s);
                    fault_cause(&state,Cause::parent(Stage::Terminal,Reason::Malformed),"Malformed terminal");
                }
                break;
            }
            _ => {
                fault(&state, "Unknown helper event");
                break;
            }
        }
    }
    if proof.is_some()
        && receipt
            .expect_eof(Instant::now() + IO, &os::Cancel::default())
            .is_err()
    {
        fault(&state, "Terminal not followed by EOF");
        proof = None;
    }
    request_stop(&state);
    receipt.close();
    // Numeric wait is mandatory. An unproved status never releases the file;
    // Collector retains the process even after an OS wait failure.
    let exit = match p.process.wait(Duration::from_secs(10)) {
        Ok(code) => Ok(code),
        Err(e) => {
            fault(
                &state,
                format!("Helper wait failed; collection still owned: {e}"),
            );
            state
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .terminal = Some(Terminal::Unproved);
            Ok::<u32, io::Error>(p.process.wait_retained())
        }
    };
    // Collect every parent pipe IO owner before making the interlock releasable.
    // The independent monitor remains active if any native join stalls.
    if let Ok(thread) = supervisor {
        if thread.join().is_err() {
            fault(&state, "Parent IPC panicked");
        }
    }
    if let Ok(thread) = stop_writer {
        if thread.join().is_err() {
            fault(&state, "STOP writer panicked");
        }
    }
    let collected = exit.is_ok();
    let unlock = {
        let mut s = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        commit_release(
            &mut s,
            state.clock.now(),
            proof,
            exit.ok(),
            b.fixture == Some(Fixture::ReapFail),
        )
    };
    if unlock {
        // Native metadata/delete never runs under the presentation mutex.
        // The monitor and poll remain responsive until this owner completes.
        let readonly_result = if b.fixture == Some(Fixture::UnlockFail) {
            let path = b
                .output
                .parent()
                .unwrap()
                .join(format!("{}.lock", hex(b.identity.key().as_bytes())));
            std::fs::metadata(&path).and_then(|m| {
                let mut permissions = m.permissions();
                permissions.set_readonly(true);
                std::fs::set_permissions(&path, permissions)
            })
        } else {
            Ok(())
        };
        let result = readonly_result
            .and_then(|_| lock.release_after_proof())
            .map_err(|error| format!("Exact-handle unlock failed: {error}"));
        finish_delete(&state, result);
    }
    // Publish completion only after the final native unlock outcome is visible.
    // Until then the presentation may report Unproved while this owner retains IO.
    state.end.store(true, Ordering::Release);
    if monitor.join().is_err() {
        fault(&state, "Parent monitor panicked");
    }
    freeze_collection(&state, collected);
    // lock Drop deliberately retains every incomplete session's marker.
}

fn response_write_failed(state: &State, error: &io::Error) {
    // STOP independently cancels helper IPC, which closes this reader before
    // publishing its terminal receipt. An already acknowledged response can
    // overlap that close. Only this expected pipe closure after STOP is benign;
    // the collector still requires the complete terminal/EOF/reap proof and
    // every existing fault/deadline check before authorizing exact-file release.
    let stopped_pipe = state.stop.load(Ordering::Acquire)
        && matches!(error.raw_os_error(), Some(109 | 232)); // BROKEN_PIPE / NO_DATA
    if !stopped_pipe {
        let mut cause=Cause::parent(Stage::Channel,Reason::Lost);
        cause.category=error.raw_os_error().and_then(|v|u32::try_from(v).ok());
        fault_cause(state, cause, "Response channel lost");
    }
}

fn finish_delete(state: &State, result: Result<(), String>) {
    let mut s = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
    s.evaluate(state.clock.now());
    match result {
        Ok(()) => {
            s.marker_deleted = true;
            s.terminal = Some(if s.first_error.is_none() {
                Terminal::CleanCancellation
            } else {
                Terminal::FailedClean
            });
        }
        Err(error) => {
            s.fault(error);
            s.terminal = Some(Terminal::Unproved);
        }
    }
}
fn freeze_collection(state: &State, collected: bool) {
    let mut s = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
    s.evaluate(state.clock.now());
    if s.marker_deleted && s.first_error.is_some() {
        s.terminal = Some(Terminal::FailedClean);
    }
    s.collected = collected;
    s.frozen = true;
}

// Serialized Holding -> Committed | Retained decision. Only the retained owner
// calls this; monitor/poll/fault use the same mutex. Committed cannot be revoked
// by a subsequently scheduled deadline or fault. Native completion is separate.
fn commit_release(
    s: &mut Snapshot,
    now: Duration,
    proof: Option<u8>,
    exit: Option<u32>,
    synthetic_reap: bool,
) -> bool {
    if s.release_committed || s.frozen {
        return false;
    }
    s.evaluate(now);
    s.exit = exit;
    let flags = proof.unwrap_or(0);
    if proof.is_none() {
        s.fault("Terminal receipt absent");
    }
    if flags & 7 != 7 {
        s.fault("Backend drain/cleanup/release unproved");
    }
    if s.exit != Some(0) {
        s.fault("Helper exit/reap unacceptable");
    }
    if synthetic_reap {
        s.fault("Injected reap acceptance failure");
    }
    // Once shutdown is declared Unproved it cannot be resurrected by late IO.
    let mut unlock = false;
    if flags & 7 == 7
        && s.exit == Some(0)
        && !synthetic_reap
        && s.terminal != Some(Terminal::Unproved)
    {
        if flags & 16 != 0 || flags & 8 == 0 || s.first_error.is_some() {
            s.fault("Helper failed after verified resource release; interlock retained");
            s.terminal = Some(Terminal::FailedClean);
        } else {
            // Irreversible linearization point. Proof, EOF, numeric collection,
            // all deadlines and all faults were evaluated under this same lock.
            s.release_committed = true;
            unlock = true;
        }
    } else {
        s.terminal = Some(Terminal::Unproved);
    }
    unlock
}

/// Safe dispatch receives only adopted typed capabilities. No raw-handle API.
pub fn helper_dispatch(mode: String, mut c: os::Child) -> i32 {
    let fake = match mode.as_str() {
        "session-fake" => true,
        "session-usb" => false,
        _ => return 78,
    };
    let early_stop = Arc::new(AtomicBool::new(false));
    let stopping = early_stop.clone();
    let stop_done = os::Cancel::default();
    let stop_cancel = stop_done.clone();
    let stop_target: Arc<Mutex<Option<runtime::Monitor>>> = Arc::new(Mutex::new(None));
    let watcher_target = stop_target.clone();
    let mut stop = c.stop;
    // Started before bootstrap. Any malformed/lost STOP channel cancels safely.
    let watcher = thread::spawn(move || {
        let result = stop.record(Instant::now() + Duration::from_secs(86400), &stop_cancel);
        stopping.store(true, Ordering::Release);
        if let Some(monitor) = watcher_target
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            monitor.stop();
        }
        result
    });
    let parsed = bootstrap_receive(&mut c.bootstrap, fake);
    c.bootstrap.close();
    let (b, binding) = match parsed {
        Ok(v) => v,
        Err(_) => {
            stop_done.cancel();
            let _ = watcher.join();
            return 78;
        }
    };
    // Qualification-only accelerated monotonic time makes the helper's exact
    // initialization boundary occur before the parent's outer 60-second bound.
    let clock: Arc<dyn runtime::Clock> = if b.fixture == Some(Fixture::InitStall) {
        Arc::new(FixtureInitializationClock(runtime::Monotonic::default()))
    } else {
        Arc::new(runtime::Monotonic::default())
    };
    let (owner, lease, monitor) = runtime::session(b.nonce, binding, clock.clone());
    *stop_target.lock().unwrap_or_else(|e| e.into_inner()) = Some(monitor.clone());
    if early_stop.load(Ordering::Acquire) {
        monitor.stop();
    }
    let (input, inbox) = runtime::ipc_mailbox();
    let supervisor = match runtime::SupervisorThread::spawn(lease, inbox) {
        Ok(v) => v,
        Err(_) => {
            stop_done.cancel();
            let _ = watcher.join();
            return 78;
        }
    };
    let end = Arc::new(AtomicBool::new(false));
    let ending = end.clone();
    let ipc_monitor = monitor.clone();
    let nonce = b.nonce;
    let fixture = b.fixture;
    let mut challenge = c.challenge;
    let mut response = c.response;
    let ipc = thread::spawn(move || {
        loop {
            if ending.load(Ordering::Acquire) || ipc_monitor.cancelled() {
                break;
            }
            let random = match os::random() {
                Ok(v) => v,
                Err(_) => break,
            };
            let (ack, accepted) = std::sync::mpsc::sync_channel(1);
            if input
                .try_send(runtime::SupervisorCommand::AcceptChallenge(random, ack))
                .is_err()
            {
                break;
            }
            let accepted = match accepted.recv_timeout(IO) {
                Ok(Ok(v)) => v,
                _ => break,
            };
            let remaining = IO.saturating_sub(clock.now().saturating_sub(accepted.accepted_at));
            if remaining.is_zero() {
                break;
            }
            let issued = Instant::now();
            let deadline = issued + remaining;
            let sequence = accepted.sequence;
            let r = Record {
                kind: Kind::Response,
                session: nonce,
                binding,
                sequence,
                challenge: accepted.challenge,
            };
            if challenge
                .write(&r.encode(), deadline, &os::Cancel::default())
                .is_err()
            {
                break;
            }
            let mut raw = match response.record(deadline, &os::Cancel::default()) {
                Ok(v) => v,
                Err(_) => break,
            };
            if fixture == Some(Fixture::Replay) && sequence == 2 {
                raw[72..80].copy_from_slice(&1u64.to_le_bytes());
            }
            if fixture == Some(Fixture::StaleSettings) {
                raw[40] ^= 1;
            }
            let (ack, consumed) = std::sync::mpsc::sync_channel(1);
            if input
                .try_send(runtime::SupervisorCommand::AcceptResponse(raw, ack))
                .is_err()
            {
                break;
            }
            // Do not issue another challenge while its predecessor's response
            // occupies the capacity-one inbox. Both handoffs are acknowledged.
            if !matches!(
                consumed.recv_timeout(deadline.saturating_duration_since(Instant::now())),
                Ok(Ok(true))
            ) {
                break;
            }
            let until = issued + Duration::from_secs(1);
            while Instant::now() < until
                && !ending.load(Ordering::Acquire)
                && !ipc_monitor.cancelled()
            {
                thread::sleep(Duration::from_millis(10));
            }
        }
        if !ipc_monitor.cancelled() && !ending.load(Ordering::Acquire) {
            ipc_monitor.pipe_lost();
        }
        challenge.close();
        response.close();
    });
    // No backend access until the real initial challenge response was accepted.
    let initial = Instant::now();
    while !owner_authorized(&owner, &monitor)
        && initial.elapsed() < IO
        && !early_stop.load(Ordering::Acquire)
    {
        thread::sleep(Duration::from_millis(5));
    }
    let mut receipt = c.receipt;
    let worker_monitor = monitor.clone();
    let worker_stop = early_stop.clone();
    let backend_b = Bootstrap {
        version: b.version,
        startup_intent: b.startup_intent,
        nonce: b.nonce,
        identity: b.identity.clone(),
        mode: b.mode.clone(),
        geometry: b.geometry.clone(),
        settings_generation: b.settings_generation,
        exposure: b.exposure,
        gain: b.gain,
        settings_digest: b.settings_digest,
        output: b.output.clone(),
        fixture: b.fixture,
    };
    let worker = thread::spawn(move || {
        if fixture == Some(Fixture::LeaseStop) {
            let at = Instant::now();
            while at.elapsed() < Duration::from_secs(5)
                && !worker_stop.load(Ordering::Acquire)
                && !worker_monitor.cancelled()
            {
                thread::sleep(Duration::from_millis(5));
            }
        }
        if worker_stop.load(Ordering::Acquire) || worker_monitor.cancelled() {
            return Outcome::empty_cancel();
        }
        if fake {
            fake_backend(&backend_b, &owner)
        } else {
            physical_backend(&backend_b, &owner)
        }
    });
    let mut stop_observed = false;
    let mut sender_error = false;
    let mut phase = 0;
    while !worker.is_finished() {
        let next = monitor.phase();
        if next > phase {
            let mut event = packet(8, &b, binding);
            event[68] = next;
            if receipt
                .write(&event, Instant::now() + IO, &os::Cancel::default())
                .is_err()
            {
                sender_error = true;
                monitor.stop();
            }
            phase = next;
        }
        if early_stop.load(Ordering::Acquire) {
            monitor.stop();
            if !stop_observed {
                stop_observed = true;
                if receipt
                    .write(
                        &packet(7, &b, binding),
                        Instant::now() + IO,
                        &os::Cancel::default(),
                    )
                    .is_err()
                {
                    sender_error = true;
                }
            }
        }
        if let Some(frame) = monitor.take_frame() {
            if frame.validate().is_err() || ((b.fixture.is_none() || b.fixture == Some(Fixture::TypedFrames)) && frame.metadata.acquisition.as_ref().is_none_or(|a| a != &frame_contract(&b,frame.metadata.sequence).unwrap())) {
                sender_error = true; monitor.stop(); continue;
            }
            let mut header = packet(5, &b, binding);
            header[68..76].copy_from_slice(&frame.metadata.sequence.to_le_bytes());
            header[76..78].copy_from_slice(&(frame.metadata.width as u16).to_le_bytes());
            header[78..80].copy_from_slice(&(frame.metadata.height as u16).to_le_bytes());
            let result = (|| -> io::Result<()> {
                header[80..112].copy_from_slice(&frame_contract_hash(&b,frame.metadata.sequence)?);
                header[112..120].copy_from_slice(&b.settings_generation.to_le_bytes());
                if let Some(o)=frame.metadata.measurement.as_ref().filter(|o|o.valid_for(&frame.metadata)) {
                    let mut observation=packet(10,&b,binding);
                    observation[68..].copy_from_slice(&crate::measurement::encode(o));
                    receipt.write(&observation,Instant::now()+IO,&os::Cancel::default())?;
                }
                receipt.write(&header, Instant::now() + IO, &os::Cancel::default())?;
                let deadline = Instant::now() + IO;
                for chunk in frame.raw.chunks(FRAME_CHUNK) {
                    receipt.write(chunk, deadline, &os::Cancel::default())?;
                }
                Ok(())
            })();
            if result.is_err() {
                sender_error = true;
                monitor.stop();
            }
        }
        monitor.wait_frame(Duration::from_millis(10));
    }
    let outcome = worker.join().unwrap_or(Outcome {
        diagnostics: None,
        pending: false,
        cleanup: false,
        released: false,
        cancelled: false,
        failed: true,
        first_error: Some("Backend panicked before release".into()),
    });
    if early_stop.load(Ordering::Acquire) && !stop_observed {
        if receipt
            .write(
                &packet(7, &b, binding),
                Instant::now() + IO,
                &os::Cancel::default(),
            )
            .is_err()
        {
            sender_error = true;
        }
    }
    end.store(true, Ordering::Release);
    monitor.stop();
    if ipc.join().is_err() {
        monitor.fault(crate::continuous::Fault::OwnerUnresponsive);
    }
    stop_done.cancel();
    let stop_record = watcher.join();
    // StopObserved is deliberately weaker than the resource-released receipt.
    let valid_stop = matches!(stop_record,Ok(Ok(raw)) if Record::decode(&raw).is_ok_and(|r|r.kind==Kind::Cancel&&r.session==b.nonce&&r.binding==binding&&r.sequence==1&&r.challenge==[0;32]));
    let mut terminal = packet(6, &b, binding);
    terminal[68] = outcome.flags(valid_stop, monitor.first_fault().is_some());
    terminal[69] = monitor.first_fault().map_or(0, |f| f as u8 + 1);
    write_terminal_error(&mut terminal, outcome.first_error.as_deref());
    // Backend has joined; its interface/device owners have already dropped.
    // No acquisition thread IO, and no diagnostic flag participates in flags().
    if let Some(d) = outcome
        .diagnostics
        .filter(|_| fixture != Some(Fixture::NoTelemetry))
    {
        let diagnostic_deadline = Instant::now() + IO;
        for i in 0..20 {
            let mut record = packet(9, &b, binding);
            record[68..].copy_from_slice(&crate::acceptance_summary::payload(&d, i));
            if fixture == Some(Fixture::BadTelemetry) && i == 19 {
                record[69] = 0;
            }
            if receipt
                .write(&record, diagnostic_deadline, &os::Cancel::default())
                .is_err()
            {
                sender_error = true;
                break;
            }
        }
    }
    if fixture == Some(Fixture::BadReceipt) {
        terminal[36] ^= 1;
    }
    if fixture != Some(Fixture::MissingReceipt) && !sender_error {
        if receipt
            .write(&terminal, Instant::now() + IO, &os::Cancel::default())
            .is_err()
        {
            sender_error = true;
        }
    }
    receipt.close();
    drop(supervisor);
    if fixture == Some(Fixture::NonzeroExit) {
        23
    } else if sender_error {
        24 // mandatory receipt transport failure cannot accompany exit-zero proof
    } else {
        0
    }
}
fn owner_authorized(owner: &runtime::OwnerPort, monitor: &runtime::Monitor) -> bool {
    // A read-only authorization query avoids treating startup waiting as an
    // unauthorized device attempt (checkpoint intentionally latches that fault).
    owner.authorized() || monitor.cancelled()
}
struct Outcome {
    diagnostics: Option<crate::acceptance_summary::Diagnostic>,
    pending: bool,
    cleanup: bool,
    released: bool,
    cancelled: bool,
    failed: bool,
    first_error: Option<String>,
}
impl Outcome {
    fn flags(&self, valid_stop: bool, fault: bool) -> u8 {
        u8::from(self.pending)
            | u8::from(self.cleanup) << 1
            | u8::from(self.released) << 2
            | u8::from(self.cancelled && valid_stop && !self.failed && !fault) << 3
            | u8::from(self.failed || fault || !valid_stop) << 4
    }
    fn empty_cancel() -> Self {
        Self {
            diagnostics: Some(crate::acceptance_summary::Diagnostic::default()),
            pending: true,
            cleanup: true,
            released: true,
            cancelled: true,
            failed: false,
            first_error: None,
        }
    }
}
fn fake_backend(b: &Bootstrap, owner: &runtime::OwnerPort) -> Outcome {
    let mut diagnostics = crate::acquisition_telemetry::Session::default();
    owner.phase(1);
    let fixture = b.fixture.unwrap();
    let start = Instant::now();
    let resource = Box::new([0u8; 64]);
    if fixture == Fixture::Crash {
        std::process::exit(29);
    }
    if fixture == Fixture::InitStall {
        while owner.checkpoint().is_ok() && start.elapsed() < Duration::from_secs(65) {
            thread::sleep(Duration::from_millis(10));
        }
    } else {
        // Same recipe and exact three five-second pattern gates, once only.
        let mut recipe = FakeRecipe {
            telemetry: &mut diagnostics,
            owner,
            gain: 8,
            patterns: 0,
        };
        let prepared =
            crate::acquisition_recipe::initialize_continuous_intent(&mut recipe, b.settings().unwrap(), b.startup_intent);
        if prepared.is_ok() && owner.enable_live().is_ok() {
            owner.phase(3);
            let mut live = crate::acquisition_telemetry::Telemetry::new(
                crate::acquisition_telemetry::Binding::new(b.settings().unwrap(), false),
            );
            let live_start = Instant::now();
            live.event(crate::acquisition_telemetry::Event::AcquisitionStart, 0);
            if fixture == Fixture::ProducerStall {
                thread::sleep(Duration::from_secs(7));
            }
            let mut sequence = 0;
            while owner.checkpoint().is_ok() && start.elapsed() < Duration::from_secs(85) {
                sequence += 1;
                if !matches!(fixture, Fixture::NoTransfer | Fixture::ProducerStall) {
                    let _ = owner.nonempty_transfer();
                }
                if !matches!(
                    fixture,
                    Fixture::NoTransfer | Fixture::NoFrame | Fixture::ProducerStall
                ) {
                    let _ = owner.complete_frame();
                    live.frame(live_start.elapsed().as_nanos().min(u64::MAX as u128) as u64);
                    let (width, height) = match fixture {
                        Fixture::FullFrames | Fixture::NoTelemetry | Fixture::BadTelemetry => {
                            (2048, 1536)
                        }
                        Fixture::XgaFrames => (1024, 768),
                        Fixture::TypedFrames => { let (w,h)=b.settings().unwrap().dimensions(); (u32::from(w),u32::from(h)) },
                        _ => (2, 2),
                    };
                    let raw = (0..width * height)
                        .map(|i| (i as u64 + sequence) as u8)
                        .collect();
                    let _ = owner.publish_frame(Frame {
                        raw: Arc::new(raw),
                        metadata: Metadata {
                            acquisition: if fixture == Fixture::TypedFrames {Some(frame_contract(b,sequence).unwrap())} else {None},
source: "No USB fixture".into(),
                            simulated: true,
                            sequence,
                            measurement: None, unix_millis: 0,
                            width,
                            height,
                            format: "BayerGRBG8".into(),
                        },
                    });
                }
                thread::sleep(Duration::from_millis(40));
            }
            live.event(
                crate::acquisition_telemetry::Event::AcquisitionEnd,
                live_start.elapsed().as_nanos().min(u64::MAX as u128) as u64,
            );
            // Generated FAKE frames own no assembly tail. Explicit empty provenance.
            live.tail_provenance = Some(Default::default());
            live.drain_returned_ok = Some(fixture != Fixture::DrainFail);
            diagnostics.records[3] = Some(live);
        }
    }
    // Explicit backend release barrier. Receipt flags originate only here, after
    // the fake resource is dropped, rather than from a parent expectation.
    owner.phase(4);
    if fixture == Fixture::ReleaseStall {
        thread::sleep(Duration::from_secs(10));
    }
    let pending = fixture != Fixture::DrainFail;
    let cleanup = pending && fixture != Fixture::CleanupFail;
    drop(resource);
    Outcome {
        diagnostics: Some(crate::acceptance_summary::Diagnostic::capture(&diagnostics)),
        pending,
        cleanup,
        released: true,
        cancelled: true,
        failed: !pending || !cleanup,
        first_error: if !pending {
            Some("Pending-zero drain failed".into())
        } else if !cleanup {
            Some("Mandatory cleanup/readbacks failed".into())
        } else {
            None
        },
    }
}
struct FakeRecipe<'a> {
    telemetry: &'a mut crate::acquisition_telemetry::Session,
    owner: &'a runtime::OwnerPort,
    gain: u16,
    patterns: usize,
}
impl crate::acquisition_recipe::Backend for FakeRecipe<'_> {
    fn checkpoint(&mut self) -> Result<(), String> {
        self.owner.checkpoint()
    }
    fn write(&mut self, a: u16, v: u16) -> Result<(), String> {
        self.checkpoint()?;
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
    fn settle(&mut self, d: Duration) -> Result<(), String> {
        let start = Instant::now();
        while start.elapsed() < d {
            self.checkpoint()?;
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
    fn progress(&mut self, _: crate::progress::Stage) {}
    fn pattern(&mut self, index: usize, _: &str, d: Duration) -> Result<(), String> {
        if index != self.patterns || d != Duration::from_secs(5) {
            return Err("Invalid pattern gate".into());
        }
        self.patterns += 1;
        let start = Instant::now();
        let result = self.settle(d);
        let mut t = crate::acquisition_telemetry::Telemetry::new(
            crate::acquisition_telemetry::Binding::new(Default::default(), true),
        );
        t.event(crate::acquisition_telemetry::Event::AcquisitionStart, 0);
        t.event(
            crate::acquisition_telemetry::Event::AcquisitionEnd,
            start.elapsed().as_nanos().min(u64::MAX as u128) as u64,
        );
        t.drain_returned_ok = Some(true);
        self.telemetry.records[index] = Some(t);
        result
    }
}

fn physical_backend(b: &Bootstrap, owner: &runtime::OwnerPort) -> Outcome {
    use nusb::MaybeFuture;
    let mut telemetry = crate::acquisition_telemetry::Session::default();
    let mut opened = false;
    let result = (|| -> Result<(), String> {
        owner.checkpoint()?;
        let mut devices = nusb::list_devices()
            .wait()
            .map_err(|e| crate::diagnostic::backend_text("@ID",e.to_string()))?
            .filter(|d| {
                d.vendor_id() == 0x0547
                    && d.product_id() == 0x4d33
                    && d.bus_id() == b.identity.bus
                    && d.port_chain() == b.identity.ports
                    && d.product_string() == Some("3.1M USB2.0 Camera")
                    && d.device_version() == 0
                    && d.serial_number().is_none()
            });
        let info = devices.next().ok_or("@ID Exact device missing")?;
        if devices.next().is_some() {
            return Err("@ID Ambiguous device".into());
        }
        owner.checkpoint()?;
        if let Some(error)=crate::driver_policy::connection_error(&info) {return Err(error.into());}
        let device = info.open().wait().map_err(|e| crate::diagnostic::backend_text("@OP",e.to_string()))?;
        opened = true;
        let config = device.active_configuration().map_err(|e| crate::diagnostic::backend_text("@DS",e.to_string()))?;
        const EXPECTED: &[u8] = &[
            9, 2, 32, 0, 1, 1, 0, 128, 50, 9, 4, 0, 0, 2, 255, 0, 0, 0, 7, 5, 129, 3, 64, 0, 0, 7,
            5, 130, 2, 0, 2, 0,
        ];
        if config.as_bytes() != EXPECTED {
            return Err("@DS Descriptor gate".into());
        }
        drop(config);
        owner.checkpoint()?;
        let interface = device
            .claim_interface(0)
            .wait()
            .map_err(|e| crate::diagnostic::backend_text("@CL",e.to_string()))?;
        let settings = b.settings().map_err(|e| crate::diagnostic::backend_text("@IN",e.to_string()))?;
        crate::physical::with_resource_owners(interface, device, |interface| {
            crate::physical::continuous_acquire(
                interface,
                settings.dimensions().0,
                settings.dimensions().1,
                &b.output.join("capture"),
                settings,
                |value| {
                    use nusb::transfer::{ControlOut, ControlType, Recipient};
                    interface
                        .control_out(
                            ControlOut {
                                control_type: ControlType::Vendor,
                                recipient: Recipient::Device,
                                request: 1,
                                value,
                                index: 0x0f,
                                data: &[],
                            },
                            Duration::from_millis(500),
                        )
                        .wait()
                        .map_err(|e| e.to_string())?;
                    thread::sleep(Duration::from_millis(20));
                    Ok(())
                },
                &mut telemetry,
                owner,
                crate::startup::Binding { intent:b.startup_intent, session:b.nonce, generation:b.settings_generation },
            )
        })
    })();
    physical_outcome(result, &telemetry, opened)
}

fn physical_outcome(
    result: Result<(), String>,
    telemetry: &crate::acquisition_telemetry::Session,
    opened: bool,
) -> Outcome {
    let pending = telemetry.records[..4]
        .iter()
        .flatten()
        .all(|t| t.drain_returned_ok == Some(true));
    let cleanup = telemetry.records[4]
        .as_ref()
        .is_some_and(|t| t.cleanup_returned_ok == Some(true));
    let cancelled = !crate::physical::quiescence_unproved(&telemetry.records[..4])
        && result
            .as_ref()
            .err()
            .is_some_and(|e| e == crate::physical::CANCELLED);
    // Fixed observations captured only after with_resource_owners returned.
    Outcome {
        diagnostics: Some(crate::acceptance_summary::Diagnostic::capture(telemetry)),
        pending,
        cleanup: cleanup || !opened,
        released: pending,
        cancelled,
        failed: result.is_err() && !cancelled,
        first_error: result.err().filter(|e| e != crate::physical::CANCELLED),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn v041_production_orchestration_physical_outcome_projection() {
        for run in crate::physical::v041_tests::unverified_runs() {
            let expected = run.result.as_ref().unwrap_err().clone();
            let outcome = physical_outcome(run.result, &run.session, true);
            assert!(!outcome.cleanup);
            assert_eq!(outcome.flags(true, false) & 2, 0);
            assert_ne!(outcome.flags(true, false) & 7, 7);
            assert_eq!(outcome.cancelled, expected == crate::physical::CANCELLED);
            assert_eq!(outcome.failed, expected != crate::physical::CANCELLED);
            assert_eq!(outcome.first_error, if expected == crate::physical::CANCELLED {None}else{Some(expected)});
        }
        for run in crate::physical::v041_tests::verified_runs() {
            let expected_cleanup = run.session.records[4].as_ref().unwrap().cleanup_returned_ok == Some(true);
            let expected_pending = !crate::physical::quiescence_unproved(&run.session.records[..4]);
            let expected_error = run.result.as_ref().err().cloned();
            let outcome = physical_outcome(run.result, &run.session, true);
            assert_eq!(outcome.cleanup, expected_cleanup);
            assert_eq!(outcome.pending, expected_pending);
            assert_eq!(outcome.released, expected_pending);
            assert_eq!(outcome.first_error, expected_error);
            assert_eq!(outcome.flags(true, false) & 2 != 0, expected_cleanup);
            if !expected_cleanup {assert_ne!(outcome.flags(true, false) & 7, 7);}
        }
    }
    #[test]
    fn startup_v3_rejects_old_missing_unknown_and_cross_intent_generation() {
        use crate::startup::Intent;
        for intent in [Intent::Normal,Intent::Diagnostics] {
            let mut b=sample();b.startup_intent=intent;
            assert!(bootstrap_decode(&envelope(&b.bytes().unwrap()),true).is_ok());
            let digest=os::digest(&b.bytes().unwrap()).unwrap();
            let mut h=packet(5,&b,digest);
            h[80..112].copy_from_slice(&frame_contract_hash(&b,1).unwrap());
            h[112..120].copy_from_slice(&b.settings_generation.to_le_bytes());
            assert!(frame_binding_valid(&h,&b,1));
            let mut changed=b.clone();changed.startup_intent=if intent==Intent::Normal {Intent::Diagnostics} else {Intent::Normal};
            assert_ne!(digest,os::digest(&changed.bytes().unwrap()).unwrap());
            assert!(!frame_binding_valid(&h,&changed,1));
            changed=b.clone();changed.settings_generation+=1;
            assert!(!frame_binding_valid(&h,&changed,1));
            for old in [1,2] { changed=b.clone();changed.version=old;assert!(bootstrap_decode(&envelope(&changed.bytes().unwrap()),true).is_err()); }
            let value=serde_json::to_value(&b).unwrap();
            let mut missing=value.clone();missing.as_object_mut().unwrap().remove("startup_intent");
            assert!(bootstrap_decode(&envelope(&serde_json::to_vec(&missing).unwrap()),true).is_err());
            let mut unknown=value;unknown["startup_intent"]="Normal".into();
            assert!(bootstrap_decode(&envelope(&serde_json::to_vec(&unknown).unwrap()),true).is_err());
        }
    }
    #[test]
    fn two_mode_bootstrap_geometry_and_frame_binding_rejections() {
        let b=sample();b.validate(true).unwrap();
        for i in 0..5 { let mut bad=b.clone();match i {
            0=>bad.geometry.version=99,1=>bad.geometry.cfa="RGGB".into(),
            2=>bad.geometry.source_window[2]=1024,3=>bad.mode="xga".into(),_=>bad.settings_generation=0,
        }; assert!(bootstrap_decode(&envelope(&bad.bytes().unwrap()),true).is_err()); }
        let digest=os::digest(&b.bytes().unwrap()).unwrap();let mut h=packet(5,&b,digest);
        h[80..112].copy_from_slice(&frame_contract_hash(&b,1).unwrap());h[112..120].copy_from_slice(&1u64.to_le_bytes());
        assert!(valid(&h,&b,digest));assert!(frame_binding_valid(&h,&b,1));
        assert!(!frame_binding_valid(&h,&b,2));
        let mut changed=b.clone();changed.nonce[0]^=1;assert!(!frame_binding_valid(&h,&changed,1));
        changed=b.clone();changed.mode=Mode::Full.profile().id.into();changed.geometry=crate::sensor_contract::Geometry::for_mode(Mode::Full).unwrap();
        changed.settings_digest=settings_hash(changed.settings().unwrap_or(Settings {mode:Mode::Full,..Default::default()})).unwrap();
        assert!(!frame_binding_valid(&h,&changed,1)); // stale Full/skip contract cannot cross
        for i in 80..128 {let mut bad=h;bad[i]^=1;assert!(!frame_binding_valid(&bad,&b,1));}
        let mut old=h;old[2]=1;assert!(!valid(&old,&b,digest));
        let old=packet(5,&b,digest);assert!(!frame_binding_valid(&old,&b,1)); // dimension-only old helper
    }
    #[test]
    fn diagnostic_binding_covers_session_profile_settings_and_generation() {
        let b = sample();
        let binding = os::digest(&b.bytes().unwrap()).unwrap();
        let mut p = packet(9, &b, binding);
        assert!(valid(&p, &b, binding));
        p[4] ^= 1;
        assert!(!valid(&p, &b, binding));
        p[4] ^= 1;
        for change in 0..3 {
            let mut changed = b.clone();
            match change {
                0 => changed.mode = Mode::Full.profile().id.into(),
                1 => changed.exposure += 1,
                _ => changed.nonce[0] ^= 1,
            }
            if change == 0 && changed.mode == b.mode {
                changed.mode = Mode::FullFieldSkip.profile().id.into();
            }
            let hash = os::digest(&changed.bytes().unwrap()).unwrap();
            assert!(!valid(&p, &changed, hash));
        }
        let mut outcome = Outcome::empty_cancel();
        let flags = outcome.flags(true, false);
        outcome.diagnostics = None;
        assert_eq!(outcome.flags(true, false), flags);
        outcome.diagnostics = Some(Default::default());
        assert_eq!(outcome.flags(true, false), flags);
        // An absent observation cannot supply the mandatory terminal proof.
        let state = State::new(Arc::new(runtime::Monotonic::default()));
        let mut s = state.snapshot.lock().unwrap();
        assert!(!commit_release(
            &mut s,
            state.clock.now(),
            None,
            Some(0),
            false
        ));
    }
    use std::sync::atomic::AtomicU64;
    #[test]
    fn v037_native_snapshot_supervisor_evaluate_first() {
      use crate::diagnostic::ParentReport as P;
      for evaluate_first in [false,true] {
       let state=State::new(Arc::new(FakeClock(AtomicU64::new(0))));
       if evaluate_first {state.snapshot.lock().unwrap().evaluate(IO);}
       parent_report(&state,P::OwnerUnresponsive);
       if !evaluate_first {state.snapshot.lock().unwrap().evaluate(IO);}
       let mut s=state.snapshot.lock().unwrap();let first=s.first_error.clone();
       let mut p=[0;128];p[69]=4;
       let Snapshot{causes,first_error,..}=&mut *s;
       crate::diagnostic::report_terminal(causes,first_error,&p).unwrap();
       assert_eq!(s.causes.first,Some(P::OwnerUnresponsive.cause()));assert_eq!(s.first_error,first);
      }
      for r in [P::ChallengeLost,P::ChallengeMalformed,P::ChallengeUnanswered,P::ChallengeStalled] {
       let state=State::new(Arc::new(FakeClock(AtomicU64::new(0))));parent_report(&state,r);
       let mut s=state.snapshot.lock().unwrap();s.evaluate(IO);assert_eq!(s.causes.first,Some(r.cause()));
      }
    }
    struct FakeClock(AtomicU64);
    impl runtime::Clock for FakeClock {
        fn now(&self) -> Duration {
            Duration::from_millis(self.0.load(Ordering::Acquire))
        }
    }
    #[test]
    fn continuous_stop_response_close_still_requires_mandatory_release_proof() {
        for code in [109, 232, 5, 121, 995] {
            for stopped in [false, true] {
                for (proof, exit, prior_fault, expired) in [
                    (Some(15), Some(0), false, false),
                    (None, Some(0), false, false),
                    (Some(11), Some(0), false, false),
                    (Some(31), Some(0), false, false),
                    (Some(7), Some(0), false, false),
                    (Some(15), Some(23), false, false),
                    (Some(15), Some(0), true, false),
                    (Some(15), Some(0), false, true),
                ] {
                    let state = State::new(Arc::new(FakeClock(AtomicU64::new(0))));
                    if stopped { request_stop(&state); }
                    if prior_fault { fault(&state, "prior lifecycle fault"); }
                    response_write_failed(&state, &io::Error::from_raw_os_error(code));
                    let mut s = state.snapshot.lock().unwrap();
                    let expected = stopped && matches!(code, 109 | 232)
                        && proof == Some(15) && exit == Some(0) && !prior_fault && !expired;
                    assert_eq!(commit_release(&mut s,
                        if expired { Duration::from_secs(9) } else { Duration::ZERO },
                        proof, exit, false), expected);
                    assert_eq!(s.release_committed, expected);
                    assert!(!s.marker_deleted); // only native exact-handle delete can set this
                    if prior_fault { assert_eq!(s.first_error.as_deref(), Some("prior lifecycle fault")); }
                }
            }
        }
    }
    fn until(test: impl Fn() -> bool) {
        let end = Instant::now() + IO;
        while !test() && Instant::now() < end {
            thread::yield_now();
        }
        assert!(test());
    }
    #[test]
    fn continuous_descheduled_monitor_never_moves_stop_or_erases_owner_gap() {
        let clock = Arc::new(FakeClock(AtomicU64::new(0)));
        let state = State::new(clock.clone());
        let session = ContinuousSession {
            state: state.clone(),
        };
        session.stop();
        clock.0.store(8000, Ordering::Release);
        let s = session.poll(); // monitor has NEVER run
        assert_eq!(s.stop_at, Some(Duration::ZERO));
        assert_eq!(s.terminal, Some(Terminal::Unproved));
        assert_eq!(
            s.first_error.as_deref(),
            Some("Lifecycle owner unresponsive")
        );
        assert!(!s.release_committed);
        assert!(!s.collected);
        assert_eq!(s.owner_at, Duration::from_secs(8));
    }
    #[test]
    fn continuous_starting_owner_barrier_poll_stop_and_spawn_failure() {
        let ran = Arc::new(AtomicBool::new(false));
        let child = ran.clone();
        assert!(
            spawn_startup(
                Box::new(move || {
                    child.store(true, Ordering::Release);
                }),
                true
            )
            .is_err()
        );
        assert!(!ran.load(Ordering::Acquire));
        let state = State::new(Arc::new(FakeClock(AtomicU64::new(0))));
        let (entered, ready) = std::sync::mpsc::sync_channel(1);
        let (release, barrier) = std::sync::mpsc::sync_channel(1);
        let session = ContinuousSession::start_background(state.clone(), move |owner, monitor| {
            entered.send(()).unwrap();
            barrier.recv().unwrap();
            assert!(owner.stop.load(Ordering::Acquire));
            owner.end.store(true, Ordering::Release);
            monitor.join().unwrap();
            Err(io::Error::other(
                "Injected delayed startup failure before launch",
            ))
        })
        .unwrap();
        ready.recv().unwrap();
        assert!(session.poll().starting);
        assert!(!session.poll().collected);
        session.stop();
        assert_eq!(session.poll().stop_at, Some(Duration::ZERO));
        release.send(()).unwrap();
        until(|| session.poll().collected);
        assert_eq!(session.poll().terminal, Some(Terminal::Unproved));
        assert!(
            session
                .poll()
                .first_error
                .unwrap()
                .contains("delayed startup")
        );
    }
    #[test]
    fn continuous_monitor_retired_before_frozen_snapshot() {
        let clock = Arc::new(FakeClock(AtomicU64::new(0)));
        let state = State::new(clock.clone());
        let monitor = parent_monitor(state.clone()).unwrap();
        until(|| state.monitor_ready.load(Ordering::Acquire));
        state.end.store(true, Ordering::Release);
        monitor.join().unwrap();
        let mut s = state.snapshot.lock().unwrap();
        s.frozen = true;
        s.collected = true;
        drop(s);
        clock.0.store(100000, Ordering::Release);
        let session = ContinuousSession { state };
        assert!(session.poll().first_error.is_none());
    }
    #[test]
    fn continuous_release_barriers_authoritative_commit_and_late_fault() {
        // No monitor runs. Each barrier advances the same production clock.
        for ms in [2000, 8000, 9000] {
            let state = State::new(Arc::new(FakeClock(AtomicU64::new(0))));
            let mut s = state.snapshot.lock().unwrap();
            s.stop_at = Some(Duration::ZERO);
            if ms >= 8000 {
                s.owner_at = Duration::from_millis(ms - 1);
            }
            assert!(!commit_release(
                &mut s,
                Duration::from_millis(ms),
                Some(15),
                Some(0),
                false
            ));
            assert!(!s.release_committed);
            assert!(!s.marker_deleted);
            assert!(s.first_error.is_some());
            assert_eq!(
                s.first_error.as_deref(),
                Some(if ms >= 8000 {
                    "Shutdown proof deadline"
                } else {
                    "Lifecycle owner unresponsive"
                })
            );
        }
        let state = State::new(Arc::new(FakeClock(AtomicU64::new(0))));
        let mut s = state.snapshot.lock().unwrap();
        s.stop_at = Some(Duration::ZERO);
        s.owner_at = Duration::from_secs(7);
        assert!(commit_release(
            &mut s,
            Duration::from_secs(7),
            Some(15),
            Some(0),
            false
        ));
        assert!(s.release_committed);
        drop(s); // barrier between commit and native delete: public mutex free
        fault(&state, "fault during committed delete");
        let mut s = state.snapshot.lock().unwrap();
        s.evaluate(Duration::from_secs(10));
        assert_ne!(s.terminal, Some(Terminal::Unproved));
        assert_eq!(
            s.first_error.as_deref(),
            Some("fault during committed delete")
        );
        assert!(!s.collected);
    }
    #[test]
    fn continuous_native_delete_barrier_late_success_error_and_publication() {
        for native_success in [false, true] {
            let clock = Arc::new(FakeClock(AtomicU64::new(0)));
            let state = State::new(clock.clone());
            let session = ContinuousSession {
                state: state.clone(),
            };
            session.stop();
            let (committed, commit_seen) = std::sync::mpsc::sync_channel(1);
            let (complete, native_barrier) = std::sync::mpsc::sync_channel(1);
            let worker_state = state.clone();
            let owner = thread::spawn(move || {
                {
                    let mut s = worker_state.snapshot.lock().unwrap();
                    assert!(commit_release(
                        &mut s,
                        worker_state.clock.now(),
                        Some(15),
                        Some(0),
                        false
                    ));
                }
                committed.send(()).unwrap();
                native_barrier.recv().unwrap(); // exact native-call completion barrier
                finish_delete(
                    &worker_state,
                    if native_success {
                        Ok(())
                    } else {
                        Err("native delete error".into())
                    },
                );
            });
            commit_seen.recv().unwrap();
            clock.0.store(9000, Ordering::Release);
            let during = session.poll();
            assert!(during.release_committed);
            assert!(!during.marker_deleted && !during.collected);
            fault(&state, "injected later fault");
            complete.send(()).unwrap();
            owner.join().unwrap();
            assert!(!session.poll().collected); // before final publication
            freeze_collection(&state, true); // production calls only after monitor join
            let final_snapshot = session.poll();
            assert!(final_snapshot.collected);
            assert_eq!(final_snapshot.marker_deleted, native_success);
            assert_eq!(
                final_snapshot.terminal,
                Some(if native_success {
                    Terminal::FailedClean
                } else {
                    Terminal::Unproved
                })
            );
            assert_eq!(
                final_snapshot.first_error.as_deref(),
                Some("Lifecycle owner unresponsive")
            );
            clock.0.store(100000, Ordering::Release);
            assert_eq!(session.poll().terminal, final_snapshot.terminal);
        }
    }
    #[test]
    fn continuous_monitor_and_owner_sample_clock_only_inside_snapshot_serialization() {
        struct AuditedClock(Mutex<Option<std::sync::Weak<State>>>);
        impl runtime::Clock for AuditedClock {
            fn now(&self) -> Duration {
                if let Some(state) = self.0.lock().unwrap().as_ref().and_then(|s| s.upgrade()) {
                    assert!(
                        state.snapshot.try_lock().is_err(),
                        "clock sampled outside decision mutex"
                    );
                }
                Duration::ZERO
            }
        }
        let clock = Arc::new(AuditedClock(Mutex::new(None)));
        let state = State::new(clock.clone());
        *clock.0.lock().unwrap() = Some(Arc::downgrade(&state));
        let monitor = parent_monitor(state.clone()).unwrap();
        until(|| state.monitor_ready.load(Ordering::Acquire));
        let session = ContinuousSession {
            state: state.clone(),
        };
        session.poll();
        session.stop();
        assert_eq!(state.owner_age(), Duration::ZERO);
        state.end.store(true, Ordering::Release);
        monitor.join().unwrap();
    }
    #[test]
    fn continuous_terminal_bounded_utf8_and_strict_proof_flags() {
        let mut p = [0u8; 128];
        p[68] = 23;
        write_terminal_error(&mut p, Some(&"失敗".repeat(50)));
        assert_eq!(terminal_error(&p).unwrap().unwrap().len(), 57);
        let mut bad = p;
        bad[70] = 58;
        assert!(terminal_error(&bad).is_err());
        let mut bad = p;
        bad[68] = 15;
        assert!(terminal_error(&bad).is_err());
        let mut p = [0; 128];
        p[68] = 15;
        assert_eq!(terminal_error(&p).unwrap(), None);
        p[127] = 1;
        assert!(terminal_error(&p).is_err());
    }
    fn sample() -> Bootstrap {
        let settings = Settings::default();
        Bootstrap {
            version: 3,
            startup_intent: crate::startup::Intent::Diagnostics,
            nonce: [1; 32],
            identity: Identity {
                bus: "fake".into(),
                ports: vec![1],
            },
            mode: settings.mode.profile().id.into(),
            geometry: crate::sensor_contract::Geometry::for_mode(settings.mode).unwrap(),
            settings_generation: 1,
            exposure: settings.exposure_rows,
            gain: settings.gain_eighths,
            settings_digest: settings_hash(settings).unwrap(),
            output: std::env::temp_dir().join("not-opened-bootstrap-validation"),
            fixture: Some(Fixture::Normal),
        }
    }
    fn envelope(payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; 2048];
        bytes[..4].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes[4..4 + payload.len()].copy_from_slice(payload);
        bytes[2016..].copy_from_slice(&os::digest(payload).unwrap());
        bytes
    }
    #[test]
    fn continuous_bootstrap_actual_parser_bounds_canonical_recompute_and_settings() {
        let b = sample();
        let payload = b.bytes().unwrap();
        let valid = envelope(&payload);
        assert!(bootstrap_decode(&valid, true).is_ok());
        for length in [0, 1, 127, 2047, 2049] {
            assert!(bootstrap_decode(&vec![0; length], true).is_err());
        }
        for index in 4 + payload.len()..2016 {
            let mut bad = valid.clone();
            bad[index] = 1;
            assert!(bootstrap_decode(&bad, true).is_err());
        }
        for index in 4..4 + payload.len() {
            let mut bad = valid.clone();
            bad[index] ^= 1;
            assert!(bootstrap_decode(&bad, true).is_err());
        }
        let mut whitespace = payload.clone();
        whitespace.push(b' ');
        assert!(bootstrap_decode(&envelope(&whitespace), true).is_err());
        let mut unknown: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        unknown["registers"] = serde_json::json!([1, 2]);
        assert!(bootstrap_decode(&envelope(&serde_json::to_vec(&unknown).unwrap()), true).is_err());
        let mut bad = b.clone();
        bad.settings_digest[0] ^= 1;
        assert!(bootstrap_decode(&envelope(&bad.bytes().unwrap()), true).is_err());
        let mut bad = b.clone();
        bad.gain = 33;
        assert!(bootstrap_decode(&envelope(&bad.bytes().unwrap()), true).is_err());
        let mut bad = b.clone();
        bad.version = 99;
        assert!(bootstrap_decode(&envelope(&bad.bytes().unwrap()), true).is_err());
        let mut bad = b.clone();
        bad.nonce = [0; 32];
        assert!(bootstrap_decode(&envelope(&bad.bytes().unwrap()), true).is_err());
        let mut bad = b.clone();
        bad.identity.bus = "x".repeat(65);
        assert!(bootstrap_decode(&envelope(&bad.bytes().unwrap()), true).is_err());
        let mut bad = b.clone();
        bad.output = "relative".into();
        assert!(bootstrap_decode(&envelope(&bad.bytes().unwrap()), true).is_err());
        assert!(bootstrap_decode(&valid, false).is_err());
        let mut windows = b.clone();
        windows.identity.bus = "PCIROOT(0)#PCI(0201)#PCI(0000)#USBROOT(0)".into();
        assert!(bootstrap_decode(&envelope(&windows.bytes().unwrap()), true).is_ok());
        // Both dispatch allowlist directions, without ever calling USB dispatch.
        windows.fixture = None;
        assert!(bootstrap_decode(&envelope(&windows.bytes().unwrap()), false).is_ok());
        assert!(bootstrap_decode(&envelope(&windows.bytes().unwrap()), true).is_err());
        for mode in Mode::ALL {
            let settings = Settings {
                mode,
                ..Settings::default()
            };
            windows.mode = mode.profile().id.into();
            windows.geometry = crate::sensor_contract::Geometry::for_mode(mode).unwrap();
            windows.settings_digest = settings_hash(settings).unwrap();
            assert!(bootstrap_decode(&envelope(&windows.bytes().unwrap()), false).is_ok());
        }
        windows.mode = "arbitrary-registers".into();
        assert!(bootstrap_decode(&envelope(&windows.bytes().unwrap()), false).is_err());
        for bus in [
            "",
            "PCIROOT(0)/USBROOT(0)",
            "PCIROOT(0)\\USBROOT(0)",
            "bus name",
            "バス",
        ] {
            let mut bad = b.clone();
            bad.identity.bus = bus.into();
            assert!(bootstrap_decode(&envelope(&bad.bytes().unwrap()), true).is_err());
        }
        let mut boundary = b.clone();
        boundary.identity.bus = "x".repeat(64);
        boundary.identity.ports = vec![1; 8];
        assert!(bootstrap_decode(&envelope(&boundary.bytes().unwrap()), true).is_ok());
        for ports in [vec![], vec![0], vec![1; 9]] {
            boundary.identity.ports = ports;
            assert!(bootstrap_decode(&envelope(&boundary.bytes().unwrap()), true).is_err());
        }
    }
}

#[cfg(test)]
pub(crate) fn assert_dual_failure_mapping(
    result: Result<(), String>,
    record: &mut Option<crate::acquisition_telemetry::Telemetry>,
    stuck: bool,
    stop: bool,
) {
    let mut session = crate::acquisition_telemetry::Session::default();
    session.records[0] = record.take();
    let outcome = physical_outcome(result, &session, true);
    assert_eq!(outcome.pending, !stuck);
    assert_eq!(outcome.released, !stuck);
    assert!(!outcome.cleanup);
    let state = State::new(Arc::new(runtime::Monotonic::default()));
    let mut snapshot = state.snapshot.lock().unwrap();
    snapshot.first_error = outcome.first_error.clone();
    let flags = outcome.flags(true, false);
    let root = std::path::PathBuf::from(
        std::env::var_os("DUAL_FAILURE_FAKE_ROOT").expect("private fake root"),
    );
    let marker = root.join(format!("fake-retained-{stop}-{stuck}"));
    std::fs::write(&marker, b"finite fake marker; no camera authority").unwrap();
    assert!(!commit_release(
        &mut snapshot,
        state.clock.now(),
        Some(flags),
        Some(0),
        false
    ));
    assert_eq!(snapshot.terminal, Some(Terminal::Unproved));
    assert!(!snapshot.release_committed);
    assert!(!snapshot.marker_deleted);
    assert_eq!(
        std::fs::read(&marker).unwrap(),
        b"finite fake marker; no camera authority"
    );
    *record = session.records[0].take();
    assert_eq!(outcome.cancelled, stop && !stuck);
    assert_eq!(outcome.failed, !stop || stuck);
    assert_eq!(
        outcome.first_error.as_deref(),
        if !stop {
            Some("device disconnected")
        } else if stuck {
            Some(crate::physical::QUIESCENCE_FAILED)
        } else {
            None
        }
    );
}

#[cfg(test)]
#[test]
fn physical_outcome_ordinary_success_stop_error_mapping() {
    for (result, cancelled, failed, flags) in [
        (Ok(()), false, false, 7),
        (Err(crate::physical::CANCELLED.to_owned()), true, false, 15),
        (Err("original exact fault".to_owned()), false, true, 23),
    ] {
        let mut session = crate::acquisition_telemetry::Session::default();
        for i in [0, 4] {
            let mut t = crate::acquisition_telemetry::Telemetry::new(
                crate::acquisition_telemetry::Binding::new(Default::default(), false),
            );
            t.drain_returned_ok = Some(true);
            t.cleanup_returned_ok = Some(true);
            session.records[i] = Some(t);
        }
        let original = result
            .clone()
            .err()
            .filter(|s| s != crate::physical::CANCELLED);
        let o = physical_outcome(result, &session, true);
        assert!(o.pending && o.cleanup && o.released);
        assert_eq!((o.cancelled, o.failed), (cancelled, failed));
        assert_eq!(o.first_error, original);
        assert_eq!(o.flags(true, false), flags);
    }
}
