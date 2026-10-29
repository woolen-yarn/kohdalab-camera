use super::*;
use camera_lab::display_recipe::Recipe;
fn draw(app:&mut App,ctx:&egui::Context,size:egui::Vec2) {
    let _=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,size)),..Default::default()},|ctx|app.render(ctx));
}
#[test]
fn forced_light_system_geometry_labels_and_footer() {
    let ctx=egui::Context::default();let mut app=App::without_scan(&ctx);app.picker=false;
    let size=egui::vec2(1100.,750.);
    let render=|app:&mut App|ctx.run(egui::RawInput{system_theme:Some(egui::Theme::Light),screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,size)),..Default::default()},|ctx|app.render(ctx));
    render(&mut app);let viewport=app.viewport_rect;
    assert_eq!(ctx.theme(),egui::Theme::Dark);
    assert_eq!(ctx.style().visuals.panel_fill,egui::Color32::from_rgb(17,19,24));
    assert_eq!(ctx.style().visuals.window_fill,egui::Color32::from_rgb(21,24,30));
    for tab in 0..5 {
        app.tab=tab;render(&mut app);let output=render(&mut app);
        assert_eq!(app.viewport_rect,viewport);
        let window=ctx.memory(|m|m.area_rect(egui::Id::new("settings-overlay"))).unwrap();
        assert!((window.left()-8.).abs()<0.1);assert!((window.top()-42.).abs()<0.1);
        assert!(window.width()>=340. && window.width()<365.,"{window:?}");
        let texts:Vec<_>=output.shapes.iter().filter_map(|s|if let egui::epaint::Shape::Text(t)=&s.shape {Some(t)} else {None}).collect();
        for (i,label) in parity_ui::TABS.iter().enumerate() {
            let t=texts.iter().find(|t|t.galley.text()==*label && t.pos.y<34.).unwrap();
            let center=t.pos.x+t.galley.size().x/2.;
            assert!((center-(53.+i as f32*98.)).abs()<1.,"{label} center {center}");
        }
        let labels:Vec<_>=texts.iter().map(|t|t.galley.text()).collect();
        assert!(!labels.iter().any(|t|t.contains("Unique preview camera frames") || t.contains("All GUI texture submits") || t.contains("Lifecycle:")),"technical counters should start collapsed");
        let expected=match tab {0=>vec!["Device","Reconnect","Change camera","Pause"],1=>vec!["Full sensor live mode · 3.1 MP","Exposure · sensor rows","Analog gain ×","Apply camera settings"],2=>vec!["Brightness ×","Auto brightness","Monochrome","Gamma","Mirror","Flip","Rotation","Set white balance","Reset white balance"],3=>vec!["Capture image…","Capture full sensor image… · 3.1 MP"],_=>vec![concat!("KohdaLab Camera · ",env!("CARGO_PKG_VERSION"))]};
        for label in expected {assert!(labels.contains(&label),"missing {label}: {labels:?}");}
    }
    app.status="failed ".repeat(1000);app.save_result="saved ".repeat(1000);app.cursor="cursor ".repeat(1000);
    render(&mut app);assert_eq!(app.viewport_rect,viewport);
    app.status="No held valid frame for scene estimate".into();app.save_result.clear();app.tab=usize::MAX;
    let output=render(&mut app);
    assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.text()==app.status)));
    app.picker=true;app.tab=2;render(&mut app);let output=render(&mut app);
    assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.text()=="Select a camera")));
    assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.text()=="Brightness ×")));
}
#[test]
fn overlay_preserves_full_window_fit_and_source_cursor() {
    let ctx=egui::Context::default();let mut app=App::without_scan(&ctx);app.picker=false;
    let f=Frame{raw:std::sync::Arc::new(vec![10,20,30,40,50,60]),metadata:camera_lab::Metadata{acquisition:None,source:"literal RGB".into(),simulated:true,sequence:1,measurement: None, unix_millis:0,width:2,height:1,format:"RGB8".into()}};
    app.show_frame(&ctx,f);
    for size in [egui::vec2(900.,620.),egui::vec2(1100.,750.),egui::vec2(1600.,1000.)] {
        app.tab=usize::MAX;draw(&mut app,&ctx,size);let viewport=app.viewport_rect;
        for tab in 0..5 {app.tab=tab;draw(&mut app,&ctx,size);assert_eq!(app.viewport_rect,viewport);}
        let rect=parity_ui::aspect_fit(viewport,egui::vec2(2.,1.));
        assert_eq!(rect.center(),viewport.center());assert!((rect.width()/rect.height()-2.).abs()<0.0001);
        assert!(viewport.contains_rect(rect));
    }
    for turns in 0..4 {for mirror in [false,true] {for flip in [false,true] {
        let r=Recipe{quarter_turns:turns,mirror,flip,..Default::default()};let (w,h)=r.dimensions(2,1);
        for y in 0..h {for x in 0..w {let (sx,sy)=r.source(x,y,2,1);assert!(sx<2 && sy<1);}}
    }}}
    app.tab=usize::MAX;app.paused=true;app.recipe=Recipe{ev:1.,quarter_turns:1,..Recipe::legacy()};app.recipe_changed();
    let end=std::time::Instant::now()+Duration::from_secs(5);
    while app.displayed_recipe!=app.recipe {assert!(std::time::Instant::now()<end);draw(&mut app,&ctx,egui::vec2(900.,620.));std::thread::yield_now();}
    let rect=parity_ui::aspect_fit(app.viewport_rect,egui::vec2(1.,2.));
    let pointer=egui::pos2(rect.center().x,rect.top()+rect.height()*0.25);
    let _=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(900.,620.))),events:vec![egui::Event::PointerMoved(pointer)],..Default::default()},|ctx|app.render(ctx));
    assert_eq!(app.cursor,"Cursor (0, 0) · RGB (20, 40, 60)");
}
#[test]
fn frozen_recipe_primary_png_tiff_literal_pixels_and_raw_unchanged() {
    let ctx=egui::Context::default();let mut app=App::without_scan(&ctx);app.picker=false;
    let root=std::env::temp_dir().join(format!("v023-frozen-{}",std::process::id()));std::fs::create_dir(&root).unwrap();
    let f=Frame{raw:std::sync::Arc::new(vec![0,10,20,30, 40,50,60,70]),metadata:camera_lab::Metadata{acquisition:None,source:"independent two GR/BG cells".into(),simulated:true,sequence:4,measurement: None, unix_millis:0,width:4,height:2,format:"BayerGRBG8".into()}};
    // Cell A RGB [10,(0+50)/2,40], B [30,(20+70)/2,60]. EV +1 then clockwise 90°:
    let expected=[20,50,80,20,50,80,20,50,80,20,50,80,60,90,120,60,90,120,60,90,120,60,90,120];
    let recipe=Recipe{ev:1.,quarter_turns:1,..Recipe::legacy()};
    for ext in ["png","tiff"] {
        let path=root.join(format!("frozen.{ext}"));app.displayed_recipe=recipe;
        app.save_frozen(f.clone(),path.clone());app.recipe=Recipe{ev:-4.,monochrome:true,..Default::default()};app.request(None);
        let deadline=std::time::Instant::now()+Duration::from_secs(10);
        while app.busy {assert!(std::time::Instant::now()<deadline);draw(&mut app,&ctx,egui::vec2(900.,620.));std::thread::yield_now();}
        let decoded=image::open(&path).unwrap().to_rgb8();assert_eq!(decoded.dimensions(),(2,4));assert_eq!(decoded.as_raw(),&expected);
        assert_eq!(std::fs::read(format!("{}.raw",path.display())).unwrap(),*f.raw);
        let metadata:serde_json::Value=serde_json::from_slice(&std::fs::read(format!("{}.json",path.display())).unwrap()).unwrap();
        assert_eq!(metadata["frame"],serde_json::to_value(&f.metadata).unwrap());assert_eq!(metadata["display_recipe"],serde_json::to_value(recipe).unwrap());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn image_zoom_is_presentation_only_and_panel_wheel_does_not_zoom() {
    let ctx=egui::Context::default();let mut app=App::without_scan(&ctx);app.picker=false;app.tab=usize::MAX;
    let frame=camera_lab::CameraAdapter::next_frame(&mut camera_lab::Simulation{sequence:1}).unwrap();
    let raw=frame.raw.clone();let meta=frame.metadata.clone();app.show_frame(&ctx,frame);let recipe=app.recipe;
    let size=egui::vec2(1100.,760.);draw(&mut app,&ctx,size);
    let pointer=egui::pos2(800.,400.);
    let input=|events|egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,size)),events,..Default::default()};
    let _=ctx.run(input(vec![egui::Event::PointerMoved(pointer)]),|ctx|app.render(ctx));
    let _=ctx.run(input(vec![egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:egui::vec2(0.,120.),modifiers:Default::default()}]),|ctx|app.render(ctx));
    assert!(app.view.zoom>1.);assert_eq!(app.recipe,recipe);assert_eq!(app.frame.as_ref().unwrap().raw,raw);assert_eq!(app.frame.as_ref().unwrap().metadata,meta);
    app.tab=2;draw(&mut app,&ctx,size);draw(&mut app,&ctx,size);
    let pointer=egui::pos2(150.,250.);let _=ctx.run(input(vec![egui::Event::PointerMoved(pointer)]),|ctx|app.render(ctx));
    let zoom=app.view.zoom;
    let _=ctx.run(input(vec![egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:egui::vec2(0.,120.),modifiers:Default::default()}]),|ctx|app.render(ctx));
    assert_eq!(app.view.zoom,zoom,"scrolling controls must not zoom the canvas");
    app.worker.close();
}

