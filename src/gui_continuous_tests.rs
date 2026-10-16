//! Actual App -> Worker -> production continuous adapter -> fixture helper.
//! Headless egui, never a native click/desktop or physical acceptance test.
use super::*;
use camera_lab::continuous_session::Fixture;
use std::time::Instant;

fn draw(app: &mut App, ctx: &egui::Context) {
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
}
fn until(app: &mut App, ctx: &egui::Context, seconds: u64, predicate: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while !predicate(app) && Instant::now() < deadline {
        draw(app, ctx);
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        predicate(app),
        "{} / {} / {:?}",
        app.status,
        app.session.message,
        app.session.lifecycle
    );
}
fn root(name: &str) -> PathBuf {
    let root = PathBuf::from(
        std::env::var_os("GUI_CONTINUOUS_FAKE_ROOT").expect("explicit fake root required"),
    )
    .join(name);
    std::fs::create_dir(&root).unwrap();
    root
}
fn source(root: &std::path::Path, fault: Fixture) -> Source {
    Source::Fixture {
        root: root.into(),
        fault,
    }
}
fn exact(frame: &Frame) {
    assert!(frame.metadata.simulated);
    assert!(
        frame
            .raw
            .iter()
            .enumerate()
            .all(|(i, b)| *b == (i as u64 + frame.metadata.sequence) as u8)
    );
}

#[test]
fn two_mode_actual_gui_full_skip_full_pause_capture_stop() {
    let root=root("two-mode");let ctx=egui::Context::default();
    let mut app=App::with_exporter(&ctx, camera_lab::export);app.fake_only=true;app.picker=false;
    let mut previous_session=None;let mut held:Option<Frame>=None;
    for mode in [Mode::Full,Mode::FullFieldSkip,Mode::Full] {
        app.settings.mode=mode;app.request(Some(source(&root,Fixture::TypedFrames)));
        let revision=app.revision;
        until(&mut app,&ctx,28,|a| a.frame.as_ref().and_then(|f|f.metadata.acquisition.as_ref()).is_some_and(|c|c.request_revision==Some(revision)));
        let frame=app.frame.clone().unwrap();frame.validate().unwrap();exact(&frame);
        let c=frame.metadata.acquisition.as_ref().unwrap();
        assert_eq!(c.settings_generation,revision);
        assert_eq!(c.settings().unwrap(),app.settings);
        assert_eq!(c.geometry.source_window,[32,20,2048,1536]);
        assert_eq!(c.geometry.address_modes,if mode==Mode::FullFieldSkip {[1,1]} else {[0,0]});
        if let Some(old)=previous_session {assert_ne!(c.session,old);}
        previous_session=Some(c.session);
        if let Some(old)=&held {assert!(old.metadata.acquisition.as_ref().unwrap().request_revision.unwrap()<revision);}
        app.paused=true;let submits=app.gui_submits;let texture=app.texture.as_ref().unwrap().id();
        let until_at=Instant::now()+Duration::from_millis(450);
        while Instant::now()<until_at {draw(&mut app,&ctx);std::thread::sleep(Duration::from_millis(20));}
        assert_eq!(app.gui_submits,submits);assert_eq!(app.frame.as_ref().unwrap().metadata,frame.metadata);
        assert_eq!(app.texture.as_ref().unwrap().id(),texture);assert!(app.session.active);
        let path=root.join(format!("held-{revision}.png"));app.tx.try_send((frame.clone(),path.clone(),Recipe::default())).unwrap();app.busy=true;
        until(&mut app,&ctx,8,|a|!a.busy);assert!(app.save_result.starts_with("Saved image"));
        let bundle=path.with_file_name(format!("held-{revision}.png.capture"));
        assert_eq!(std::fs::read(bundle.join("raw.bin")).unwrap(),*frame.raw);
        let meta:camera_lab::Metadata=serde_json::from_slice(&std::fs::read(bundle.join("metadata.json")).unwrap()).unwrap();assert_eq!(meta,frame.metadata);
        held=Some(frame);app.paused=false;
    }
    let retained=app.frame.clone().unwrap();let submits=app.gui_submits;
    app.request(None);until(&mut app,&ctx,12,|a|!a.session.active);
    assert_eq!(app.gui_submits,submits);assert_eq!(app.frame.as_ref().unwrap().metadata,retained.metadata);
    assert_eq!(app.session.state,camera_lab::session::State::Disconnected);
    assert!(!std::fs::read_dir(&root).unwrap().any(|e| e.unwrap().path().extension().is_some_and(|e|e=="lock")));
    app.worker.close();
}

