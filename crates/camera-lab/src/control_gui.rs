//! Common control adapter for the opt-in `--gui-api` window.
//!
//! Each mutation is admitted by the same readiness predicate as its window
//! control and performed by the same App action. Receipts become terminal only
//! from Worker snapshots and export results observed on the GUI thread.
use super::*;
use camera_lab::external_control::{
    Cleanup, Kind, Refusal, Revocation, SourceKind, Status, cleanup_evidence, policy,
};
use camera_lab::session::State;
use kohdalab_control_protocol::{describe, status as common_status};
use serde_json::{Value, json};
use std::time::Instant;

pub(super) const METHODS: [&str; 11] = [
    "control.describe",
    "control.status",
    "control.config",
    "control.result",
    "control.acquire",
    "control.heartbeat",
    "control.release",
    "control.start_live",
    "control.stop",
    "control.capture",
    "control.capture_full",
];
const MUTATIONS: [&str; 7] = [
    "acquire",
    "heartbeat",
    "release",
    "start_live",
    "stop",
    "capture",
    "capture_full",
];
/// Protocol and identity fields that are not operation parameters.
const RESERVED: [&str; 11] = [
    "token",
    "method",
    "action",
    "control_protocol",
    "_control_method",
    "client",
    "client_id",
    "lease",
    "request_id",
    "run_id",
    "id",
];
pub(super) const EXTERNAL_IDLE: &str = "External control · Available";
pub(super) const EXTERNAL_ACTIVE: &str = "External control · Active";
pub(super) const EXTERNAL_BLOCKED: &str = "External control · Blocked";

fn ok(result: Value) -> Value {
    json!({"ok": true, "result": result})
}
fn refuse((code, message): Refusal) -> Value {
    json!({"ok": false, "error_code": code, "error": message})
}
fn source_kind(source: &Source) -> SourceKind {
    match source {
        Source::Physical(_) => SourceKind::Physical,
        _ => SourceKind::Software,
    }
}
fn params(native: &Value) -> Value {
    let mut p = native.as_object().cloned().unwrap_or_default();
    for key in RESERVED {
        p.remove(key);
    }
    Value::Object(p)
}
fn allowed(params: &Value, keys: &[&str]) -> Result<(), Refusal> {
    match params
        .as_object()
        .and_then(|o| o.keys().find(|k| !keys.contains(&k.as_str())))
    {
        Some(key) => Err((
            "validation",
            format!("Unsupported parameter {key:?} for this Camera operation"),
        )),
        None => Ok(()),
    }
}
/// A new absolute PNG/TIFF destination in an existing folder, with no
/// existing primary or sidecar. The save pipeline repeats the no-overwrite
/// check atomically; this one refuses before acquisition changes.
fn output_path(params: &Value) -> Result<PathBuf, Refusal> {
    let Some(text) = params.get("path").and_then(Value::as_str) else {
        return Err((
            "validation",
            "Capture requires path: an absolute new .png, .tif or .tiff file".into(),
        ));
    };
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err(("validation", "Output path must be absolute".into()));
    }
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("png" | "tif" | "tiff")) {
        return Err(("validation", "Use a .png, .tif or .tiff output path".into()));
    }
    if !path.parent().is_some_and(std::path::Path::is_dir) {
        return Err(("validation", "Output folder must already exist".into()));
    }
    if camera_lab::image_export::outputs(&path)
        .iter()
        .any(|p| std::fs::symlink_metadata(p).is_ok())
    {
        return Err((
            "storage",
            "Existing image or sidecars: choose a new name; overwrite is disabled".into(),
        ));
    }
    Ok(path)
}