#[test]
fn preferences_restore_and_reset_do_not_connect_or_apply() {
    let ctx=egui::Context::default();let mut app=App::without_scan(&ctx);
    let saved=preferences::Preferences{exposure:155,gain:16,live_size:[1234.,800.],recipe:Recipe{gamma:1.8,..Default::default()},next_sequence:42,..Default::default()};
    app.restore_preferences(saved.clone());assert_eq!(app.current_preferences(),saved);
    assert!(app.source.is_none() && app.picker);assert_eq!(app.revision,0);
    app.reset_preferences(&ctx);assert_eq!(app.current_preferences(),preferences::Preferences::default());assert!(app.source.is_none());assert_eq!(app.revision,0);
    app.worker.close();
}

#[test]
fn failed_preference_write_can_retry_without_losing_changes() {
    let root=std::env::temp_dir().join(format!("camera-pref-retry-{}",std::process::id()));std::fs::create_dir_all(&root).unwrap();
    let blocked=root.join("blocked");std::fs::write(&blocked,b"file").unwrap();
    let ctx=egui::Context::default();let mut app=App::without_scan(&ctx);app.preferences_path=Some(blocked.join("settings.json"));app.settings.exposure_rows=155;
    app.persist_preferences(true);assert_eq!(app.saved_preferences.exposure,120);
    std::fs::remove_file(&blocked).unwrap();app.persist_preferences(true);
    assert_eq!(preferences::Preferences::load(app.preferences_path.as_ref().unwrap()).unwrap().exposure,155);
    app.worker.close();std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn footer_cursor_starts_at_the_left_without_reserved_space() {
    let ctx=egui::Context::default();let mut app=App::without_scan(&ctx);app.picker=false;app.tab=usize::MAX;
    let frame=camera_lab::CameraAdapter::next_frame(&mut camera_lab::Simulation{sequence:1}).unwrap();app.show_frame(&ctx,frame);
    let size=egui::vec2(1100.,760.);draw(&mut app,&ctx,size);
    let input=|events|egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,size)),events,..Default::default()};
    let _=ctx.run(input(vec![egui::Event::PointerMoved(egui::pos2(800.,400.))]),|ctx|app.render(ctx));
    let out=ctx.run(input(vec![]),|ctx|app.render(ctx));
    let text=out.shapes.iter().find_map(|s|match &s.shape {egui::epaint::Shape::Text(t) if t.galley.text().starts_with("Cursor (")=>Some(t),_=>None}).unwrap();
    assert!(text.pos.x<=8.,"cursor text has an unwanted leading gap: {}",text.pos.x);
    let (text_left,row_left)=ctx.data(|d|d.get_temp::<(f32,f32)>(egui::Id::new("cursor-left-edge"))).unwrap();assert!((text_left-row_left).abs()<0.1);
    app.worker.close();
}

#[test]
fn idle_windows_stop_polling_but_pending_work_keeps_updates() {
    let ctx=egui::Context::default();let mut app=App::without_scan(&ctx);
    assert!(!app.needs_periodic_updates());app.busy=true;assert!(app.needs_periodic_updates());app.busy=false;
    app.source=Some(Source::Simulation);assert!(app.needs_periodic_updates());app.source=None;
    app.closing=true;assert!(app.needs_periodic_updates());app.worker.close();
}
