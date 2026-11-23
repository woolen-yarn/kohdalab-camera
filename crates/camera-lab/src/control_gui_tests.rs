//! Deterministic fake-GUI conformance tests for authenticated GUI mutation.
//! A software Worker factory stands in for acquisition; no device is opened.
use super::*;
use camera_lab::external_control::Revocation;
use camera_lab::session::{Event, Factory, Request, Transport};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Software owner emitting validated frames with acquisition metadata, so the
/// existing Full-capture transaction and Live restore run unchanged.
struct Fake {
    clean: bool,
}
struct Owner {
    measurement: camera_lab::measurement::Source,
    next: Instant,
    stop: bool,
    clean: bool,
    revision: u64,
    settings: Settings,
    sequence: u64,
    intent: camera_lab::startup::Intent,
}
impl Factory for Fake {
    fn open(&mut self, r: &Request) -> Result<Box<dyn Transport>, String> {
        self.open_generation(r, 1)
    }
    fn open_generation(
        &mut self,
        r: &Request,
        revision: u64,
    ) -> Result<Box<dyn Transport>, String> {
        Ok(Box::new(Owner {
            measurement: Default::default(),
            next: Instant::now(),
            stop: false,
            clean: self.clean,
            revision,
            settings: r.settings,
            sequence: 0,
            intent: r.startup_intent,
        }))
    }
}
impl Transport for Owner {
    fn stop(&mut self) -> Result<(), String> {
        self.stop = true;
        Ok(())
    }
    fn poll(&mut self) -> Result<Event, String> {
        if self.stop {
            return Ok(Event::Done { clean: self.clean });
        }
        if Instant::now() < self.next {
            return Ok(Event::Waiting);
        }
        self.next = Instant::now() + Duration::from_millis(20);
        self.sequence += 1;
        self.measurement.complete();
        let (w, h) = self.settings.dimensions();
        let mut a = camera_lab::sensor_contract::Acquisition::new(
            self.settings,
            [self.revision as u8; 32],
            self.revision,
            self.sequence,
        )?
        .with_startup(self.intent);
        a.request_revision = Some(self.revision);
        let mut frame = Frame {
            raw: Arc::new(vec![
                (self.sequence % 200 + 20) as u8;
                usize::from(w) * usize::from(h)
            ]),
            metadata: camera_lab::Metadata {
                acquisition: Some(a),
                source: "control conformance fake".into(),
                simulated: true,
                sequence: self.sequence,
                measurement: None,
                unix_millis: 0,
                width: w.into(),
                height: h.into(),
                format: "BayerGRBG8".into(),
            },
        };
        self.measurement.bind(&mut frame);
        Ok(Event::Frame(frame))
    }
}

fn app(clean: bool) -> (App, egui::Context, camera_lab::session::LifecycleGuardian) {
    let ctx = egui::Context::default();
    let worker = Worker::with_factory(Fake { clean });
    let guardian = worker.guardian();
    let mut app =
        App::with_recipe_exporter_worker(&ctx, camera_lab::image_export::save_recipe, worker);
    app.fake_only = true;
    app.picker_simulation = true;
    (app, ctx, guardian)
}
fn draw(app: &mut App, ctx: &egui::Context) {
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| app.render(ui));
}
fn until(app: &mut App, ctx: &egui::Context, what: &str, p: impl Fn(&App) -> bool) {
    let end = Instant::now() + Duration::from_secs(30);
    while !p(app) {
        assert!(
            Instant::now() < end,
            "{what}: {} / {}",
            app.primary_state(),
            app.outcome_detail()
        );
        draw(app, ctx);
        std::thread::sleep(Duration::from_millis(5));
    }
}
/// Dispatch exactly as process_control_api does after authentication.
fn api(
    app: &mut App,
    method: &str,
    client: &str,
    lease: Option<&str>,
    request_id: Option<&str>,
    mut body: Value,
) -> Value {
    body["control_protocol"] = json!(1);
    body["method"] = json!(format!("control.{method}"));
    body["client_id"] = json!(client);
    if let Some(lease) = lease {
        body["lease"] = json!(lease);
    }
    if let Some(id) = request_id {
        body["request_id"] = json!(id);
    }
    let native = prepare_common(&body).unwrap();
    app.control_dispatch(method, &native)
}
fn fresh() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn mutate(app: &mut App, method: &str, client: &str, lease: &str, body: Value) -> Value {
    api(app, method, client, Some(lease), Some(&fresh()), body)
}
fn refused(value: &Value) -> &str {
    assert_eq!(value["ok"], false, "{value}");
    value["error_code"].as_str().unwrap()
}
fn result(value: &Value) -> &Value {
    assert_eq!(value["ok"], true, "{value}");
    &value["result"]
}
fn receipt(app: &App, job: &str) -> Value {
    app.external.job(job).unwrap().receipt()
}
fn terminal(app: &mut App, ctx: &egui::Context, job: &str) -> Value {
    let id = job.to_owned();
    until(app, ctx, "terminal receipt", move |a| {
        a.external.job(&id).unwrap().terminal()
    });
    receipt(app, job)
}
fn acquire(app: &mut App, client: &str) -> String {
    let r = api(app, "acquire", client, None, Some(&fresh()), json!({}));
    result(&r)["lease"].as_str().unwrap().to_owned()
}
fn start_live(app: &mut App, ctx: &egui::Context, client: &str, lease: &str) -> Value {
    let r = mutate(
        app,
        "start_live",
        client,
        lease,
        json!({"source": "simulation"}),
    );
    let id = result(&r)["job_id"].as_str().unwrap().to_owned();
    assert_eq!(result(&r)["status"], "running");
    terminal(app, ctx, &id)
}
fn finish(mut app: App, ctx: egui::Context, mut guardian: camera_lab::session::LifecycleGuardian) {
    app.fake_until = Some(Instant::now());
    draw(&mut app, &ctx);
    until(&mut app, &ctx, "close", |a| a.worker.closed());
    guardian.wait();
}