#[test]
fn repair_actual_app_drop_gui_return_unwind_and_active_poll_panic() {
    use camera_lab::continuous_session::ContinuousSession;
    use camera_lab::session::{Event, Factory, Lifecycle, Transport};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Make {
        panic: bool,
        hit: Arc<AtomicBool>,
    }
    struct Owner {
        session: ContinuousSession,
        panic: bool,
        hit: Arc<AtomicBool>,
        lifecycle: Lifecycle,
    }
    impl Factory for Make {
        fn open(&mut self, r: &Request) -> Result<Box<dyn Transport>, String> {
            let Source::Fixture { root, fault } = &r.source else {
                return Err("fake only".into());
            };
            Ok(Box::new(Owner {
                session: ContinuousSession::start_fixture_settings(root, *fault, r.settings)
                    .map_err(|e| e.to_string())?,
                panic: self.panic,
                hit: self.hit.clone(),
                lifecycle: Lifecycle::default(),
            }))
        }
    }
    impl Transport for Owner {
        fn stop(&mut self) -> Result<(), String> {
            self.session.stop();
            Ok(())
        }
        fn lifecycle(&self) -> Option<Lifecycle> {
            Some(self.lifecycle.clone())
        }
        fn poll(&mut self) -> Result<Event, String> {
            let s = self.session.poll();
            self.lifecycle = Lifecycle {
                collected: s.collected,
                exit: s.exit,
                unproved: s.terminal == Some(camera_lab::continuous_session::Terminal::Unproved),
                marker_deleted: s.marker_deleted,
                release_committed: s.release_committed,
                phase: s.helper_phase,
                ..Default::default()
            };
            if self.panic && s.frame.is_some() {
                self.panic = false;
                self.hit.store(true, Ordering::Release);
                panic!("finite one active poll panic");
            }
            if s.collected {
                return Ok(Event::Done {
                    clean: s.exit == Some(0) && s.marker_deleted,
                });
            }
            Ok(s.frame.map(Event::Frame).unwrap_or(Event::Waiting))
        }
    }
    for (name, panic_poll, unwind, fault) in [
        ("drop", false, false, Fixture::FullFrames),
        ("gui-unwind", false, true, Fixture::FullFrames),
        ("poll-panic", true, false, Fixture::FullFrames),
        ("unproved-return", false, false, Fixture::ReleaseStall),
    ] {
        let root = root(name);
        let ctx = egui::Context::default();
        let hit = Arc::new(AtomicBool::new(false));
        let worker = if panic_poll {
            Worker::with_factory(Make {
                panic: true,
                hit: hit.clone(),
            })
        } else {
            Worker::new()
        };
        let mut guardian = worker.guardian(); // before App construction
        let mut app = App::with_exporter_worker(&ctx, camera_lab::export, worker);
        app.fake_only = true;
        app.picker = false;
        app.request(Some(source(&root, fault)));
        until(&mut app, &ctx, 28, |a| {
            if panic_poll {
                hit.load(Ordering::Acquire)
            } else {
                a.frame.is_some()
            }
        });
        if !panic_poll {
            assert_eq!(app.frame.as_ref().unwrap().metadata.unix_millis, 0);
            fn unavailable(s: &egui::epaint::Shape) -> bool {
                match s {
                    egui::epaint::Shape::Text(t) => {
                        t.galley.job.text.contains("Capture time unavailable")
                    }
                    egui::epaint::Shape::Vec(v) => v.iter().any(unavailable),
                    _ => false,
                }
            }
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100., 750.),
                    )),
                    ..Default::default()
                },
                |ctx| app.render(ctx),
            );
            assert!(output.shapes.iter().any(|s| unavailable(&s.shape)));
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            if unwind {
                panic!("finite GUI unwind with actual App");
            }
            drop(app); // actual event-loop return/App teardown
            Err::<(), _>("finite GUI error return")
        }));
        assert_eq!(result.is_err(), unwind);
        if !matches!(&result, Ok(Ok(()))) {
            guardian.gui_failed();
        }
        let final_state = guardian.wait();
        assert!(!final_state.active);
        let l = final_state.lifecycle.as_ref().unwrap();
        assert!(l.collected);
        assert_eq!(l.exit, Some(0));
        assert!(final_state.restart_disabled);
        assert_eq!(final_state.state, camera_lab::session::State::Error);
        if panic_poll {
            assert!(final_state.restart_disabled);
            assert_eq!(final_state.state, camera_lab::session::State::Error);
        }
        if fault == Fixture::ReleaseStall {
            assert!(l.unproved);
            assert!(!l.marker_deleted);
        }
        // Read through a retained observer, without owning an App or replacing transport.
        // Child native waits and exact PIDs are independently retained by SSH harness.
        println!(
            "SIMULATION repair {name}: guardian completed; collected={} numeric_exit={:?} unproved={} marker_deleted={} poll_panic={}",
            l.collected,
            l.exit,
            l.unproved,
            l.marker_deleted,
            hit.load(Ordering::Acquire)
        );
    }
}

