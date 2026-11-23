//! Common-protocol ownership, request de-duplication and job receipts for the
//! opt-in GUI adapter.
//!
//! This module has no transport, Worker, filesystem or device authority. The
//! eframe App performs every action through its existing GUI paths and only
//! records here who may ask for them and what evidence it later observed.
use kohdalab_control_protocol::receipt;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    path::PathBuf,
    time::{Duration, Instant},
};

/// Interval advertised to clients; the SDK accepts 0.1–2 s.
pub const HEARTBEAT: Duration = Duration::from_secs(1);
/// A lease without a processed heartbeat for this long is revoked.
pub const LEASE_TIMEOUT: Duration = Duration::from_secs(5);
const JOBS: usize = 64;
const REQUESTS: usize = 128;
const REVOKED: usize = 16;

/// Why a lease ended. Every variant is reported to a late caller as `revoked`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Revocation {
    Released,
    Expired,
    TakenByGui,
    StoppedByGui,
    Closing,
}
impl Revocation {
    pub fn id(self) -> &'static str {
        match self {
            Self::Released => "released",
            Self::Expired => "expired",
            Self::TakenByGui => "taken_by_gui",
            Self::StoppedByGui => "stopped_by_gui",
            Self::Closing => "closing",
        }
    }
    fn message(self) -> &'static str {
        match self {
            Self::Released => "Lease was released; acquire control again",
            Self::Expired => {
                "Lease expired without a heartbeat; the GUI requested Stop and reclaimed control"
            }
            Self::TakenByGui => "Control was taken in the Camera window",
            Self::StoppedByGui => "The Camera window stopped external control",
            Self::Closing => "The Camera window is closing",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    StartLive,
    Stop,
    Capture,
    CaptureFull,
    /// Stop requested by `control.release`.
    Release,
    /// Stop requested by heartbeat expiry.
    LeaseExpiry,
}
impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Self::StartLive => "start_live",
            Self::Stop => "stop",
            Self::Capture => "capture",
            Self::CaptureFull => "capture_full",
            Self::Release => "release",
            Self::LeaseExpiry => "lease_expiry",
        }
    }
    /// Jobs whose terminal state is decided by acquisition-stop evidence.
    pub fn stops_acquisition(self) -> bool {
        matches!(self, Self::Stop | Self::Release | Self::LeaseExpiry)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    Completed,
    Stopped,
    Error,
}
impl Status {
    fn id(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cleanup {
    Verified,
    Pending,
    NotRequired,
    Unknown,
}
impl Cleanup {
    pub fn id(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Pending => "pending",
            Self::NotRequired => "not_required",
            Self::Unknown => "unknown",
        }
    }
}

/// What kind of acquisition owner a job touched; decides cleanup evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    /// USB device: verified only from collected Worker lifecycle evidence.
    Physical,
    /// In-process software simulation or explicit fake fixture.
    Software,
}

/// Cleanup claim allowed by the Worker evidence available for `source`.
///
/// A physical owner is `verified` only when its lifecycle reports collection
/// without unproved cleanup; absent or incomplete evidence is `unknown`.
pub fn cleanup_evidence(
    source: Option<SourceKind>,
    lifecycle: Option<&crate::session::Lifecycle>,
    restart_disabled: bool,
) -> Cleanup {
    if restart_disabled || lifecycle.is_some_and(|l| l.unproved) {
        return Cleanup::Unknown;
    }
    match (lifecycle, source) {
        (Some(l), _) if l.collected => Cleanup::Verified,
        (Some(_), _) => Cleanup::Unknown,
        (None, Some(SourceKind::Physical)) => Cleanup::Unknown,
        (None, _) => Cleanup::NotRequired,
    }
}

#[derive(Clone, Debug)]
pub struct Job {
    pub id: String,
    pub kind: Kind,
    pub status: Status,
    pub cleanup: Cleanup,
    pub revision: u64,
    pub source: Option<SourceKind>,
    pub path: Option<PathBuf>,
    /// Export pipeline result for capture jobs; `None` until reported.
    pub saved: Option<bool>,
    pub live_restored: Option<bool>,
    pub outputs: Vec<String>,
    pub message: String,
    pub request_id: Option<String>,
    /// False until a Worker snapshot taken after this job's request exists.
    /// The GUI's current snapshot predates a request issued in the same tick.
    pub armed: bool,
}
impl Job {
    pub fn terminal(&self) -> bool {
        self.status != Status::Running
    }
    pub fn finish(&mut self, status: Status, cleanup: Cleanup, message: impl Into<String>) {
        if !self.terminal() {
            self.status = status;
            self.cleanup = cleanup;
            self.message = message.into();
        }
    }
    pub fn receipt(&self) -> Value {
        receipt(
            &self.id,
            self.kind.id(),
            self.status.id(),
            self.cleanup.id(),
            None,
            json!({
                "revision": self.revision,
                "source": self.source.map(|s| match s {
                    SourceKind::Physical => "physical",
                    SourceKind::Software => "software",
                }),
                "path": self.path,
                "saved": self.saved,
                "live_restored": self.live_restored,
                "outputs": self.outputs,
                "message": self.message,
                "request_id": self.request_id,
            }),
        )
    }
}