#[test]
fn lease_admission_identity_and_describe_policy() {
    let (mut app, ctx, guardian) = app(true);
    let d = api(&mut app, "describe", &fresh(), None, None, json!({}));
    let d = result(&d);
    for method in control_gui::METHODS {
        assert!(d["methods"].as_array().unwrap().contains(&json!(method)));
    }
    assert_eq!(d["safety"]["supports_ownership"], true);
    assert_eq!(d["safety"]["stop_requires_lease"], true);
    assert_eq!(d["safety"]["policy"]["on_release"], "stop_acquisition");
    assert_eq!(d["safety"]["recording"], "unsupported");

    let a = fresh();
    let b = fresh();
    // No lease: every mutation is refused before any action.
    for method in ["start_live", "stop", "release", "capture", "heartbeat"] {
        let r = mutate(&mut app, method, &a, "nope", json!({}));
        assert_eq!(refused(&r), "invalid_lease", "{method}");
    }
    // Identity and request ID are required for mutations.
    let r = api(
        &mut app,
        "acquire",
        "not-a-uuid",
        None,
        Some(&fresh()),
        json!({}),
    );
    assert_eq!(refused(&r), "invalid_request");
    let r = api(&mut app, "acquire", &a, None, None, json!({}));
    assert_eq!(refused(&r), "invalid_request");
    let r = api(
        &mut app,
        "acquire",
        &a,
        None,
        Some(&fresh()),
        json!({"policy": "keep"}),
    );
    assert_eq!(refused(&r), "validation");

    let lease = acquire(&mut app, &a);
    assert!(app.manual_locked());
    let r = api(&mut app, "acquire", &b, None, Some(&fresh()), json!({}));
    assert_eq!(refused(&r), "ownership");
    let r = mutate(&mut app, "heartbeat", &b, &lease, json!({}));
    assert_eq!(refused(&r), "invalid_lease");
    let r = mutate(&mut app, "heartbeat", &a, "forged", json!({}));
    assert_eq!(refused(&r), "invalid_lease");
    assert!(mutate(&mut app, "heartbeat", &a, &lease, json!({}))["ok"] == true);

    // Source must be stated and match the window selection; no API bypass.
    let r = mutate(&mut app, "start_live", &a, &lease, json!({}));
    assert_eq!(refused(&r), "validation");
    let r = mutate(&mut app, "start_live", &a, &lease, json!({"source": "usb"}));
    assert_eq!(refused(&r), "validation");
    assert!(app.source.is_none());
    let s = api(&mut app, "status", &a, None, None, json!({}));
    let s = result(&s);
    assert_eq!(s["ownership"]["owned"], true);
    assert_eq!(s["ownership"]["client_id"], a);
    assert!(
        s["ownership"].get("lease").is_none(),
        "status never leaks the lease"
    );

    // Idle release returns ownership without claiming physical cleanup.
    let r = mutate(&mut app, "release", &a, &lease, json!({}));
    assert_eq!(result(&r)["released"], true);
    assert_eq!(result(&r)["cleanup"], "not_required");
    assert!(!app.manual_locked());
    let r = mutate(&mut app, "heartbeat", &a, &lease, json!({}));
    assert_eq!(refused(&r), "revoked");
    finish(app, ctx, guardian);
}

