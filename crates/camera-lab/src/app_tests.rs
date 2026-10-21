//! Application regression tests, kept separate from production event handling.
#[test]
fn two_mode_chooser_and_terminal_preview_survives_export_same_apply() {
    use super::*;
    use camera_lab::{CameraAdapter,Simulation};
    let ctx=egui::Context::default();
    let mut app=App::with_exporter(&ctx, camera_lab::export);app.picker=false;app.fake_only=true;
    assert_eq!(Mode::ALL,[Mode::FullFieldSkip,Mode::Full]);
    assert_eq!(app.settings.mode,Mode::FullFieldSkip);
    let retained=Simulation {sequence:4}.next_frame().unwrap();
    app.show_frame(&ctx,retained.clone());let texture=app.texture.as_ref().unwrap().id();let count=app.gui_submits;
    let p=camera_lab::preview_pipeline::Pipeline::new(||panic!("injected terminal callback")).unwrap();
    let g=camera_lab::preview_pipeline::Generation {revision:1,settings:app.settings};
    p.set_generation(Some(g));p.submit(camera_lab::preview_pipeline::Key {generation:g,sequence:retained.metadata.sequence},retained.clone());
    let end=std::time::Instant::now()+Duration::from_secs(5);
    while !p.terminal_failure() {assert!(std::time::Instant::now()<end);std::thread::yield_now();}
    app.preview=Some(p);app.preview_generation=Some(g);
    draw(&mut app,&ctx,egui::vec2(1100.,750.));
    let diagnosis=app.terminal_preview_failure.clone().unwrap();assert!(diagnosis.contains("Reopen the application"));
    let root=std::env::temp_dir().join(format!("two-mode-terminal-{}",std::process::id()));std::fs::create_dir(&root).unwrap();
    app.tx.try_send((retained.clone(),root.join("retained.png"),Recipe::default())).unwrap();app.busy=true;
    while app.busy {assert!(std::time::Instant::now()<end);draw(&mut app,&ctx,egui::vec2(1100.,750.));}
    assert!(app.save_result.starts_with("Saved image"));
    app.request(Some(Source::Simulation));let revision=app.revision;
    app.request(Some(Source::Simulation));assert!(app.revision>revision);
    draw(&mut app,&ctx,egui::vec2(1100.,750.));
    assert_eq!(app.terminal_preview_failure.as_deref(),Some(diagnosis.as_str()));
    assert_eq!(app.gui_submits,count);assert_eq!(app.texture.as_ref().unwrap().id(),texture);
    assert_eq!(app.frame.as_ref().unwrap().metadata,retained.metadata);
    assert_eq!(std::fs::read(root.join("retained.png.capture/raw.bin")).unwrap(),*retained.raw);
    app.request(None);std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn repair_capture_time_render_and_frozen_export() {
    fn text(shape: &egui::epaint::Shape, out: &mut String) {
        match shape {
            egui::epaint::Shape::Text(t) => {
                out.push_str(&t.galley.job.text);
                out.push('\n');
            }
            egui::epaint::Shape::Vec(v) => {
                for s in v {
                    text(s, out);
                }
            }
            _ => {}
        }
    }
    let ctx = egui::Context::default();
    let mut app = App::with_exporter(&ctx, camera_lab::export);
    app.picker = false;
    app.tab = 4;
    let root = std::env::temp_dir().join(format!("repair-time-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    for stamp in [0, 1730000000123] {
        let mut frame = Simulation { sequence: 3 }.next_frame().unwrap();
        frame.metadata.unix_millis = stamp;
        app.show_frame(&ctx, frame.clone());
        // Overlay Areas perform an initial size pass before painting contents.
        draw(&mut app,&ctx,egui::vec2(1100.,750.));
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
        let mut rendered = String::new();
        for s in output.shapes {
            text(&s.shape, &mut rendered);
        }
        assert!(!rendered.contains("Texture update age:"),"technical timing starts collapsed");
        // A nonzero legacy timestamp without valid measurement provenance
        // must not be promoted to host completion or sensor exposure time.
        assert!(rendered.contains("Capture timing unavailable"));
        assert!(camera_lab::measurement::timestamp_semantics(&frame.metadata).contains("legacy unix_millis has unspecified semantics"));
        assert!(!rendered.contains("captured at 0"));
        assert!(!rendered.contains("host completion 1730000000123 ms"));
        app.request(None);
        camera_lab::export(&frame, &root.join(format!("{stamp}.png"))).unwrap();
        let saved: camera_lab::Metadata = serde_json::from_slice(
            &std::fs::read(root.join(format!("{stamp}.png.capture/metadata.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(saved, frame.metadata);
        assert_eq!(app.frame.as_ref().unwrap().metadata, frame.metadata);
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn retained_simulation_is_labeled_even_after_disconnect_or_source_failure() {
    fn has_badge(shape: &egui::epaint::Shape) -> bool {
        match shape {
            egui::epaint::Shape::Text(text) => {
                text.galley.job.text.contains("SIMULATION")
            }
            egui::epaint::Shape::Vec(shapes) => shapes.iter().any(has_badge),
            _ => false,
        }
    }
    let ctx = egui::Context::default();
    let mut app = App::with_exporter(&ctx, camera_lab::export);
    app.picker = false;
    app.show_frame(&ctx, Simulation { sequence: 7 }.next_frame().unwrap());
    app.request(None);
    for tab in 0..3 {
        app.tab = tab;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 550.),
                )),
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
        assert!(output.shapes.iter().any(|shape| has_badge(&shape.shape)));
        assert!(app.source.is_none());
    }
}

#[test]
fn frozen_export_survives_stop_and_close_with_exact_bundle_location() {
    let ctx = egui::Context::default();
    let mut app = App::with_exporter(&ctx, camera_lab::export);
    let root = std::env::temp_dir().join(format!("camera-ui-export-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let frozen = Simulation { sequence: 7 }.next_frame().unwrap();
    app.show_frame(&ctx, frozen.clone());
    for ext in ["png", "tiff"] {
        let path = root.join(format!("frozen.{ext}"));
        app.tx.try_send((frozen.clone(), path.clone(),Recipe::default())).unwrap();
        app.busy = true;
        app.request(None);
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.busy && Instant::now() < deadline {
            draw(&mut app, &ctx, egui::vec2(800., 550.));
            std::thread::yield_now();
        }
        assert!(!app.busy);
        let bundle = root.join(format!("frozen.{ext}.capture"));
        assert!(app.save_result.contains(&path.display().to_string()));
        assert!(!path.exists());
        assert!(bundle.join(path.file_name().unwrap()).exists());
        assert_eq!(std::fs::read(bundle.join("raw.bin")).unwrap(), *frozen.raw);
        let metadata: camera_lab::Metadata =
            serde_json::from_slice(&std::fs::read(bundle.join("metadata.json")).unwrap())
                .unwrap();
        assert_eq!(metadata, frozen.metadata);
        assert_eq!(
            image::open(bundle.join(path.file_name().unwrap()))
                .unwrap()
                .to_rgb8()
                .as_raw(),
            frozen.raw.as_ref()
        );
        assert!(camera_lab::export(&frozen, &path).is_err());
    }
    app.tx
        .try_send((frozen.clone(), root.join("closing.png"),Recipe::default()))
        .unwrap();
    app.busy = true;
    app.closing = true;
    app.worker.close();
    let deadline = Instant::now() + Duration::from_secs(3);
    while (app.busy || !app.worker.closed()) && Instant::now() < deadline {
        draw(&mut app, &ctx, egui::vec2(800., 550.));
        std::thread::yield_now();
    }
    assert!(!app.busy);
    assert!(app.worker.closed());
    assert!(root.join("closing.png.capture/COMPLETE").exists());
    assert_eq!(app.frame.as_ref().unwrap().metadata, frozen.metadata);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn failed_source_does_not_generate_or_substitute_frames() {
    struct Failed;
    impl camera_lab::session::Factory for Failed {
        fn open(
            &mut self,
            _: &Request,
        ) -> Result<Box<dyn camera_lab::session::Transport>, String> {
            Err("injected connection failure; no fallback".into())
        }
    }
    let ctx = egui::Context::default();
    let mut app = App::with_exporter(&ctx, camera_lab::export);
    app.worker = Worker::with_factory(Failed);
    app.request(Some(Source::Simulation));
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.session.state != camera_lab::session::State::Error && Instant::now() < deadline {
        draw(&mut app, &ctx, egui::vec2(800., 550.));
        std::thread::yield_now();
    }
    assert_eq!(app.session.state, camera_lab::session::State::Error);
    assert!(app.session.message.contains("injected connection failure"));
    assert!(app.frame.is_none());
    assert!(app.texture.is_none());
}
use super::*;
use camera_lab::{CameraAdapter, Simulation};
use std::time::Instant;
fn draw(app: &mut App, ctx: &egui::Context, size: egui::Vec2) {
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        },
        |ctx| app.render(ctx),
    );
    assert!(!output.shapes.is_empty());
}
#[test]
fn tabs_sizes_simulation_pause_and_disconnect_keep_truthful_frame() {
    let ctx = egui::Context::default();
    let mut app = App::with_exporter(&ctx, camera_lab::export);
    assert!(app.source.is_none());
    assert!(app.frame.is_none());
    assert!(ctx.style().visuals.dark_mode);
    for size in [
        egui::vec2(800., 550.),
        egui::vec2(1100., 750.),
        egui::vec2(1600., 900.),
    ] {
        for tab in 0..3 {
            app.tab = tab;
            draw(&mut app, &ctx, size);
            assert!(app.source.is_none());
        }
    }
    app.request(Some(Source::Simulation));
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.frame.is_none() && Instant::now() < deadline {
        draw(&mut app, &ctx, egui::vec2(800., 550.));
        std::thread::yield_now();
    }
    let frozen = app.frame.clone().expect("explicit simulation frame");
    assert!(frozen.metadata.simulated);
    assert!(app.texture.is_some());
    app.paused = true;
    for _ in 0..4 {
        draw(&mut app, &ctx, egui::vec2(800., 550.));
    }
    assert_eq!(app.frame.as_ref().unwrap().metadata, frozen.metadata);
    app.request(None);
    while app.session.active && Instant::now() < deadline {
        draw(&mut app, &ctx, egui::vec2(800., 550.));
    }
    assert_eq!(app.frame.as_ref().unwrap().metadata, frozen.metadata);
    assert!(app.source.is_none());
}
#[test]
fn invalid_controls_and_failed_export_do_not_change_display_or_select_simulation() {
    let ctx = egui::Context::default();
    let mut app = App::with_exporter(&ctx, camera_lab::export);
    let frame = Simulation { sequence: 0 }.next_frame().unwrap();
    let original = frame.metadata.clone();
    app.show_frame(&ctx, frame.clone());
    app.settings.exposure_rows = 0;
    // Validate the same request path used by physical Apply, without USB enumeration.
    let revision = app.revision;
    assert!(
        app.worker
            .request(Some(Request {
            startup_intent: camera_lab::startup::Intent::Normal,
                source: Source::Simulation,
                settings: app.settings
            }))
            .is_err()
    );
    assert_eq!(app.revision, revision);
    app.tx
        .try_send((frame, PathBuf::from("invalid.jpg"),Recipe::default()))
        .unwrap();
    app.busy = true;
    app.request(None); // A pending export owns its frozen frame through disconnect.
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.busy && Instant::now() < deadline {
        draw(&mut app, &ctx, egui::vec2(1100., 750.));
        std::thread::yield_now();
    }
    assert!(!app.busy);
    assert!(app.save_result.starts_with("Save failed:"));
    assert_eq!(app.frame.as_ref().unwrap().metadata, original);
    assert!(app.source.is_none());
}
#[test]
fn disconnected_panicked_export_and_scan_are_terminal_keep_last_frame() {
    let ctx = egui::Context::default();
    let mut app = App::with_exporter(&ctx, camera_lab::export);
    let frozen = Simulation { sequence: 42 }.next_frame().unwrap();
    app.show_frame(&ctx, frozen.clone());
    let (tx, rx) = mpsc::sync_channel(1);
    app.rx = rx;
    std::thread::spawn(move || {
        drop(tx);
        panic!("injected exporter panic");
    })
    .join()
    .unwrap_err();
    app.busy = true;
    let (tx, rx) = mpsc::sync_channel(1);
    drop(tx);
    app.scan_rx = Some(rx);
    draw(&mut app, &ctx, egui::vec2(800., 550.));
    assert!(!app.busy);
    assert!(app.scan_rx.is_none());
    assert!(app.save_result.contains("Export worker disconnected"));
    assert_eq!(app.frame.as_ref().unwrap().metadata, frozen.metadata);
    app.closing = true;
    app.worker.close();
    app.busy = true; // models a blocked filesystem worker with a connected channel
    app.close_started = Some(Instant::now() - Duration::from_secs(71));
    draw(&mut app, &ctx, egui::vec2(800., 550.));
    assert!(app.status.contains("Close deadline"));
    assert_eq!(app.frame.as_ref().unwrap().metadata, frozen.metadata);
}
#[test]
fn close_outer_deadline_uses_injected_exact_boundary() {
    let ctx = egui::Context::default();
    let mut app = App::with_exporter(&ctx, camera_lab::export);
    let zero = Instant::now();
    app.closing = true;
    app.close_started = Some(zero);
    for (elapsed, expired) in [
        (Duration::from_secs(70) - Duration::from_nanos(1), false),
        (Duration::from_secs(70), true),
    ] {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.render_at(ctx, || zero + elapsed)
        });
        assert_eq!(app.status.contains("Close deadline"), expired);
        assert_eq!(app.close_started, Some(zero));
    }
}