struct Lease {
    client_id: String,
    token: String,
    expires: Instant,
}
struct Remembered {
    request_id: String,
    client_id: String,
    method: String,
    params: Value,
    response: Value,
}

/// Refusal as `(application_code, message)`; categorized by the common crate.
pub type Refusal = (&'static str, String);

pub struct Controller {
    lease: Option<Lease>,
    revoked: VecDeque<(String, Revocation)>,
    jobs: VecDeque<Job>,
    requests: VecDeque<Remembered>,
    /// Heartbeat timeout; tests may shorten it.
    pub timeout: Duration,
    /// Last revocation shown in the External control UI.
    pub last_revocation: Option<Revocation>,
    /// Job awaiting the single in-flight export result, if it is external.
    pub export_job: Option<String>,
    /// Cleanup evidence ended unknown: no new external operation until restart.
    blocked: Option<String>,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            lease: None,
            revoked: VecDeque::new(),
            jobs: VecDeque::new(),
            requests: VecDeque::new(),
            timeout: LEASE_TIMEOUT,
            last_revocation: None,
            export_job: None,
            blocked: None,
        }
    }
}
impl Controller {
    pub fn leased(&self) -> bool {
        self.lease.is_some()
    }
    pub fn holder(&self) -> Option<&str> {
        self.lease.as_ref().map(|l| l.client_id.as_str())
    }
    pub fn needs_updates(&self) -> bool {
        self.leased() || self.jobs.iter().any(|j| !j.terminal())
    }
    pub fn stop_pending(&self) -> bool {
        self.jobs
            .iter()
            .any(|job| !job.terminal() && job.kind.stops_acquisition())
    }