#[test]
fn usb_start_uses_existing_selection_and_simulation_window_interlock() {
    let (mut app, ctx, guardian) = app(true);
    app.picker_simulation = false;
    let a = fresh();
    let lease = acquire(&mut app, &a);
    // No selected camera: refused with the window's own gate.
    let r = mutate(&mut app, "start_live", &a, &lease, json!({"source": "usb"}));
    assert_eq!(refused(&r), "busy");
    assert!(
        r["error"]
            .as_str()
            .unwrap()
            .contains("Select a supported camera")
    );
    assert!(app.source.is_none());
    finish(app, ctx, guardian);
}

#[test]
fn start_capture_full_stop_and_release_follow_gui_paths_and_evidence() {
    let (mut app, ctx, guardian) = app(true);
    let root = tempfile::tempdir().unwrap();
    let a = fresh();
    let lease = acquire(&mut app, &a);
    let live = start_live(&mut app, &ctx, &a, &lease);
    assert_eq!(live["status"], "completed");
    assert_eq!(live["cleanup"], "not_required");
    assert_eq!(live["native"]["source"], "software");
    until(&mut app, &ctx, "displayed Live", |a| {
        a.product.applied.is_some() && a.capture_ready()
    });
    // Competing start while running uses the window gate.
    let r = mutate(
        &mut app,
        "start_live",
        &a,
        &lease,
        json!({"source": "simulation"}),
    );
    assert_eq!(refused(&r), "busy");

    // Output path validation happens before any save or acquisition change.
    for (path, code) in [
        (json!("relative.png"), "validation"),
        (json!(root.path().join("x.bmp")), "validation"),
        (json!(root.path().join("missing/x.png")), "validation"),
    ] {
        let r = mutate(&mut app, "capture", &a, &lease, json!({"path": path}));
        assert_eq!(refused(&r), code, "{path}");
    }
    let r = mutate(
        &mut app,
        "capture",
        &a,
        &lease,
        json!({"overwrite": true, "path": "/x.png"}),
    );
    assert_eq!(refused(&r), "validation");

    let preferences = app.current_preferences();

    // Current-ROI capture through the GUI save pipeline.
    let path = root.path().join("roi.png");
    let rid = fresh();
    let body = json!({"path": path});
    let first = api(
        &mut app,
        "capture",
        &a,
        Some(&lease),
        Some(&rid),
        body.clone(),
    );
    let job = result(&first)["job_id"].as_str().unwrap().to_owned();
    assert!(app.busy, "the window's save pipeline owns the export");
    // A duplicate request ID returns the same decision: no second export.
    let again = api(&mut app, "capture", &a, Some(&lease), Some(&rid), body);
    assert_eq!(result(&again)["job_id"], job.as_str());
    let other = json!({"path": root.path().join("other.png")});
    let conflict = api(&mut app, "capture", &a, Some(&lease), Some(&rid), other);
    assert_eq!(refused(&conflict), "request_id_conflict");
    let done = terminal(&mut app, &ctx, &job);
    assert_eq!(done["status"], "completed", "{done}");
    assert_eq!(done["native"]["saved"], true);
    assert!(path.is_file());
    assert!(!root.path().join("other.png").exists());
    let outputs = done["native"]["outputs"].as_array().unwrap();
    assert!(outputs.contains(&json!(path.display().to_string())));
    assert!(outputs.len() >= 7, "{outputs:?}");
    // Never overwrite: an existing primary refuses before the pipeline.
    let r = mutate(&mut app, "capture", &a, &lease, json!({"path": path}));
    assert_eq!(refused(&r), "storage");

    assert_eq!(app.current_preferences(), preferences);

    // Full-sensor capture: same transaction, save and Live restore as the button.
    let original = app.product.applied.unwrap();
    let full = root.path().join("full.tiff");
    let r = mutate(&mut app, "capture_full", &a, &lease, json!({"path": full}));
    let job = result(&r)["job_id"].as_str().unwrap().to_owned();
    assert!(app.product.busy());
    // Competing captures are refused while the transaction runs.
    let r = mutate(
        &mut app,
        "capture",
        &a,
        &lease,
        json!({"path": root.path().join("c.png")}),
    );
    assert_eq!(refused(&r), "busy");
    let done = terminal(&mut app, &ctx, &job);
    assert_eq!(done["status"], "completed", "{done}");
    assert_eq!(done["native"]["saved"], true);
    assert_eq!(done["native"]["live_restored"], true);
    assert!(full.is_file());
    assert_eq!(app.product.applied.unwrap().settings, original.settings);
    assert_eq!(app.current_preferences(), preferences);
    let held = app.product.held.as_ref().unwrap();
    assert_eq!(
        held.metadata
            .acquisition
            .as_ref()
            .unwrap()
            .settings()
            .unwrap()
            .mode,
        Mode::Full
    );

    // Stop uses the window's Stop request; terminal only once the Worker is idle.
    let r = mutate(&mut app, "stop", &a, &lease, json!({"job_id": job}));
    assert_eq!(refused(&r), "validation");
    let r = mutate(&mut app, "stop", &a, &lease, json!({}));
    let job = result(&r)["job_id"].as_str().unwrap().to_owned();
    assert_eq!(result(&r)["terminal"], false);
    let done = terminal(&mut app, &ctx, &job);
    assert_eq!(done["status"], "stopped");
    assert_eq!(done["cleanup"], "not_required");
    assert!(!app.session.active);
    let s = api(&mut app, "status", &a, None, None, json!({}));
    assert_eq!(result(&s)["busy"], false);

    // Release with acquisition running returns a stop job for the SDK to follow.
    let live = start_live(&mut app, &ctx, &a, &lease);
    assert_eq!(live["status"], "completed");
    let r = mutate(&mut app, "release", &a, &lease, json!({}));
    let r = result(&r).clone();
    assert_eq!(r["released"], true);
    assert_eq!(r["cleanup"], "pending");
    assert_eq!(r["job"]["kind"], "release");
    assert!(
        !app.manual_locked(),
        "ownership returns to the window at once"
    );
    let contender = fresh();
    let response = mutate(&mut app, "acquire", &contender, "", json!({}));
    assert_eq!(refused(&response), "busy");
    let done = terminal(&mut app, &ctx, r["job_id"].as_str().unwrap());
    assert_eq!(done["status"], "stopped");
    assert_eq!(done["cleanup"], "not_required");
    assert!(app.source.is_none() && !app.session.active);
    let r = api(
        &mut app,
        "result",
        &a,
        None,
        None,
        json!({"job_id": "missing"}),
    );
    assert_eq!(refused(&r), "unknown_job");
    finish(app, ctx, guardian);
}

