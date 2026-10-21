//! One coherent exercise of the production App actions, Worker and transaction controller.
use super::*;
use camera_lab::{
    session::{Event, Factory, Transport},
    startup::Intent,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

struct FakeFactory(Arc<AtomicU64>);
struct FakeOwner {
    settings: Settings,
    revision: u64,
    sequence: u64,
    stopped: bool,
    intent: Intent,
}
impl Factory for FakeFactory {
    fn open(&mut self, r: &Request) -> Result<Box<dyn Transport>, String> {
        self.open_generation(r, 1)
    }
    fn open_generation(&mut self, r: &Request, id: u64) -> Result<Box<dyn Transport>, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeOwner {
            settings: r.settings,
            revision: id,
            sequence: 0,
            stopped: false,
            intent: r.startup_intent,
        }))
    }
}
impl Transport for FakeOwner {
    fn stop(&mut self) -> Result<(), String> {
        self.stopped = true;
        Ok(())
    }
    fn poll(&mut self) -> Result<Event, String> {
        if self.stopped {
            return Ok(Event::Done { clean: true });
        }
        if self.settings.mode == Mode::Full && self.settings.gain_eighths == 31 {
            return Err("deterministic Full helper fault".into());
        }
        self.sequence += 1;
        let (w, h) = self.settings.dimensions();
        let mut a = camera_lab::sensor_contract::Acquisition::new(
            self.settings,
            [self.revision as u8; 32],
            self.revision,
            self.sequence,
        )?
        .with_startup(self.intent);
        a.request_revision = Some(self.revision);
        Ok(Event::Frame(Frame {
            raw: Arc::new(
                (0..usize::from(w) * usize::from(h))
                    .map(|i| (i as u64 + self.sequence) as u8)
                    .collect(),
            ),
            metadata: camera_lab::Metadata {
                acquisition: Some(a),
                source: "typed deterministic product fake".into(),
                simulated: true,
                sequence: self.sequence,
                measurement: None, unix_millis: 0,
                width: w.into(),
                height: h.into(),
                format: "BayerGRBG8".into(),
            },
        }))
    }
}
fn draw(app: &mut App, ctx: &egui::Context) {
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
}
fn until(app: &mut App, ctx: &egui::Context, p: impl Fn(&App) -> bool) {
    let end = Instant::now() + Duration::from_secs(15);
    while !p(app) {
        assert!(
            Instant::now() < end,
            "{} / {} / {}",
            app.session.message,
            app.product.capture_result,
            app.product.restore_result
        );
        draw(app, ctx);
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn fresh(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("integrated-v022-{name}-{}", std::process::id()));
    std::fs::create_dir(&p).unwrap();
    p
}
fn frozen_equal(path: &std::path::Path, f: &Frame) {
    frozen_recipe_equal(path,f,Recipe::default());
}
fn frozen_recipe_equal(path: &std::path::Path, f: &Frame,recipe:Recipe) {
    assert!(path.is_file());
    assert_eq!(
        image::open(path).unwrap().to_rgb8().as_raw(),
        &recipe.render(f,None).unwrap().2
    );
    assert_eq!(
        std::fs::read(format!("{}.raw", path.display())).unwrap(),
        *f.raw
    );
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(format!("{}.json", path.display())).unwrap())
            .unwrap();
    assert_eq!(meta["frame"], serde_json::to_value(&f.metadata).unwrap());
    assert_eq!(meta["display_recipe"],serde_json::to_value(recipe).unwrap());
    assert!(meta["exposure_time_seconds"].is_null());
}
#[test]
fn coherent_flow_and_stop_precedence() {
    let root = fresh("flow");
    let ctx = egui::Context::default();
    let opens = Arc::new(AtomicU64::new(0));
    let worker = Worker::with_factory(FakeFactory(opens.clone()));
    let mut guardian = worker.guardian();
    let mut app = App::with_exporter_worker(&ctx, camera_lab::image_export::save, worker);
    app.fake_only = true;
    app.picker = false;
    app.request(Some(Source::Simulation));
    until(&mut app, &ctx, |a| {
        a.product.applied.is_some() && a.frame.is_some()
    });
    let first = app.revision;
    let count = opens.load(Ordering::SeqCst);
    app.settings.exposure_rows = 101;
    app.settings.gain_eighths = 9;
    for _ in 0..3 {
        draw(&mut app, &ctx);
    }
    assert_eq!(opens.load(Ordering::SeqCst), count);
    assert_eq!(app.product.applied.unwrap().settings.exposure_rows, Settings::default().exposure_rows);
    // The production action submits draft once; edits alone submit nothing.
    app.request(Some(Source::Simulation));
    let applied_revision = app.revision;
    assert!(applied_revision > first);
    until(&mut app, &ctx, |a| {
        a.product
            .applied
            .is_some_and(|p| p.revision == applied_revision)
    });
    let original = app.product.applied.unwrap();
    app.full_selection(None);
    assert_eq!(app.revision, applied_revision);
    assert!(!app.product.busy());
    assert!(app.save_result.contains("cancelled"));
    app.paused = true;
    let frozen = app.frame.clone().unwrap();
    let submits = app.gui_submits;
    until(&mut app, &ctx, |a| {
        a.product.applied.unwrap().sequence > frozen.metadata.sequence + 3
    });
    assert_eq!(app.gui_submits, submits);
    assert_eq!(app.frame.as_ref().unwrap().metadata, frozen.metadata);
    let path = root.join("chosen.png");
    app.save_frozen(frozen.clone(), path.clone());
    until(&mut app, &ctx, |a| !a.busy);
    frozen_equal(&path, &frozen);
    app.paused = false;
    until(&mut app, &ctx, |a| {
        a.frame.as_ref().unwrap().metadata.sequence > frozen.metadata.sequence
    });
    // Preserve an unapplied draft throughout Full and restoration.
    app.settings.gain_eighths = 12;
    let fullpath = root.join("full.tiff");
    app.begin_full(fullpath.clone());
    let full_revision = app.revision;
    app.begin_full(root.join("duplicate.png"));
    assert_eq!(app.revision, full_revision);
    until(&mut app, &ctx, |a| !a.product.busy() && !a.busy);
    let held = app.product.held.clone().unwrap();
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
    frozen_equal(&fullpath, &held);
    assert_eq!(app.product.applied.unwrap().settings, original.settings);
    assert_eq!(app.settings.gain_eighths, 12);
    assert!(app.product.restore_result.contains("verified"));
    assert!(!root.join("duplicate.png").exists());
    // Already Full waits two new sequences without any restart.
    app.settings.mode = Mode::Full;
    app.request(Some(Source::Simulation));
    let rev = app.revision;
    until(&mut app, &ctx, |a| {
        a.product.applied.is_some_and(|p| p.revision == rev)
    });
    let n = opens.load(Ordering::SeqCst);
    let seq = app.product.applied.unwrap().sequence;
    app.begin_full(root.join("already-full.png"));
    until(&mut app, &ctx, |a| !a.product.busy() && !a.busy);
    assert_eq!(opens.load(Ordering::SeqCst), n);
    assert!(app.product.held.as_ref().unwrap().metadata.sequence > seq);
    // Stop cancels every auto-return even when Full was just submitted.
    app.settings.mode = Mode::FullFieldSkip;
    app.request(Some(Source::Simulation));
    let rev = app.revision;
    until(&mut app, &ctx, |a| {
        a.product.applied.is_some_and(|p| p.revision == rev)
    });
    app.begin_full(root.join("interrupted.png"));
    app.request(None);
    let rev = app.revision;
    until(&mut app, &ctx, |a| !a.session.active);
    for _ in 0..5 {
        draw(&mut app, &ctx);
    }
    assert_eq!(app.revision, rev);
    assert!(app.product.applied.is_none());
    app.worker.close();
    guardian.wait();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_failures_and_close_during_save() {
    let root = fresh("failures");
    let ctx = egui::Context::default();
    let opens = Arc::new(AtomicU64::new(0));
    let (release, gate) = mpsc::sync_channel(1);
    let (started, start) = mpsc::sync_channel(1);
    let worker = Worker::with_factory(FakeFactory(opens.clone()));
    let mut guardian = worker.guardian();
    let mut app = App::with_exporter_worker(
        &ctx,
        move |f, p| {
            started.send(()).unwrap();
            gate.recv().unwrap();
            camera_lab::image_export::save(f, p)
        },
        worker,
    );
    app.fake_only = true;
    app.picker = false;
    app.request(Some(Source::Simulation));
    until(&mut app, &ctx, |a| {
        a.product.applied.is_some() && a.frame.is_some()
    });
    let retained = app.frame.clone().unwrap();
    let applied = app.product.applied.unwrap();
    // Wrong generation/session/duplicate never confirms a product action.
    let mut c = camera_lab::product::Controller::default();
    c.start();
    assert!(
        c.observe(
            &retained,
            applied.revision + 1,
            applied.settings,
            applied.intent,
            Instant::now()
        )
        .is_none()
    );
    assert!(c.applied.is_none());
    c.observe(
        &retained,
        applied.revision,
        applied.settings,
        applied.intent,
        Instant::now(),
    );
    c.begin(Instant::now()).unwrap();
    assert!(
        c.observe(
            &retained,
            applied.revision,
            applied.settings,
            applied.intent,
            Instant::now()
        )
        .is_none()
    );
    assert!(c.held.is_none());
    let mut wrong = retained.clone();
    wrong.metadata.acquisition.as_mut().unwrap().session = [99; 32];
    assert!(
        c.observe(
            &wrong,
            applied.revision,
            applied.settings,
            applied.intent,
            Instant::now()
        )
        .is_none()
    );
    assert!(c.held.is_none());
    let mut wrong = retained.clone();
    wrong.metadata.acquisition.as_mut().unwrap().startup_intent = Some(Intent::Diagnostics);
    assert!(
        c.observe(
            &wrong,
            applied.revision,
            applied.settings,
            applied.intent,
            Instant::now()
        )
        .is_none()
    );
    assert!(c.held.is_none());
    c.fail("helper fault");
    assert!(c.restore_result.contains("unverified"));
    assert!(!c.busy());
    c.observe(
        &retained,
        applied.revision,
        applied.settings,
        applied.intent,
        Instant::now(),
    );
    assert!(c.applied.is_none());
    c.start();
    c.observe(
        &retained,
        applied.revision,
        applied.settings,
        applied.intent,
        Instant::now(),
    );
    let at = Instant::now();
    c.begin(at).unwrap();
    assert!(c.tick(at + Duration::from_secs(66)));
    assert!(c.applied.is_none());
    assert!(c.restore_result.contains("unverified"));
    // Real disk error; primary absent, without preventing a verified restoration.
    assert!(camera_lab::image_export::save(&retained, &root.join("absent/image.png")).is_err());
    app.begin_full(root.join("absent/full.png"));
    until(&mut app, &ctx, |a| a.busy);
    start.recv_timeout(Duration::from_secs(3)).unwrap();
    release.send(()).unwrap();
    until(&mut app, &ctx, |a| !a.product.busy() && !a.busy);
    assert!(app.save_result.contains("failed"));
    assert!(app.product.restore_result.contains("verified"));
    // Stop during restore, while a save is blocked, cannot be undone by save completion.
    app.begin_full(root.join("stop-save.png"));
    until(&mut app, &ctx, |a| a.busy);
    start.recv_timeout(Duration::from_secs(3)).unwrap();
    app.request(None);
    let rev = app.revision;
    release.send(()).unwrap();
    until(&mut app, &ctx, |a| !a.busy && !a.session.active);
    assert_eq!(app.revision, rev);
    assert!(app.product.applied.is_none());
    // Close while restoration and a blocked save are pending also cancels return.
    app.request(Some(Source::Simulation));
    until(&mut app, &ctx, |a| a.product.applied.is_some());
    app.begin_full(root.join("close-save.png"));
    until(&mut app, &ctx, |a| a.busy);
    start.recv_timeout(Duration::from_secs(3)).unwrap();
    app.fake_until = Some(Instant::now());
    draw(&mut app, &ctx);
    release.send(()).unwrap();
    until(&mut app, &ctx, |a| a.worker.closed() && !a.busy);
    assert!(app.product.applied.is_none());
    guardian.wait();
    // Use a fresh controller/worker for a separate Close during Full startup.
    let worker = Worker::with_factory(FakeFactory(opens.clone()));
    let mut guardian = worker.guardian();
    let mut app = App::with_exporter_worker(&ctx, camera_lab::image_export::save, worker);
    app.fake_only = true;
    app.picker = false;
    // Close has precedence before processing a newly arrived Full frame.
    app.request(Some(Source::Simulation));
    until(&mut app, &ctx, |a| a.product.applied.is_some());
    app.begin_full(root.join("close.png"));
    app.fake_until = Some(Instant::now());
    draw(&mut app, &ctx);
    until(&mut app, &ctx, |a| a.worker.closed());
    assert!(!app.product.busy());
    assert!(app.product.applied.is_none());
    guardian.wait();
    let worker = Worker::with_factory(FakeFactory(opens.clone()));
    let mut guardian = worker.guardian();
    let mut app = App::with_exporter_worker(&ctx, camera_lab::image_export::save, worker);
    app.fake_only = true;
    app.picker = false;
    app.settings.gain_eighths = 31;
    app.request(Some(Source::Simulation));
    until(&mut app, &ctx, |a| a.product.applied.is_some());
    app.begin_full(root.join("helper-fault.png"));
    until(&mut app, &ctx, |a| {
        a.session.state == camera_lab::session::State::Error && !a.session.active
    });
    assert!(app.product.applied.is_none());
    assert!(app.product.capture_result.contains("failed"));
    assert!(app.product.restore_result.contains("unverified"));
    assert!(!root.join("helper-fault.png").exists());
    app.worker.close();
    guardian.wait();
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn native_paired_completion() {
    let root = PathBuf::from(
        std::env::var_os("INTEGRATED_UX_FAKE_ROOT").expect("new dedicated fake root required"),
    );
    std::fs::create_dir(&root).unwrap();
    let ctx = egui::Context::default();
    let worker = Worker::new();
    let mut guardian = worker.guardian();
    let mut app = App::with_recipe_exporter_worker(&ctx, camera_lab::image_export::save_recipe, worker);
    app.fake_only = true;
    app.picker = false;
    app.request(Some(Source::Fixture {
        root: root.clone(),
        fault: camera_lab::continuous_session::Fixture::TypedFrames,
    }));
    until(&mut app, &ctx, |a| {
        a.product.applied.is_some() && a.frame.is_some()
    });
    app.settings.exposure_rows = 101;
    app.request(app.source.clone());
    let rev = app.revision;
    until(&mut app, &ctx, |a| {
        a.product.applied.is_some_and(|p| p.revision == rev)
    });
    let recipe=Recipe{ev:0.5,gamma:1.25,wb:[1.1,1.,0.9],quarter_turns:1,mirror:true,..Default::default()};
    app.recipe=recipe;app.recipe_changed();
    until(&mut app,&ctx,|a|a.displayed_recipe==recipe);
    app.paused = true;
    let held = app.frame.clone().unwrap();
    until(&mut app, &ctx, |a| {
        a.product.applied.unwrap().sequence > held.metadata.sequence + 3
    });
    let path = root.join("visible.png");
    app.save_frozen(held.clone(), path.clone());
    app.recipe=Recipe::default();app.recipe_changed();
    until(&mut app, &ctx, |a| !a.busy);
    frozen_recipe_equal(&path, &held,recipe);
    app.paused = false;
    until(&mut app, &ctx, |a| {
        a.frame.as_ref().unwrap().metadata.sequence > held.metadata.sequence
    });
    let original = app.product.applied.unwrap().settings;
    let path = root.join("full.tiff");
    app.recipe=recipe;app.recipe_changed();app.begin_full(path.clone());
    app.recipe=Recipe::default();app.recipe_changed();
    until(&mut app, &ctx, |a| !a.product.busy() && !a.busy);
    assert_eq!(app.product.applied.unwrap().settings, original);
    frozen_recipe_equal(&path, app.product.held.as_ref().unwrap(),recipe);
    // Hold the final session briefly so external native handles can be retained.
    let end = Instant::now() + Duration::from_millis(500);
    while Instant::now() < end {
        draw(&mut app, &ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    app.request(None);
    until(&mut app, &ctx, |a| !a.session.active);
    app.worker.close();
    let s = guardian.wait();
    assert!(!s.restart_disabled);
    assert!(!s.active);
    assert!(
        !std::fs::read_dir(&root).unwrap().any(|e| e
            .unwrap()
            .path()
            .extension()
            .is_some_and(|x| x == "lock"))
    );
    println!(
        "integrated native fake: exact PNG/TIFF + RAW sidecars; new Full; original Live verified; guardian collected"
    );
}