    pub fn blocked(&self) -> Option<&str> {
        self.blocked.as_deref()
    }
    pub fn block(&mut self, reason: impl Into<String>) {
        self.blocked.get_or_insert_with(|| reason.into());
    }
    pub fn ownership(&self, now: Instant) -> Value {
        json!({
            "owned": self.leased(),
            "holder": if self.leased() { "external" } else { "gui" },
            "client_id": self.holder(),
            "lease_expires_in_seconds": self.lease.as_ref()
                .map(|l| l.expires.saturating_duration_since(now).as_secs_f64()),
            "heartbeat_seconds": HEARTBEAT.as_secs_f64(),
            "heartbeat_timeout_seconds": self.timeout.as_secs_f64(),
            "last_revocation": self.last_revocation.map(Revocation::id),
        })
    }
    /// Grant a new lease, or return the caller's own still-valid lease.
    pub fn acquire(
        &mut self,
        client_id: &str,
        presented: Option<&str>,
        now: Instant,
    ) -> Result<Value, Refusal> {
        if let Some(reason) = &self.blocked {
            return Err(("cleanup_unconfirmed", reason.clone()));
        }
        let holder = self
            .lease
            .as_ref()
            .map(|l| l.client_id == client_id && presented == Some(l.token.as_str()));
        match holder {
            Some(true) => {
                let timeout = self.timeout;
                self.lease.as_mut().unwrap().expires = now + timeout;
            }
            Some(false) => {
                return Err((
                    "ownership",
                    "Another client holds Camera control; the window can Take control".into(),
                ));
            }
            None => {
                self.lease = Some(Lease {
                    client_id: client_id.into(),
                    token: uuid::Uuid::new_v4().to_string(),
                    expires: now + self.timeout,
                });
                self.last_revocation = None;
            }
        }
        let lease = self.lease.as_ref().unwrap();
        Ok(json!({
            "lease": lease.token,
            "client_id": lease.client_id,
            "heartbeat_seconds": HEARTBEAT.as_secs_f64(),
            "heartbeat_timeout_seconds": self.timeout.as_secs_f64(),
            "policy": policy(),
        }))
    }
    /// Validate the caller's lease for a mutation; stale leases say why.
    pub fn check(&self, client_id: &str, presented: Option<&str>) -> Result<(), Refusal> {
        if let Some(l) = &self.lease
            && l.client_id == client_id
            && presented == Some(l.token.as_str())
        {
            return Ok(());
        }
        if let Some((_, why)) = presented.and_then(|p| self.revoked.iter().find(|(t, _)| t == p)) {
            return Err(("revoked", why.message().into()));
        }
        Err((
            "invalid_lease",
            if self.lease.is_some() {
                "Lease does not match the current Camera control holder"
            } else {
                "No Camera control lease is held; call acquire first"
            }
            .into(),
        ))
    }
    pub fn heartbeat(
        &mut self,
        client_id: &str,
        presented: Option<&str>,
        now: Instant,
    ) -> Result<Value, Refusal> {
        self.check(client_id, presented)?;
        let lease = self.lease.as_mut().unwrap();
        lease.expires = now + self.timeout;
        Ok(json!({"lease_expires_in_seconds": self.timeout.as_secs_f64()}))
    }
    /// End the current lease. Returns whether a lease was held.
    pub fn revoke(&mut self, why: Revocation) -> bool {
        let Some(lease) = self.lease.take() else {
            return false;
        };
        self.revoked.push_back((lease.token, why));
        while self.revoked.len() > REVOKED {
            self.revoked.pop_front();
        }
        self.last_revocation = Some(why);
        true
    }
    pub fn expired(&self, now: Instant) -> bool {
        self.lease.as_ref().is_some_and(|l| now >= l.expires)
    }
    /// A repeated request ID returns the original decision; never a new action.
    pub fn remembered(
        &self,
        request_id: &str,
        client_id: &str,
        method: &str,
        params: &Value,
    ) -> Option<Result<Value, Refusal>> {
        let r = self.requests.iter().find(|r| r.request_id == request_id)?;
        Some(
            if r.client_id == client_id && r.method == method && &r.params == params {
                Ok(r.response.clone())
            } else {
                Err((
                    "request_id_conflict",
                    "request_id was already used for a different request".into(),
                ))
            },
        )
    }
    pub fn remember(
        &mut self,
        request_id: &str,
        client_id: &str,
        method: &str,
        params: Value,
        response: Value,
    ) {
        self.requests.push_back(Remembered {
            request_id: request_id.into(),
            client_id: client_id.into(),
            method: method.into(),
            params,
            response,
        });
        while self.requests.len() > REQUESTS {
            self.requests.pop_front();
        }
    }
    pub fn create(
        &mut self,
        kind: Kind,
        revision: u64,
        source: Option<SourceKind>,
        request_id: Option<&str>,
    ) -> &mut Job {
        self.jobs.push_back(Job {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            status: Status::Running,
            cleanup: Cleanup::Pending,
            revision,
            source,
            path: None,
            saved: None,
            live_restored: None,
            outputs: Vec::new(),
            message: String::new(),
            request_id: request_id.map(str::to_owned),
            armed: false,
        });
        // Bound history by discarding the oldest terminal receipts only.
        while self.jobs.len() > JOBS {
            match self.jobs.iter().position(Job::terminal) {
                Some(i) => {
                    self.jobs.remove(i);
                }
                None => break,
            }
        }
        self.jobs.back_mut().unwrap()
    }
    pub fn job(&self, id: &str) -> Option<&Job> {
        self.jobs.iter().find(|j| j.id == id)
    }
    pub fn job_mut(&mut self, id: &str) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }
    pub fn running(&self) -> Vec<String> {
        self.jobs
            .iter()
            .filter(|j| !j.terminal())
            .map(|j| j.id.clone())
            .collect()
    }
}