#[test]
fn heartbeat_expiry_stops_and_reclaims_control() {
    let (mut app, ctx, guardian) = app(true);
    let a = fresh();
    let lease = acquire(&mut app, &a);
    let live = start_live(&mut app, &ctx, &a, &lease);
    assert_eq!(live["status"], "completed");
    // Shorten the timeout; the next heartbeat applies it.
    app.external.timeout = Duration::from_millis(400);
    assert_eq!(
        mutate(&mut app, "heartbeat", &a, &lease, json!({}))["ok"],
        true
    );
    // No heartbeat: the GUI revokes the lease and requests its own Stop.
    until(&mut app, &ctx, "expiry", |a| !a.external.leased());
    assert!(app.source.is_none());
    until(&mut app, &ctx, "stopped", |a| {
        !a.session.active && a.external.running().is_empty()
    });
    let s = api(&mut app, "status", &a, None, None, json!({}));
    let s = result(&s);
    assert_eq!(s["ownership"]["owned"], false);
    assert_eq!(s["ownership"]["last_revocation"], "expired");
    assert_eq!(s["busy"], false);
    let r = mutate(&mut app, "heartbeat", &a, &lease, json!({}));
    assert_eq!(refused(&r), "revoked");
    assert!(r["error"].as_str().unwrap().contains("expired"));
    finish(app, ctx, guardian);
}