impl App {
    /// Manual mutations are disabled while an external client holds the lease.
    /// Stop / Disconnect stay available in the window.
    pub(super) fn manual_locked(&self) -> bool {
        self.external.leased()
    }
    /// Enables "Capture image…" and admits `control.capture`.
    pub(super) fn capture_ready(&self) -> bool {
        self.frame.is_some()
            && (self.source.is_none() && !self.session.active
                || self.matching_revision == Some(self.revision)
                    && self.displayed_revision == Some(self.revision))
            && !self.busy
            && !self.closing
            && !self.product.busy()
    }
    /// Enables "Capture full sensor image…" and admits `control.capture_full`.
    pub(super) fn capture_full_ready(&self) -> bool {
        self.product.applied.is_some() && !self.busy && !self.closing && !self.product.busy()
    }
    fn acquisition_present(&self) -> bool {
        self.source.is_some() || self.session.active || self.product.busy()
    }
    fn current_source_kind(&self) -> Option<SourceKind> {
        self.source.as_ref().map(source_kind).or(self.last_source)
    }
    /// Worker or receipt evidence that forbids new external operations.
    pub(super) fn external_blocked(&self) -> Option<String> {
        self.external.blocked().map(str::to_owned).or_else(|| {
            (self.session.restart_disabled
                || self.session.lifecycle.as_ref().is_some_and(|l| l.unproved))
            .then(|| {
                "Worker cleanup is unproved and reconnection is disabled; follow the recovery guidance in the window".into()
            })
        })
    }
    pub(super) fn control_dispatch(&mut self, method: &str, native: &Value) -> Value {
        match method {
            "describe" => ok(self.control_describe()),
            "status" => ok(self.control_status()),
            // Draft plus applied state from this tick's Worker snapshot only.
            "config" => ok(camera_lab::control_api::config(
                self.settings,
                &self.session,
            )),
            "result" => match native
                .get("job_id")
                .and_then(Value::as_str)
                .and_then(|id| self.external.job(id))
            {
                Some(job) => ok(job.receipt()),
                None => refuse(("unknown_job", "No Camera job has this job_id".into())),
            },
            m if MUTATIONS.contains(&m) => self.control_mutation(m, native),
            _ => refuse((
                "unsupported",
                "Camera does not implement this control.* operation; see describe()".into(),
            )),
        }
    }
    fn control_describe(&self) -> Value {
        describe(
            "kohdalab-camera",
            env!("CARGO_PKG_VERSION"),
            1,
            &METHODS,
            json!({
                "supports_ownership": true,
                "exclusive_control": true,
                "stop_requires_lease": true,
                "heartbeat_seconds": camera_lab::external_control::HEARTBEAT.as_secs_f64(),
                "heartbeat_timeout_seconds": self.external.timeout.as_secs_f64(),
                "policy": policy(),
                "physical_auto_connect": false,
                "source_selection": "window_only",
                "start_live_sources": ["simulation", "usb"],
                "output": {"formats": ["png", "tif", "tiff"], "path": "absolute new file in an existing folder", "overwrite": false},
                "recording": "unsupported",
                "simulation_required_for_tests": true,
            }),
            json!({"gui_owned": !self.external.leased()}),
        )
    }
    fn control_status(&self) -> Value {
        let s = &self.session;
        let blocked = self.external_blocked();
        let phase = if blocked.is_some() {
            "error"
        } else {
            match s.state {
                State::Disconnected => "disconnected",
                State::Connecting | State::SettingsPending => "initializing",
                State::Streaming => "running",
                State::Stopping => "stopping",
                State::Error => "error",
            }
        };
        let cleanup = if phase == "error" {
            Cleanup::Unknown
        } else if phase == "stopping" || (self.source.is_none() && s.active) {
            Cleanup::Pending
        } else if s.active {
            // A running acquisition owes no cleanup until it is stopped.
            Cleanup::NotRequired
        } else {
            cleanup_evidence(self.last_source, s.lifecycle.as_ref(), s.restart_disabled)
        };
        common_status(
            phase,
            s.active || self.busy || self.product.busy(),
            self.external.ownership(Instant::now()),
            cleanup.id(),
            json!({
                "active": s.active,
                "message": s.message,
                "source_selected": self.source.is_some(),
                "selected_source": if self.picker_simulation { "simulation" } else { "usb" },
                "gui_state": self.primary_state(),
                "saving": self.busy,
                "full_capture": self.product.busy(),
                "blocked": blocked,
                "running_jobs": self.external.running().len(),
            }),
        )
    }
    fn control_mutation(&mut self, method: &str, native: &Value) -> Value {
        let Some(client) = native
            .get("client_id")
            .and_then(Value::as_str)
            .filter(|c| uuid::Uuid::parse_str(c).is_ok())
            .map(str::to_owned)
        else {
            return refuse((
                "invalid_request",
                "Mutations require a UUID client_id".into(),
            ));
        };
        let Some(request_id) = native
            .get("request_id")
            .and_then(Value::as_str)
            .filter(|r| !r.is_empty())
            .map(str::to_owned)
        else {
            return refuse(("invalid_request", "Mutations require a request_id".into()));
        };
        let lease = native.get("lease").and_then(Value::as_str);
        let params = params(native);
        if let Some(previous) = self
            .external
            .remembered(&request_id, &client, method, &params)
        {
            // Same request again: the original decision, never a new action.
            return previous.unwrap_or_else(refuse);
        }
        let now = Instant::now();
        let response = match method {
            "acquire" => match allowed(&params, &[]) {
                Err(e) => refuse(e),
                Ok(()) if self.closing => refuse(("busy", "The window is closing".into())),
                Ok(()) => {
                    if let Some(reason) = self.external_blocked() {
                        refuse(("cleanup_unconfirmed", reason))
                    } else if !self.external.leased()
                        && (self.session.state == State::Stopping || self.external.stop_pending())
                    {
                        refuse(("busy", "Wait for the current Stop to complete".into()))
                    } else {
                        self.external
                            .acquire(&client, lease, now)
                            .map_or_else(refuse, ok)
                    }
                }
            },
            "heartbeat" => self
                .external
                .heartbeat(&client, lease, now)
                .map_or_else(refuse, ok),
            _ => match self.external.check(&client, lease) {
                Err(e) => refuse(e),
                Ok(()) => match method {
                    "release" => self.control_release(&params, &request_id),
                    "start_live" => self.control_start_live(&params, &request_id),
                    "stop" => self.control_stop(&params, &request_id),
                    "capture" => self.control_capture(&params, &request_id, false),
                    _ => self.control_capture(&params, &request_id, true),
                },
            },
        };
        if method != "heartbeat" {
            self.external
                .remember(&request_id, &client, method, params, response.clone());
        }
        response
    }
    /// Revoke the lease and, if acquisition is present, request the same Stop
    /// as the window. Returns the job that tracks that Stop's evidence.
    fn end_lease(
        &mut self,
        why: Revocation,
        kind: Kind,
        request_id: Option<&str>,
    ) -> Option<String> {
        self.external.revoke(why);
        if !self.acquisition_present() {
            return None;
        }
        let source = self.current_source_kind();
        self.request(None);
        Some(
            self.external
                .create(kind, self.revision, source, request_id)
                .id
                .clone(),
        )
    }
    fn control_release(&mut self, params: &Value, request_id: &str) -> Value {
        if let Err(e) = allowed(params, &["reason"]) {
            return refuse(e);
        }
        match self.end_lease(Revocation::Released, Kind::Release, Some(request_id)) {
            Some(id) => {
                let job = self.external.job(&id).unwrap().receipt();
                ok(
                    json!({"released": true, "continuing": false, "cleanup": "pending",
                    "job_id": id, "job": job, "policy": "stop_acquisition"}),
                )
            }
            None => {
                let s = &self.session;
                let cleanup = if self.external_blocked().is_some() || s.state == State::Error {
                    Cleanup::Unknown
                } else {
                    cleanup_evidence(self.last_source, s.lifecycle.as_ref(), s.restart_disabled)
                };
                ok(json!({"released": true, "continuing": false, "cleanup": cleanup.id()}))
            }
        }
    }
    fn connect_refusal(&self) -> String {
        let s = &self.session;
        if self.closing {
            "The window is closing"
        } else if self.busy {
            "An image save is in progress"
        } else if self.connection_fault || s.restart_disabled || s.state == State::Error {
            "Camera operation failed; reconnection is disabled in the window"
        } else if s.active || s.state != State::Disconnected || s.requested_revision.is_some() {
            "Acquisition is running or stopping; stop it first"
        } else if !self.picker_simulation && self.scan_rx.is_some() {
            "Camera scan is in progress"
        } else if !self.picker_simulation && self.selected.is_none() {
            "Select a supported camera in the window"
        } else {
            "The selected camera is not eligible to connect; see the window"
        }
        .into()
    }
    fn control_start_live(&mut self, params: &Value, request_id: &str) -> Value {
        if let Err(e) = allowed(params, &["source"]) {
            return refuse(e);
        }
        let selected = if self.picker_simulation {
            "simulation"
        } else {
            "usb"
        };
        match params.get("source").and_then(Value::as_str) {
            Some(wanted @ ("simulation" | "usb")) if wanted != selected => {
                return refuse((
                    "validation",
                    format!(
                        "The window has {selected} selected; choose the source in the Camera window"
                    ),
                ));
            }
            Some("simulation" | "usb") => {}
            _ => {
                return refuse((
                    "validation",
                    "start_live requires source \"simulation\" or \"usb\", matching the window selection".into(),
                ));
            }
        }
        if let Some(reason) = self.external_blocked() {
            return refuse(("cleanup_unconfirmed", reason));
        }
        if !self.picker_connect_ready() {
            return refuse(("busy", self.connect_refusal()));
        }
        let before = self.revision;
        // Same selection, driver gate, startup intent and settings as Load camera.
        self.connect_picker();
        let Some(source) = self.source.as_ref().filter(|_| self.revision != before) else {
            return refuse((
                "device",
                self.control_error
                    .clone()
                    .unwrap_or_else(|| "Start was not accepted".into()),
            ));
        };
        let kind = source_kind(source);
        self.last_source = Some(kind);
        ok(self
            .external
            .create(Kind::StartLive, self.revision, Some(kind), Some(request_id))
            .receipt())
    }
    fn control_stop(&mut self, params: &Value, request_id: &str) -> Value {
        if params.get("job_id").is_some() {
            return refuse((
                "validation",
                "Camera Stop is application-wide; wait on the returned stop receipt instead of a job_id".into(),
            ));
        }
        if let Err(e) = allowed(params, &[]) {
            return refuse(e);
        }
        let source = self.current_source_kind();
        let present = self.acquisition_present();
        if present {
            // Same request as the window's Stop / Disconnect.
            self.request(None);
        }
        let job = self
            .external
            .create(Kind::Stop, self.revision, source, Some(request_id));
        // Nothing was requested, so the current snapshot is already valid.
        job.armed = !present;
        let id = job.id.clone();
        self.evaluate_external_jobs();
        ok(self.external.job(&id).unwrap().receipt())
    }
    fn control_capture(&mut self, params: &Value, request_id: &str, full: bool) -> Value {
        if let Err(e) = allowed(params, &["path"]) {
            return refuse(e);
        }
        let path = match output_path(params) {
            Ok(path) => path,
            Err(e) => return refuse(e),
        };
        if let Some(reason) = self.external_blocked() {
            return refuse(("cleanup_unconfirmed", reason));
        }
        let source = self.current_source_kind();
        if full {
            if !self.capture_full_ready() {
                return refuse((
                    "busy",
                    "Capture full sensor image is unavailable: it needs an applied Live frame with acquisition metadata (Simulation has none) and no save or capture in progress".into(),
                ));
            }
            // Same transaction as the button after its dialog: Full frame,
            // save, then the existing verified return to the applied Live settings.
            self.begin_full_with_origin(path.clone(), SaveOrigin::External);
            if !self.product.busy() {
                return refuse((
                    "device",
                    self.control_error
                        .clone()
                        .unwrap_or_else(|| "Full capture was not started".into()),
                ));
            }
        } else {
            if !self.capture_ready() {
                return refuse((
                    "busy",
                    "Capture image is unavailable: wait for a matching displayed frame and no save, Full capture or close in progress".into(),
                ));
            }
            let frame = self.frame.clone().unwrap();
            let recipe = self.displayed_recipe;
            self.capture_to(frame, path.clone(), recipe, SaveOrigin::External);
            if !self.busy {
                return refuse(("storage", self.save_result.clone()));
            }
        }
        let kind = if full {
            Kind::CaptureFull
        } else {
            Kind::Capture
        };
        let job = self
            .external
            .create(kind, self.revision, source, Some(request_id));
        job.path = Some(path);
        let (id, receipt) = (job.id.clone(), job.receipt());
        self.external.export_job = Some(id);
        ok(receipt)
    }
    /// Bind the single in-flight export result to its external job, with the
    /// files actually present as persistence evidence.
    pub(super) fn external_export_finished(&mut self, exported: bool) {
        let Some(id) = self.external.export_job.take() else {
            return;
        };
        let message = self.save_result.clone();
        let Some(job) = self.external.job_mut(&id) else {
            return;
        };
        let Some(path) = job.path.clone() else {
            return;
        };
        let all = camera_lab::image_export::outputs(&path);
        job.outputs = all
            .iter()
            .filter(|p| p.is_file())
            .map(|p| p.display().to_string())
            .collect();
        // Primary, RAW, metadata and the four standard sidecars; the
        // original-sensor TIFF exists only for Bayer sources.
        let persisted = all.iter().enumerate().all(|(i, p)| i == 3 || p.is_file());
        job.saved = Some(exported && persisted);
        if job.kind == Kind::Capture {
            if exported && persisted {
                job.finish(Status::Completed, Cleanup::NotRequired, message);
            } else {
                job.finish(Status::Error, Cleanup::NotRequired, message);
            }
        }
    }
    pub(super) fn expire_external_lease(&mut self, now: Instant) {
        if self.external.expired(now) {
            self.end_lease(Revocation::Expired, Kind::LeaseExpiry, None);
            self.status =
                "External control lease expired; Stop requested and control returned to this window"
                    .into();
        }
    }
    /// Stop evidence: `None` while the Worker still owns or is stopping.
    fn stop_outcome(&self, source: Option<SourceKind>) -> Option<(Status, Cleanup, String)> {
        let s = &self.session;
        if s.restart_disabled {
            return Some((
                Status::Error,
                Cleanup::Unknown,
                format!("Reconnection disabled; cleanup unproved: {}", s.message),
            ));
        }
        if s.active
            || self.product.busy()
            || self.source.is_some()
            || !matches!(s.state, State::Disconnected | State::Error)
        {
            return None;
        }
        let cleanup = cleanup_evidence(source, s.lifecycle.as_ref(), false);
        Some(if s.state == State::Error || cleanup == Cleanup::Unknown {
            (
                Status::Error,
                cleanup,
                format!("Stopped without confirmed cleanup: {}", s.message),
            )
        } else {
            (Status::Stopped, cleanup, s.message.clone())
        })
    }
    fn start_outcome(
        &self,
        job: &camera_lab::external_control::Job,
    ) -> Option<(Status, Cleanup, String)> {
        let s = &self.session;
        if s.restart_disabled {
            return self.stop_outcome(job.source);
        }
        if self.revision == job.revision && self.source.is_some() {
            if s.state == State::Error || self.connection_fault {
                return (!s.active).then(|| {
                    (
                        Status::Error,
                        cleanup_evidence(job.source, s.lifecycle.as_ref(), false),
                        s.message.clone(),
                    )
                });
            }
            return (self.matching_revision == Some(job.revision)).then(|| {
                (
                    Status::Completed,
                    Cleanup::NotRequired,
                    "Live: a valid frame for this request was received".into(),
                )
            });
        }
        if self.source.is_some() {
            return Some((
                Status::Stopped,
                Cleanup::NotRequired,
                "Superseded by a newer acquisition request".into(),
            ));
        }
        self.stop_outcome(job.source)
            .map(|(status, cleanup, message)| {
                (
                    if status == Status::Error {
                        status
                    } else {
                        Status::Stopped
                    },
                    cleanup,
                    format!("Stopped before Live: {message}"),
                )
            })
    }
    fn full_outcome(
        &self,
        job: &camera_lab::external_control::Job,
    ) -> Option<(Status, Cleanup, String)> {
        if self.product.busy()
            || (job.saved.is_none()
                && self.busy
                && self.external.export_job.as_deref() == Some(job.id.as_str()))
        {
            return None;
        }
        let restore = &self.product.restore_result;
        if restore.starts_with("Interrupted") {
            return self
                .stop_outcome(job.source)
                .map(|(status, cleanup, message)| {
                    (
                        if status == Status::Error {
                            status
                        } else {
                            Status::Stopped
                        },
                        cleanup,
                        format!("{restore}; {message}"),
                    )
                });
        }
        Some(match (job.saved, restore.starts_with("Original applied")) {
            (Some(true), true) => (
                Status::Completed,
                Cleanup::NotRequired,
                format!("{}; {restore}", self.save_result),
            ),
            (Some(true), false) => (
                Status::Error,
                Cleanup::NotRequired,
                format!("Full image saved; {restore}"),
            ),
            (Some(false), _) => (
                Status::Error,
                Cleanup::NotRequired,
                self.save_result.clone(),
            ),
            (None, _) => (
                Status::Error,
                Cleanup::NotRequired,
                format!("{}; {restore}", self.product.capture_result),
            ),
        })
    }
    /// Decide running receipts from this tick's evidence only.
    pub(super) fn evaluate_external_jobs(&mut self) {
        if let Some(source) = &self.source {
            self.last_source = Some(source_kind(source));
        }
        for id in self.external.running() {
            let Some(job) = self.external.job(&id).cloned() else {
                continue;
            };
            if !job.armed {
                self.external.job_mut(&id).unwrap().armed = true;
                continue;
            }
            let outcome = match job.kind {
                kind if kind.stops_acquisition() => self.stop_outcome(job.source),
                Kind::StartLive => self.start_outcome(&job),
                Kind::CaptureFull => self.full_outcome(&job),
                // Decided by the export result in external_export_finished.
                _ => None,
            };
            let Some((status, cleanup, message)) = outcome else {
                continue;
            };
            if cleanup == Cleanup::Unknown {
                self.external.block(format!(
                    "{} ended with unknown cleanup; follow the recovery guidance and restart the application",
                    job.kind.id()
                ));
            }
            if self.external.export_job.as_deref() == Some(id.as_str()) {
                self.external.export_job = None;
            }
            let restored = self.product.restore_result.starts_with("Original applied");
            let job = self.external.job_mut(&id).unwrap();
            if job.kind == Kind::CaptureFull {
                job.live_restored = Some(restored);
            }
            job.finish(status, cleanup, message);
        }
    }
    /// External control Stop: revoke the lease and use the window's Stop.
    pub(super) fn external_window_stop(&mut self) {
        self.external.revoke(Revocation::StoppedByGui);
        if self.acquisition_present() {
            self.request(None);
        }
    }
    /// Restrained `--gui-api` indicator with Stop and Take control while a
    /// client holds the lease. Laid out right-to-left by the caller.
    pub(super) fn external_control_ui(&mut self, ui: &mut egui::Ui) {
        if self.external.leased() {
            if ui
                .small_button("Take control")
                .on_hover_text(
                    "Revoke the external lease. Acquisition continues under this window's control.",
                )
                .clicked()
            {
                self.external.revoke(Revocation::TakenByGui);
                return;
            }
            if ui
                .small_button("Stop")
                .on_hover_text("Stop acquisition and revoke the external lease.")
                .clicked()
            {
                self.external_window_stop();
                return;
            }
            ui.weak(EXTERNAL_ACTIVE).on_hover_text(format!(
                "Local client {} holds control. Window settings, source selection and captures are locked; Stop / Disconnect stay available.",
                self.external.holder().unwrap_or("")
            ));
        } else if let Some(reason) = self.external_blocked() {
            ui.weak(EXTERNAL_BLOCKED).on_hover_text(reason);
        } else {
            let last = match self.external.last_revocation {
                Some(Revocation::Expired) => " The last lease expired; Stop was requested.",
                _ => "",
            };
            ui.weak(EXTERNAL_IDLE).on_hover_text(format!(
                "Started with --gui-api. A local client may acquire a lease to start/stop Live and capture; this window keeps Stop, Take control and its existing safety checks.{last}"
            ));
        }
    }
}