#[test]
fn continuous_actual_gui_worker_finite_long_slow_export_latest_close() {
    let root = root("long-full");
    let ctx = egui::Context::default();
    let exports = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let started = exports.clone();
    let mut app = App::with_exporter(&ctx, move |f, p| {
        started.fetch_add(1, std::sync::atomic::Ordering::Release);
        std::thread::sleep(Duration::from_millis(500));
        camera_lab::export(f, p)
    });
    app.fake_only = true;
    app.picker = false;
    app.settings.mode = Mode::Full;
    app.request(Some(source(&root, Fixture::FullFrames)));
    assert_eq!(
        app.worker.snapshot().state,
        camera_lab::session::State::Connecting
    );
    until(&mut app, &ctx, 8, |a| {
        a.session.lifecycle.as_ref().is_some_and(|l| l.phase >= 1)
    });
    assert!(app.frame.is_none());
    until(&mut app, &ctx, 25, |a| a.frame.is_some());
    let initial_revision = app.revision;
    let start = Instant::now();
    let mut seen = 0;
    let mut deliveries = [0; 2];
    // Both intentionally slow preview consumers use the actual App conversion.
    for (index, delay) in [100, 500].into_iter().enumerate() {
        let phase = Instant::now();
        while phase.elapsed() < Duration::from_secs(16) {
            draw(&mut app, &ctx);
            let f = app.frame.as_ref().unwrap();
            exact(f);
            assert!(f.metadata.sequence >= seen);
            deliveries[index] += u64::from(f.metadata.sequence > seen);
            seen = f.metadata.sequence;
            assert_eq!(app.revision, initial_revision);
            assert!(
                app.session.active,
                "{} {:?}",
                app.session.message, app.session.lifecycle
            );
            assert!(app.session.lifecycle.as_ref().is_some_and(|l| !l.unproved));
            std::thread::sleep(Duration::from_millis(delay));
        }
    }
    assert!(start.elapsed() > Duration::from_secs(30));
    let frozen = app.frame.clone().unwrap();
    exact(&frozen);
    for ext in ["png", "tiff"] {
        let path = root.join(format!("held.{ext}"));
        let next_export = exports.load(std::sync::atomic::Ordering::Acquire) + 1;
        app.tx.try_send((frozen.clone(), path.clone(),Recipe::default())).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while exports.load(std::sync::atomic::Ordering::Acquire) < next_export
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            exports.load(std::sync::atomic::Ordering::Acquire),
            next_export
        );
        app.busy = true;
        assert!(
            app.tx
                .try_send((frozen.clone(), root.join("one-pending.png"),Recipe::default()))
                .is_ok()
        );
        assert!(
            app.tx
                .try_send((frozen.clone(), root.join("overflow.png"),Recipe::default()))
                .is_err()
        );
        // Drain the one explicitly queued immutable payload as well.
        until(&mut app, &ctx, 8, |a| !a.busy);
        let bundle = root.join(format!("held.{ext}.capture"));
        assert_eq!(std::fs::read(bundle.join("raw.bin")).unwrap(), *frozen.raw);
        let meta: camera_lab::Metadata =
            serde_json::from_slice(&std::fs::read(bundle.join("metadata.json")).unwrap()).unwrap();
        assert_eq!(meta, frozen.metadata);
        let decoded = image::open(bundle.join(path.file_name().unwrap()))
            .unwrap()
            .to_rgb8();
        assert_eq!(decoded.as_raw(), frozen.display_rgb().as_ref());
        // Wait for second bounded export result before starting the next export.
        app.busy = true;
        until(&mut app, &ctx, 8, |a| !a.busy);
    }
    let old = app.frame.clone().unwrap();
    let across = root.join("across-change.tiff");
    app.tx.try_send((old.clone(), across.clone(),Recipe::default())).unwrap();
    app.busy = true;
    for (index, exposure) in (101..111).enumerate() {
        app.settings.mode = Mode::ALL[index % Mode::ALL.len()];
        app.settings.gain_eighths = 8 + index as u16;
        app.settings.exposure_rows = exposure;
        app.request(Some(source(&root, Fixture::FullFrames)));
    }
    let wanted = app.revision;
    draw(&mut app, &ctx);
    assert_eq!(app.frame.as_ref().unwrap().metadata, old.metadata);
    assert_eq!(app.session.requested.unwrap().exposure_rows, 110);
    assert_eq!(
        app.session.state,
        camera_lab::session::State::SettingsPending
    );
    until(&mut app, &ctx, 30, |a| {
        a.frame
            .as_ref()
            .is_some_and(|f| f.metadata.source.contains(&format!("revision={wanted};")))
    });
    assert!(
        app.frame
            .as_ref()
            .unwrap()
            .metadata
            .source
            .contains("exposure_rows=110")
    );
    exact(app.frame.as_ref().unwrap());
    until(&mut app, &ctx, 8, |a| !a.busy);
    let bundle = root.join("across-change.tiff.capture");
    assert_eq!(std::fs::read(bundle.join("raw.bin")).unwrap(), *old.raw);
    let meta: camera_lab::Metadata =
        serde_json::from_slice(&std::fs::read(bundle.join("metadata.json")).unwrap()).unwrap();
    assert_eq!(meta, old.metadata);
    let decoded = image::open(bundle.join("across-change.tiff"))
        .unwrap()
        .to_rgb8();
    assert_eq!(decoded.as_raw(), old.display_rgb().as_ref());
    // A real bounded export error must not end acquisition.
    app.tx.try_send((old.clone(), across,Recipe::default())).unwrap();
    app.busy = true;
    until(&mut app, &ctx, 8, |a| !a.busy);
    assert!(app.save_result.contains("Save failed"));
    assert!(app.session.active);
    // No painting or snapshot requests: a native modal dialog may do this.
    std::thread::sleep(Duration::from_secs(3));
    draw(&mut app, &ctx);
    assert!(app.session.active && !app.session.restart_disabled);
    app.closing = true;
    app.worker.close();
    assert!(!app.worker.closed());
    until(&mut app, &ctx, 15, |a| a.worker.closed());
    draw(&mut app, &ctx);
    let l = app.session.lifecycle.as_ref().unwrap();
    assert!(l.collected && l.marker_deleted && l.release_committed);
    assert_eq!(l.exit, Some(0));
    assert!(
        std::fs::read_dir(&root).unwrap().all(|e| e
            .unwrap()
            .path()
            .extension()
            .is_none_or(|e| e != "lock"))
    );
    println!(
        "SIMULATION long live_ms={} consumers_ms=100,500 deliveries={deliveries:?} full_bytes={} revision={} final_numeric_exit={:?} collected={} lock_gone={}",
        start.elapsed().as_millis(),
        frozen.raw.len(),
        wanted,
        l.exit,
        l.collected,
        l.marker_deleted
    );
}