#[test]
fn take_control_keeps_acquisition_and_window_stop_stops_it() {
    let (mut app, ctx, guardian) = app(true);
    let a = fresh();
    let lease = acquire(&mut app, &a);
    start_live(&mut app, &ctx, &a, &lease);
    // The window's Take control action.
    app.external.revoke(Revocation::TakenByGui);
    assert!(app.source.is_some() && !app.manual_locked());
    let r = mutate(&mut app, "stop", &a, &lease, json!({}));
    assert_eq!(refused(&r), "revoked");
    assert!(
        app.source.is_some(),
        "a stale lease cannot stop the window's session"
    );
    // A new lease, then the window's External control Stop.
    let lease = acquire(&mut app, &a);
    app.external_window_stop();
    assert!(app.source.is_none() && !app.manual_locked());
    let r = mutate(&mut app, "capture", &a, &lease, json!({"path": "/x.png"}));
    assert_eq!(refused(&r), "revoked");
    until(&mut app, &ctx, "stopped", |a| !a.session.active);
    finish(app, ctx, guardian);
}

#[test]
fn manual_stop_stays_available_and_resolves_external_jobs() {
    let (mut app, ctx, guardian) = app(true);
    let a = fresh();
    let lease = acquire(&mut app, &a);
    let r = mutate(
        &mut app,
        "start_live",
        &a,
        &lease,
        json!({"source": "simulation"}),
    );
    let job = result(&r)["job_id"].as_str().unwrap().to_owned();
    // The window's Stop / Disconnect action while leased.
    app.request(None);
    let done = terminal(&mut app, &ctx, &job);
    assert_eq!(done["status"], "stopped", "{done}");
    assert!(app.manual_locked(), "manual Stop does not take the lease");
    finish(app, ctx, guardian);
}

#[test]
fn unknown_cleanup_blocks_external_operations_until_restart() {
    let (mut app, ctx, guardian) = app(false);
    let a = fresh();
    let lease = acquire(&mut app, &a);
    start_live(&mut app, &ctx, &a, &lease);
    let r = mutate(&mut app, "release", &a, &lease, json!({}));
    let job = result(&r)["job_id"].as_str().unwrap().to_owned();
    let done = terminal(&mut app, &ctx, &job);
    assert_eq!(done["status"], "error");
    assert_eq!(done["cleanup"], "unknown");
    let r = api(&mut app, "acquire", &a, None, Some(&fresh()), json!({}));
    assert_eq!(refused(&r), "cleanup_unconfirmed");
    let s = api(&mut app, "status", &a, None, None, json!({}));
    assert_eq!(result(&s)["phase"], "error");
    assert_eq!(result(&s)["cleanup"], "unknown");
    finish(app, ctx, guardian);
}

#[test]
fn expired_transport_request_is_not_executed() {
    use std::io::{BufRead, BufReader, Write};
    let (mut app, ctx, guardian) = app(true);
    let dir = tempfile::tempdir().unwrap();
    let server = camera_lab::control_api::Server::start(Some(dir.path().join("control"))).unwrap();
    let descriptor: Value =
        serde_json::from_slice(&std::fs::read(&server.descriptor).unwrap()).unwrap();
    app.control_api = Some(server);
    let port = descriptor["port"].as_u64().unwrap() as u16;
    let token = descriptor["token"].clone();
    let send = move |body: Value| {
        std::thread::spawn(move || {
            let mut peer =
                std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            writeln!(peer, "{body}").unwrap();
            let mut line = String::new();
            BufReader::new(peer).read_line(&mut line).unwrap();
            serde_json::from_str::<Value>(&line).unwrap()
        })
    };
    let a = fresh();
    let acquired = send(
        json!({"control_protocol":1,"method":"control.acquire","token":token,"client_id":a,"request_id":fresh()}),
    );
    while !acquired.is_finished() {
        draw(&mut app, &ctx);
        std::thread::sleep(Duration::from_millis(5));
    }
    let acquired = acquired.join().unwrap();
    assert_eq!(acquired["ok"], true, "{acquired}");
    let lease = acquired["result"]["lease"].clone();
    // The GUI does not run while the transport deadline passes.
    let late = send(
        json!({"control_protocol":1,"method":"control.start_live","token":token,
        "client_id":a,"lease":lease,"request_id":fresh(),"source":"simulation"}),
    )
    .join()
    .unwrap();
    assert_eq!(late["error"]["code"], "timeout");
    for _ in 0..5 {
        draw(&mut app, &ctx);
    }
    assert!(
        app.source.is_none(),
        "an expired request never starts acquisition"
    );
    assert!(app.external.running().is_empty());
    finish(app, ctx, guardian);
}