/// The fixed Camera ownership policy reported by describe/acquire.
pub fn policy() -> Value {
    json!({
        "on_release": "stop_acquisition",
        "on_lease_expiry": "stop_acquisition",
        "on_take_control": "keep_acquisition_gui_owned",
        "on_window_stop": "stop_acquisition_and_revoke",
        "manual_stop": "always_available",
        "manual_mutations_while_leased": "disabled",
        "disconnect": "detected_only_as_heartbeat_expiry",
        "unknown_cleanup": "blocks_external_operations_until_restart",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lease_is_exclusive_renewable_and_stale_leases_report_revocation() {
        let mut c = Controller::default();
        let t = Instant::now();
        let first = c.acquire("a", None, t).unwrap();
        let lease = first["lease"].as_str().unwrap().to_owned();
        assert_eq!(c.acquire("b", None, t).unwrap_err().0, "ownership");
        // Same holder re-acquire is idempotent and renews the same lease.
        let again = c.acquire("a", Some(&lease), t).unwrap();
        assert_eq!(again["lease"], first["lease"]);
        // Acquire without presenting the lease is not the holder.
        assert_eq!(c.acquire("a", None, t).unwrap_err().0, "ownership");
        assert_eq!(c.check("a", Some("forged")).unwrap_err().0, "invalid_lease");
        assert_eq!(c.check("b", Some(&lease)).unwrap_err().0, "invalid_lease");
        c.heartbeat("a", Some(&lease), t + Duration::from_secs(4))
            .unwrap();
        assert!(!c.expired(t + Duration::from_secs(8)));
        assert!(c.expired(t + Duration::from_secs(9)));
        assert!(c.revoke(Revocation::Expired));
        let (code, message) = c.check("a", Some(&lease)).unwrap_err();
        assert_eq!(code, "revoked");
        assert!(message.contains("expired"));
        assert_eq!(c.heartbeat("a", Some(&lease), t).unwrap_err().0, "revoked");
        assert_eq!(c.ownership(t)["owned"], false);
        assert_eq!(c.ownership(t)["last_revocation"], "expired");
        // A new client can acquire after revocation.
        assert!(c.acquire("b", None, t).is_ok());
    }
    #[test]
    fn duplicate_request_ids_replay_or_conflict_without_new_action() {
        let mut c = Controller::default();
        let p = json!({"path": "/tmp/x.png"});
        assert!(c.remembered("r", "a", "capture", &p).is_none());
        c.remember("r", "a", "capture", p.clone(), json!({"job_id": "j"}));
        assert_eq!(
            c.remembered("r", "a", "capture", &p).unwrap().unwrap()["job_id"],
            "j"
        );
        for (client, method, params) in [
            ("b", "capture", p.clone()),
            ("a", "capture_full", p.clone()),
            ("a", "capture", json!({"path": "/tmp/y.png"})),
        ] {
            assert_eq!(
                c.remembered("r", client, method, &params)
                    .unwrap()
                    .unwrap_err()
                    .0,
                "request_id_conflict"
            );
        }
    }
    #[test]
    fn blocked_cleanup_refuses_new_leases() {
        let mut c = Controller::default();
        c.block("cleanup unknown");
        assert_eq!(
            c.acquire("a", None, Instant::now()).unwrap_err().0,
            "cleanup_unconfirmed"
        );
    }
    #[test]
    fn cleanup_evidence_never_claims_unobserved_physical_cleanup() {
        use crate::session::Lifecycle;
        let collected = Lifecycle {
            collected: true,
            ..Default::default()
        };
        let uncollected = Lifecycle::default();
        let unproved = Lifecycle {
            collected: true,
            unproved: true,
            ..Default::default()
        };
        let p = Some(SourceKind::Physical);
        assert_eq!(
            cleanup_evidence(p, Some(&collected), false),
            Cleanup::Verified
        );
        assert_eq!(
            cleanup_evidence(p, Some(&uncollected), false),
            Cleanup::Unknown
        );
        assert_eq!(
            cleanup_evidence(p, Some(&unproved), false),
            Cleanup::Unknown
        );
        assert_eq!(cleanup_evidence(p, None, false), Cleanup::Unknown);
        assert_eq!(
            cleanup_evidence(p, Some(&collected), true),
            Cleanup::Unknown
        );
        let s = Some(SourceKind::Software);
        assert_eq!(cleanup_evidence(s, None, false), Cleanup::NotRequired);
        assert_eq!(cleanup_evidence(None, None, false), Cleanup::NotRequired);
        assert_eq!(cleanup_evidence(s, None, true), Cleanup::Unknown);
    }
    #[test]
    fn job_history_is_bounded_but_running_jobs_are_kept() {
        let mut c = Controller::default();
        let running = c.create(Kind::Stop, 1, None, None).id.clone();
        for _ in 0..(JOBS * 2) {
            let job = c.create(Kind::Capture, 1, None, None);
            job.finish(Status::Completed, Cleanup::NotRequired, "");
        }
        assert!(c.jobs.len() <= JOBS);
        assert!(c.job(&running).is_some());
        let r = c.job(&running).unwrap().receipt();
        assert_eq!(r["terminal"], false);
        assert_eq!(r["kind"], "stop");
        assert_eq!(r["linked_job_id"], Value::Null);
    }
}