#[test]
fn continuous_actual_gui_worker_unproved_close_retains_owner_and_fake_marker() {
    let root = root("unproved");
    let ctx = egui::Context::default();
    let mut app = App::with_exporter(&ctx, camera_lab::export);
    app.fake_only = true;
    app.picker = false;
    app.request(Some(source(&root, Fixture::ReleaseStall)));
    until(&mut app, &ctx, 28, |a| a.frame.is_some());
    let held = app.frame.clone().unwrap();
    app.closing = true;
    app.worker.close();
    // The GUI's old outer deadline must never authorize process exit.
    app.close_started = Some(Instant::now() - Duration::from_secs(71));
    until(&mut app, &ctx, 12, |a| a.session.restart_disabled);
    assert!(!app.worker.closed());
    assert!(
        app.session.active,
        "{} {:?}",
        app.session.message, app.session.lifecycle
    );
    let lock = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "lock"))
        .unwrap();
    let bytes = std::fs::read(&lock).unwrap();
    assert!(
        app.worker
            .request(Some(Request {
                startup_intent: camera_lab::startup::Intent::Normal,
                source: source(&root, Fixture::Normal),
                settings: Settings::default()
            }))
            .is_err()
    );
    until(&mut app, &ctx, 10, |a| a.worker.closed());
    assert_eq!(std::fs::read(&lock).unwrap(), bytes);
    assert_eq!(app.frame.as_ref().unwrap().metadata, held.metadata);
    draw(&mut app, &ctx);
    let l = app.session.lifecycle.as_ref().unwrap();
    assert!(l.collected && l.unproved && !l.marker_deleted);
    assert_eq!(l.exit, Some(0));
    println!(
        "SIMULATION unproved fake_marker={} retained_bytes={} collected={} numeric_exit={:?}",
        lock.display(),
        bytes.len(),
        l.collected,
        l.exit
    );
}
