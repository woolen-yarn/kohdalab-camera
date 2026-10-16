//! Presentation only. Worker snapshots remain the cleanup/restart authority.
use super::*;
impl App {
    pub(super) fn refreshed_selection<T:PartialEq>(selected:Option<T>,identities:impl Iterator<Item=T>)->Option<usize> {selected.and_then(|id|identities.enumerate().find_map(|(i,d)|(d==id).then_some(i)))}
    pub(super) fn can_connect(&self) -> bool {
        !self.closing && !self.busy && !self.connection_fault && !self.session.active
            && !self.session.restart_disabled
            && self.session.state==camera_lab::session::State::Disconnected
            && self.session.requested_revision.is_none()
    }
    fn picker_connect_ready(&self)->bool {self.can_connect() && (self.picker_simulation || (self.scan_rx.is_none() && self.selected.and_then(|i|self.devices.get(i)).is_some()))}
    fn connect_picker(&mut self) {if self.picker_connect_ready() {if self.picker_simulation {self.request(Some(Source::Simulation));} else if let Some(d)=self.selected.and_then(|i|self.devices.get(i)).cloned() {self.request(Some(Source::Physical(d)));}}}
    pub(super) fn change_camera(&mut self) {
        self.request(None);
        self.picker=true;
    }
    pub(super) fn sync_stage(&mut self,ctx:&egui::Context) {
        // Remember valid Live measurements before the picker resize is requested.
        // Ignore missing/stale picker-sized reports during native transitions.
        if !self.viewport_picker {
            if let Some(size)=ctx.input(|i|i.viewport().inner_rect.map(|r|r.size()))
                .filter(|s|s.x.is_finite() && s.y.is_finite() && s.x>=900. && s.y>=620.) {
                self.live_size=size;
            }
        }
        if self.viewport_picker==self.picker {return;}
        self.viewport_picker=self.picker;
        let (title,min,size)=if self.picker {
            ("Select a camera — KohdaLab Camera",egui::vec2(520.,360.),egui::vec2(640.,480.))
        } else {("Live image — KohdaLab Camera",egui::vec2(900.,620.),self.live_size)};
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.into()));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(min));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
    }
    pub(super) fn primary_state(&self)->&'static str {
        use camera_lab::session::State;
        if self.closing || self.session.state==State::Stopping || (self.source.is_none() && self.session.active) {"Stopping"}
        else if self.product.busy() && self.product.held.is_some() {"Returning to Live"}
        else if self.product.busy() {"Capturing Full"}
        else if self.busy {"Saving"}
        else if self.source.is_none() || self.session.state==State::Error
            || (self.session.state==State::Disconnected && (self.session.requested_revision==Some(self.revision) || self.matching_revision==Some(self.revision))) {"Disconnected"}
        else if self.matching_revision!=Some(self.revision) || self.displayed_revision!=Some(self.revision) {
            if self.session.state==State::SettingsPending {"Applying"} else {"Connecting"}
        }
        else if self.paused {"Paused (acquisition continues)"}
        else {"Live"}
    }
    pub(super) fn has_failure_panel(&self)->bool {
        self.session.first_failure.is_some() || self.terminal_preview_failure.is_some() || self.control_error.is_some() || self.outcome_fault.is_some()
    }
    fn pinned_failure_summary(&self)->String {
        if self.session.first_failure.is_some() {
            return format!("First cause: {}",camera_lab::diagnostic::summary(self.session.first_cause));
        }
        let (source,error)=if let Some(e)=self.terminal_preview_failure.as_deref() {("Preview",e)}
            else if let Some(e)=self.control_error.as_deref() {("Control",e)}
            else {("Outcome",self.outcome_fault.as_deref().unwrap_or(""))};
        // Bound before normalization; no unbounded copy of existing UI prose.
        let text=camera_lab::failure_record::bounded(error).split_whitespace().collect::<Vec<_>>().join(" ");
        format!("{source}: {}{}",if text.is_empty() {"Error details unavailable"} else {&text},if error.len()>2048 {"…"} else {""})
    }
    pub(super) fn failure_visibility(&self,ctx:&egui::Context)->Option<(egui::Rect,egui::Vec2)> {
        if self.has_failure_panel() {
            let mut geometry=None;
            egui::TopBottomPanel::top("failure-details").show(ctx,|ui| {
                ui.colored_label(egui::Color32::LIGHT_RED,"Error details — lifecycle proof remains required");
                ui.add(egui::Label::new(&self.session.persistence).wrap());
                let summary=self.pinned_failure_summary();
                let summary=ui.add(egui::Label::new(summary).truncate());
                #[cfg(test)] ctx.data_mut(|d|d.insert_temp(egui::Id::new("v035-summary-geometry"),(summary.rect,ui.clip_rect())));
                if let Some(backend)=self.session.lifecycle.as_ref().and_then(|l|l.backend_cause) {
                    ui.add(egui::Label::new(format!("Backend detail: {}",camera_lab::diagnostic::summary(Some(backend)))).wrap());
                }
                ui.small("Full bounded first cause and lifecycle details (scroll)");
                let scroll=egui::ScrollArea::vertical().id_salt("failure-scroll").auto_shrink([false,false]).max_height((ctx.screen_rect().height()*0.3).clamp(48.,160.)).show(ui,|ui| {
                    ui.add(egui::Label::new(self.session.first_failure.as_deref().unwrap_or("No Worker failure captured")).wrap());
                    ui.add(egui::Label::new(self.outcome_detail()).wrap());
                    ui.label(format!("Lifecycle: {:?}",self.session.lifecycle));
                });
                geometry=Some((scroll.inner_rect,scroll.state.offset));
            });
            #[cfg(test)] if let Some(g)=geometry {ctx.data_mut(|d|d.insert_temp(egui::Id::new("v034-error-geometry"),g));}
            geometry
        } else {None}
    }
    pub(super) fn outcome_detail(&self)->String {
        [self.terminal_preview_failure.as_deref().unwrap_or(""),self.control_error.as_deref().unwrap_or(""),self.outcome_fault.as_deref().unwrap_or(""),
            &self.status,&self.save_result,&self.product.capture_result,&self.product.restore_result,&self.session.message].into_iter().filter(|s|!s.is_empty()).collect::<Vec<_>>().join("\n")
    }
    pub(super) fn concise_outcome(&self)->(String,bool) {
        if let Some(e)=self.terminal_preview_failure.as_ref() {return ("Preview unavailable. Close the app and inspect the error in Error details.".into(),!e.is_empty());}
        let restore_bad=self.product.restore_result.contains("unverified") || self.product.restore_result.contains("Interrupted");
        if let Some(e)=self.control_error.as_ref().or(self.outcome_fault.as_ref()) {
            let lower=e.to_lowercase();
            let save_bad=lower.contains("save") || lower.contains("sidecar") || lower.contains("incomplete");
            return (if save_bad && restore_bad {"Save failed · return to Live unconfirmed. See Error details."}
                else if lower.contains("sidecar") || lower.contains("incomplete") {"INCOMPLETE · image/sidecar save failed. See Error details."}
                else if save_bad && self.product.restore_result.starts_with("Original applied") {"Image save failed · Live restored. Check the destination."}
                else if save_bad {"Image save failed. Check the destination in Error details."}
                else {"Camera operation failed. Stop and check Error details before reconnecting."}.into(),true);
        }
        let restore=&self.product.restore_result;
        if restore_bad {return (if self.save_result.starts_with("Saved image") {"Full image saved · return to Live unconfirmed"} else {"Return to Live not confirmed · see Error details"}.into(),true);}
        if self.save_result.starts_with("Saved image") {
            return (if self.export_full && restore.starts_with("Original applied") {"Full image saved · Live restored"} else if self.product.busy() {"Full image saved · returning to Live"} else {"Image and sidecars saved"}.into(),false);
        }
        if self.busy {return ("Saving image and sidecars…".into(),false);}
        if self.save_result.contains("cancelled") {return ("Save cancelled".into(),false);}
        if self.source.is_none() && self.frame.is_some() {return ("Retained image · historical".into(),false);}
        (self.status.clone(),false)
    }
    pub(super) fn selector_ui(&mut self,ctx:&egui::Context) {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(ctx.style().visuals.panel_fill).inner_margin(8.)).show(ctx,|ui| {
            ui.heading("Select a camera");
            ui.label("Choose the camera to open. You can change it later from the capture window.");
            ui.separator();
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.picker_simulation,false,"USB camera");
                ui.radio_value(&mut self.picker_simulation,true,"Simulation / demo");
                if ui.add_enabled(!self.closing && self.scan_rx.is_none(),egui::Button::new("Refresh")).clicked() {self.scan();}
            });
            if !self.picker_simulation {
                egui::ScrollArea::vertical().max_height((ui.available_height()-120.).max(48.)).show(ui,|ui| {
                    if self.devices.is_empty() {ui.label(if self.scan_rx.is_some() {"Looking for cameras…"} else {"No eligible camera found"});}
                    for (i,d) in self.devices.iter().enumerate() {
                        let text=format!("{} · {:04x}:{:04x}",d.product_string().unwrap_or("Supported camera"),d.vendor_id(),d.product_id());
                        if ui.add_enabled(!self.closing,egui::Button::new(text).selected(self.selected==Some(i)).min_size(egui::vec2(ui.available_width(),34.))).on_hover_text(format!("{d:?}")).clicked() {self.selected=Some(i);}
                    }
                    ui.collapsing("Other USB devices",|ui| {for d in &self.other_devices {ui.label(format!("{} · {:04x}:{:04x} (unsupported)",d.product_string().unwrap_or("USB device"),d.vendor_id(),d.product_id())).on_hover_text(format!("{d:?}"));}});
                });
            } else {ui.label("Software simulation · no camera required");}
            ui.separator();
            let ready=self.can_connect();
            if ui.add_enabled(self.picker_connect_ready(),egui::Button::new("Connect and open capture window")).clicked() {self.connect_picker();}
            ui.separator();
            if !ready {ui.label(if self.connection_fault || self.session.restart_disabled || self.session.state==camera_lab::session::State::Error {"Camera operation failed. Reconnection is disabled; see the error below."} else if self.busy {"Waiting for the image save to finish…"} else {"Stopping the previous camera…"});}
            else if !self.picker_simulation && self.selected.is_none() {ui.small("Select a supported camera to connect.");}
            if let Some(e)=&self.scan_error {ui.colored_label(egui::Color32::LIGHT_RED,e);}
            if self.control_error.is_some() || self.outcome_fault.is_some() || self.terminal_preview_failure.is_some() {
                let (text,_)=self.concise_outcome();ui.colored_label(egui::Color32::LIGHT_RED,text).on_hover_text(self.outcome_detail());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camera_lab::session::{Factory,Transport,Event};
    use std::{sync::{Arc,atomic::{AtomicBool,Ordering}},time::Instant};
    struct Fake(Arc<AtomicBool>);
    struct Owner {measurement:camera_lab::measurement::Source,next:Instant,stop:bool,gate:Arc<AtomicBool>,revision:u64,settings:Settings,sequence:u64,intent:camera_lab::startup::Intent}
    impl Factory for Fake {
        fn open(&mut self,r:&Request)->Result<Box<dyn Transport>,String> {self.open_generation(r,1)}
        fn open_generation(&mut self,r:&Request,revision:u64)->Result<Box<dyn Transport>,String> {
            Ok(Box::new(Owner{measurement:Default::default(),next:Instant::now(),stop:false,gate:self.0.clone(),revision,settings:r.settings,sequence:0,intent:r.startup_intent}))
        }
    }
    impl Transport for Owner {
        fn stop(&mut self)->Result<(),String> {self.stop=true;Ok(())}
        fn poll(&mut self)->Result<Event,String> {
            if self.stop {return Ok(if self.gate.load(Ordering::SeqCst) {Event::Done{clean:true}} else {Event::Waiting});}
            if Instant::now()<self.next {return Ok(Event::Waiting);}
            self.next=Instant::now()+Duration::from_millis(40);
            self.sequence+=1;
            self.measurement.complete();
            let (w,h)=self.settings.dimensions();
            let mut a=camera_lab::sensor_contract::Acquisition::new(self.settings,[self.revision as u8;32],self.revision,self.sequence)?.with_startup(self.intent);
            a.request_revision=Some(self.revision);
            let mut frame=Frame{raw:Arc::new(vec![(self.sequence%200+20) as u8;usize::from(w)*usize::from(h)]),metadata:camera_lab::Metadata{acquisition:Some(a),source:"v026 deterministic fake".into(),simulated:true,sequence:self.sequence,measurement: None, unix_millis:0,width:w.into(),height:h.into(),format:"BayerGRBG8".into()}};
            self.measurement.bind(&mut frame);Ok(Event::Frame(frame))
        }
    }
    fn draw(app:&mut App,ctx:&egui::Context)->egui::FullOutput {
        ctx.run(egui::RawInput::default(),|ctx|app.render(ctx))
    }
    fn until(app:&mut App,ctx:&egui::Context,p:impl Fn(&App)->bool) {
        let end=Instant::now()+Duration::from_secs(30);
        while !p(app) {assert!(Instant::now()<end,"{} / {}",app.primary_state(),app.outcome_detail());draw(app,ctx);std::thread::sleep(Duration::from_millis(5));}
    }
    fn sizes(out:&egui::FullOutput)->Vec<egui::Vec2> {
        out.viewport_output.values().flat_map(|v|&v.commands).filter_map(|c|if let egui::ViewportCommand::InnerSize(s)=c {Some(*s)} else {None}).collect()
    }

    struct ReportFactory;
    struct ReportTransport {a:camera_lab::diagnostic::Accumulator,f:camera_lab::diagnostic::Facts,reported:bool}
    impl Factory for ReportFactory {
      fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{Ok(Box::new(ReportTransport{a:Default::default(),f:Default::default(),reported:false}))}
    }
    impl Transport for ReportTransport {
      fn stop(&mut self)->Result<(),String>{Ok(())}
      fn lifecycle(&self)->Option<camera_lab::session::Lifecycle>{Some(camera_lab::diagnostic::lifecycle(&self.f,&self.a))}
      fn poll(&mut self)->Result<Event,String>{
       use camera_lab::diagnostic as d;
       let mut p=[0;128];p[68]=16;p[69]=if self.reported {5}else{4};
       if !self.reported {d::write_terminal_error(&mut p,Some("@OP SENTINEL_BUS /Users/SENTINEL_USER"));}
       d::report_terminal(&mut self.a,&mut self.f.parent_error,&p).unwrap();
       if self.reported {self.f.collected=true;self.f.terminal=Some(d::Terminal::FailedClean);self.f.exit=Some(2);}
       Ok(d::transport_event(&d::lifecycle(&self.f,&self.a),&mut self.reported,"SENTINEL_PORT").unwrap_or(Event::Waiting))
      }
    }
    #[test]
    fn v035_failure_geometry_and_retained_scroll() {
        let mut records=Vec::new();
        for (picker,tab) in [(true,0),(false,usize::MAX),(false,4)] {
        for size in [egui::vec2(520.,360.),egui::vec2(640.,480.),egui::vec2(900.,620.)] {
            let root=std::env::temp_dir().join(format!("v037-panel-{}-{picker}-{tab}-{}",std::process::id(),size.x));
            let worker=Worker::with_factory_recorder(ReportFactory,camera_lab::failure_record::Recorder::at(root.clone()));
            worker.request(Some(Request{source:Source::Simulation,settings:camera_lab::controls::Settings::default(),startup_intent:camera_lab::startup::Intent::Normal})).unwrap();
            let end=Instant::now()+Duration::from_secs(5);while worker.snapshot().first_failure.is_none(){assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(2));}
            worker.close();while !worker.closed(){assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(2));}let mut guard=worker.guardian();guard.wait();
            let ctx=egui::Context::default();let mut app=App::with_exporter_worker(&ctx,|_,_|Ok(()),worker);
            app.session=app.worker.snapshot();let cause=app.session.first_cause.unwrap();assert_eq!(cause,camera_lab::diagnostic::backend_code("@OP x"));
            assert!(app.session.first_failure.as_deref().unwrap().contains("SENTINEL_BUS"));
            let mut first=None;for name in ["first.json","collection.json"] {
             let raw=std::fs::read_to_string(root.join("slot-00").join(name)).unwrap();for sentinel in ["SENTINEL_BUS","SENTINEL_PORT","SENTINEL_USER","/Users/"]{assert!(!raw.contains(sentinel));}
             let v:serde_json::Value=serde_json::from_str(&raw).unwrap();assert_eq!(v["first_failure"]["cause"],serde_json::json!(cause));if let Some(ref first)=first{assert_eq!(&v["first_failure"],first);}else{first=Some(v["first_failure"].clone());}
            }
            app.picker=picker;app.tab=tab;app.session.persistence="FAILURE RECORD NOT PERSISTED: injected denial".into();app.session.message=(0..60).map(|i|format!("Lifecycle detail {i}\n")).collect();
            let input=|events|egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,size)),events,..Default::default()};
            let mut geo=None;
            for _ in 0..3 {let _=ctx.run(input(vec![]),|ctx|{app.parity_ui(ctx,Instant::now());geo=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Vec2)>(egui::Id::new("v034-error-geometry")));});}
            let (rect,offset)=geo.unwrap();assert!(rect.min.x>=0. && rect.min.y>=0. && rect.max.x<=size.x && rect.max.y<=size.y);assert_eq!(offset.y,0.);
            let mut routed=None;
            for _ in 0..3 {let _=ctx.run(input(vec![egui::Event::PointerMoved(rect.center()),egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:egui::vec2(0.,-120.),modifiers:Default::default()}]),|ctx|{app.parity_ui(ctx,Instant::now());routed=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Vec2)>(egui::Id::new("v034-error-geometry")));});}
            assert!(routed.unwrap().1.y>0.,"wheel routing: initial={:?}, final={:?}, pointer={:?}, delta={:?}, layer={:?}",geo,routed,ctx.input(|i|i.pointer.hover_pos()),ctx.input(|i|i.smooth_scroll_delta),ctx.layer_id_at(rect.center()));
            let before=routed.unwrap().1.y;let mut outside=None;
            let out=ctx.run(input(vec![egui::Event::PointerMoved(egui::pos2(size.x-10.,size.y-10.)),egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:egui::vec2(0.,-120.),modifiers:Default::default()}]),|ctx|{app.parity_ui(ctx,Instant::now());outside=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Vec2)>(egui::Id::new("v034-error-geometry")));});
            assert_eq!(outside.unwrap().1.y,before,"outside wheel must not move error details");
            assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.text().contains("Error details"))));
            let check_summary=|out:&egui::FullOutput| {
                let (rect,clip)=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Rect)>(egui::Id::new("v035-summary-geometry"))).unwrap();
                assert_eq!(ctx.layer_id_at(rect.center()).unwrap().order,egui::Order::Background,"summary must not be covered by settings overlay");
                assert!(clip.contains_rect(rect));assert!(egui::Rect::from_min_size(egui::Pos2::ZERO,size).contains_rect(rect));
                let shape=out.shapes.iter().find_map(|s|match &s.shape {egui::epaint::Shape::Text(t) if t.galley.text().starts_with("First cause: Backend / Open / Operation failed")=>Some((s.clip_rect,t)),_=>None}).expect("useful cause text, not heading");
                let text_rect=egui::Rect::from_min_size(shape.1.pos,shape.1.galley.size());
                assert!(shape.0.contains_rect(text_rect),"summary painted text must be visible: {text_rect:?} clip {:?}",shape.0);
                assert!(rect.contains_rect(text_rect));assert!(text_rect.height()<30.);
            };
            check_summary(&out);
            let retained=outside.unwrap().1.y;
            for (next_picker,next_tab) in [(true,0),(false,0),(false,4),(picker,tab)] {
                app.picker=next_picker;app.tab=next_tab;
                let out=ctx.run(input(vec![]),|ctx|app.parity_ui(ctx,Instant::now()));check_summary(&out);
                let (_,offset)=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Vec2)>(egui::Id::new("v034-error-geometry"))).unwrap();assert_eq!(offset.y,retained,"retained scroll across picker/live/Advanced");
            }
            let (summary_rect,summary_clip)=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Rect)>(egui::Id::new("v035-summary-geometry"))).unwrap();
            records.push(serde_json::json!({"summary_rect":[summary_rect.min.x,summary_rect.min.y,summary_rect.max.x,summary_rect.max.y],"summary_clip":[summary_clip.min.x,summary_clip.min.y,summary_clip.max.x,summary_clip.max.y],"summary_occlusion_checked":true,"retained_navigation_checked":true,"viewport":[size.x,size.y],"picker":picker,"tab":tab,"error_rect":[rect.min.x,rect.min.y,rect.max.x,rect.max.y],"inside_wheel_offset":before,"outside_wheel_offset":outside.unwrap().1.y,"physical":false,"input":"egui native PointerMoved and MouseWheel events"}));
            assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.text().contains("FAILURE RECORD NOT PERSISTED"))));
            app.worker.close();std::fs::remove_dir_all(root).unwrap();
        }}
        if let Some(path)=std::env::var_os("V035_GEOMETRY") {std::fs::write(path,serde_json::to_vec_pretty(&records).unwrap()).unwrap();}
    }
    #[test]
    fn v039_parent_then_sensor_ui_and_durable_records() {
        use camera_lab::diagnostic as d;
        let root=std::env::temp_dir().join(format!("v039-ui-record-{}",std::process::id()));
        let mut recorder=camera_lab::failure_record::Recorder::at(root.clone());
        let mut a=d::Accumulator::default();let mut first=None;
        d::report_parent(&mut a,&mut first,d::ParentReport::ChallengeLost);
        let ctx=egui::Context::default();let mut app=App::with_exporter_worker(&ctx,|_,_|Ok(()),Worker::with_factory(Fake(Arc::new(AtomicBool::new(true)))));
        app.session.first_failure=first.clone();app.session.first_cause=a.first;app.session.active=true;
        app.session.lifecycle=Some(d::lifecycle(&d::Facts::default(),&a));recorder.observe(&mut app.session);
        let mut p=[0;128];p[68]=21;
        d::write_terminal_error(&mut p,Some(&format!("{} {}",d::SensorFailure::ReadLength.text(),"SENTINEL/path".repeat(20))));
        assert_eq!(p[70],57);d::report_terminal(&mut a,&mut first,&p).unwrap();
        assert_eq!(first.as_deref(),Some(d::ParentReport::ChallengeLost.text()));
        app.session.lifecycle=Some(d::lifecycle(&d::Facts{collected:true,terminal:Some(d::Terminal::Unproved),exit:Some(0),..Default::default()},&a));
        app.session.active=false;recorder.finish(&mut app.session);
        let expected="Backend detail: Backend / Sensor / Operation failed / read length";
        for size in [egui::vec2(520.,360.),egui::vec2(900.,620.)] {
          let input=||egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,size)),..Default::default()};
          for _ in 0..3 {let _=ctx.run(input(),|ctx|{app.failure_visibility(ctx);});}
          let out=ctx.run(input(),|ctx|{app.failure_visibility(ctx);});
          for expected in ["First cause: Parent / Channel / Lost",expected] {
            let (clip,text)=out.shapes.iter().find_map(|s|match &s.shape {egui::epaint::Shape::Text(t) if t.galley.text()==expected=>Some((s.clip_rect,t)),_=>None}).expect("painted fixed summary");
            let rect=egui::Rect::from_min_size(text.pos,text.galley.size());assert!(clip.contains_rect(rect));assert!(egui::Rect::from_min_size(egui::Pos2::ZERO,size).contains_rect(rect));
          }
        }
        let read=|name|{let raw=std::fs::read_to_string(root.join("slot-00").join(name)).unwrap();assert!(!raw.contains("SENTINEL"));serde_json::from_str::<serde_json::Value>(&raw).unwrap()};
        let before=read("first.json");let after=read("collection.json");
        assert_eq!(before["first_failure"],after["first_failure"]);assert!(before["backend_cause"].is_null());
        assert_eq!(after["backend_cause"],serde_json::json!(a.backend));assert_eq!(after["backend_cause"]["category"],2);
        assert_eq!(after["release_committed"],false);assert_eq!(after["marker_deleted"],false);assert!(after["marker_present"].is_null());
        assert_eq!(after["candidate"],concat!("kohdalab-camera-",env!("CARGO_PKG_VERSION")));
        app.worker.close();std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn v038_worker_summary_precedence() {
        use camera_lab::diagnostic as d;
        for case in 0..3 {
            let mut acc=d::Accumulator::default();let mut first=None;
            if case==2 {d::report_parent(&mut acc,&mut first,d::ParentReport::OwnerUnresponsive);}
            let mut packet=[0;128];packet[68]=16;packet[69]=4;
            if case!=1 {d::write_terminal_error(&mut packet,Some("@OP backend detail"));}
            d::report_terminal(&mut acc,&mut first,&packet).unwrap();
            let ctx=egui::Context::default();let mut app=App::with_exporter_worker(&ctx,|_,_|Ok(()),Worker::with_factory(Fake(Arc::new(AtomicBool::new(true)))));
            app.session.first_failure=first;app.session.first_cause=acc.first;
            app.terminal_preview_failure=Some("preview must not replace Worker".into());app.control_error=Some("control must not replace Worker".into());
            let expected=["First cause: Backend / Open / Operation failed","First cause: Helper / Supervisor / Supervisor fault","First cause: Parent / Owner / Unresponsive"][case];
            assert_eq!(app.pinned_failure_summary(),expected);
            for _ in 0..3 {let _=ctx.run(egui::RawInput::default(),|ctx|{app.failure_visibility(ctx);});}
            let out=ctx.run(egui::RawInput::default(),|ctx|{app.failure_visibility(ctx);});
            assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.text()==expected)));
            app.worker.close();
        }
    }
    #[test]
    fn v038_nonworker_failure_geometry_and_retained_scroll() {
        let mut records=Vec::new();
        for source in ["Preview","Control","Outcome"] {
        for (picker,tab) in [(true,0),(false,usize::MAX),(false,4)] {
        for size in [egui::vec2(520.,360.),egui::vec2(640.,480.),egui::vec2(900.,620.)] {
            let root=std::env::temp_dir().join(format!("v038-panel-{}-{source}-{picker}-{tab}-{}",std::process::id(),size.x));
            std::fs::create_dir_all(&root).unwrap();
            let worker=Worker::with_factory_recorder(Fake(Arc::new(AtomicBool::new(true))),camera_lab::failure_record::Recorder::at(root.clone()));
            let ctx=egui::Context::default();let mut app=App::with_exporter_worker(&ctx,|_,_|Ok(()),worker);
            assert!(app.session.first_failure.is_none());
            let error=format!("Useful existing error\n{}", "é".repeat(3000));
            match source {"Preview"=>app.terminal_preview_failure=Some(error),"Control"=>app.control_error=Some(error),_=>app.outcome_fault=Some(error)};
            let summary=app.pinned_failure_summary();assert!(summary.starts_with(&format!("{source}: Useful existing error ")));assert!(summary.ends_with('…'));assert!(summary.len()<2100);assert!(!summary.contains('\n'));
            app.picker=picker;app.tab=tab;app.session.persistence="FAILURE RECORD NOT PERSISTED: injected denial".into();app.session.message=(0..60).map(|i|format!("Lifecycle detail {i}\n")).collect();
            let input=|events|egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,size)),events,..Default::default()};
            let mut geo=None;
            for _ in 0..3 {let _=ctx.run(input(vec![]),|ctx|{app.parity_ui(ctx,Instant::now());geo=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Vec2)>(egui::Id::new("v034-error-geometry")));});}
            let (rect,offset)=geo.unwrap();assert!(rect.min.x>=0. && rect.min.y>=0. && rect.max.x<=size.x && rect.max.y<=size.y);assert_eq!(offset.y,0.);
            let mut routed=None;
            for _ in 0..3 {let _=ctx.run(input(vec![egui::Event::PointerMoved(rect.center()),egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:egui::vec2(0.,-120.),modifiers:Default::default()}]),|ctx|{app.parity_ui(ctx,Instant::now());routed=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Vec2)>(egui::Id::new("v034-error-geometry")));});}
            assert!(routed.unwrap().1.y>0.,"wheel routing: initial={:?}, final={:?}, pointer={:?}, delta={:?}, layer={:?}",geo,routed,ctx.input(|i|i.pointer.hover_pos()),ctx.input(|i|i.smooth_scroll_delta),ctx.layer_id_at(rect.center()));
            let before=routed.unwrap().1.y;let mut outside=None;
            let out=ctx.run(input(vec![egui::Event::PointerMoved(egui::pos2(size.x-10.,size.y-10.)),egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:egui::vec2(0.,-120.),modifiers:Default::default()}]),|ctx|{app.parity_ui(ctx,Instant::now());outside=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Vec2)>(egui::Id::new("v034-error-geometry")));});
            assert_eq!(outside.unwrap().1.y,before,"outside wheel must not move error details");
            assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.text().contains("Error details"))));
            let check_summary=|out:&egui::FullOutput| {
                let (rect,clip)=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Rect)>(egui::Id::new("v035-summary-geometry"))).unwrap();
                assert_eq!(ctx.layer_id_at(rect.center()).unwrap().order,egui::Order::Background,"summary must not be covered by settings overlay");
                assert!(clip.contains_rect(rect));assert!(egui::Rect::from_min_size(egui::Pos2::ZERO,size).contains_rect(rect));
                let shape=out.shapes.iter().find_map(|s|match &s.shape {egui::epaint::Shape::Text(t) if t.galley.text().starts_with(&format!("{source}: Useful existing error"))=>Some((s.clip_rect,t)),_=>None}).expect("useful cause text, not heading");
                let text_rect=egui::Rect::from_min_size(shape.1.pos,shape.1.galley.size());
                assert!(shape.0.contains_rect(text_rect),"summary painted text must be visible: {text_rect:?} clip {:?}",shape.0);
                assert!(rect.contains_rect(text_rect));assert!(text_rect.height()<30.);
            };
            check_summary(&out);
            let retained=outside.unwrap().1.y;
            for (next_picker,next_tab) in [(true,0),(false,0),(false,4),(picker,tab)] {
                app.picker=next_picker;app.tab=next_tab;
                let out=ctx.run(input(vec![]),|ctx|app.parity_ui(ctx,Instant::now()));check_summary(&out);
                let (_,offset)=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Vec2)>(egui::Id::new("v034-error-geometry"))).unwrap();assert_eq!(offset.y,retained,"retained scroll across picker/live/Advanced");
            }
            let (summary_rect,summary_clip)=ctx.data(|d|d.get_temp::<(egui::Rect,egui::Rect)>(egui::Id::new("v035-summary-geometry"))).unwrap();
            records.push(serde_json::json!({"summary_rect":[summary_rect.min.x,summary_rect.min.y,summary_rect.max.x,summary_rect.max.y],"summary_clip":[summary_clip.min.x,summary_clip.min.y,summary_clip.max.x,summary_clip.max.y],"summary_occlusion_checked":true,"retained_navigation_checked":true,"viewport":[size.x,size.y],"picker":picker,"tab":tab,"error_rect":[rect.min.x,rect.min.y,rect.max.x,rect.max.y],"inside_wheel_offset":before,"outside_wheel_offset":outside.unwrap().1.y,"physical":false,"input":"egui native PointerMoved and MouseWheel events"}));
            assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.text().contains("FAILURE RECORD NOT PERSISTED"))));
            app.worker.close();std::fs::remove_dir_all(root).unwrap();
        }}
        }
        if let Some(path)=std::env::var_os("V038_GEOMETRY") {std::fs::write(path,serde_json::to_vec_pretty(&records).unwrap()).unwrap();}
    }
    #[test]
    fn v035_worker_panic_failed_persistence_panel() {
        struct Panics;impl Factory for Panics{fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{panic!("pre-owner open generation")}}
        let root=std::env::temp_dir().join(format!("v035-panel-{}",std::process::id()));std::fs::write(&root,b"blocked").unwrap();
        // Recorder path override is bounded test injection; no platform/device factory.
        let worker=Worker::with_factory_recorder(Panics,camera_lab::failure_record::Recorder::at(root.clone()));
        let ctx=egui::Context::default();let mut app=App::with_exporter_worker(&ctx,|_,_|Ok(()),worker);
        app.worker.request(Some(Request{source:Source::Simulation,settings:camera_lab::controls::Settings::default(),startup_intent:camera_lab::startup::Intent::Normal})).unwrap();
        let end=Instant::now()+Duration::from_secs(5);while !app.worker.closed(){assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(2));}
        app.session=app.worker.snapshot();assert!(app.session.first_failure.is_some());
        for _ in 0..3 {let _=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(640.,480.))),..Default::default()},|ctx|{app.failure_visibility(ctx);});}
        let out=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(640.,480.))),..Default::default()},|ctx|{assert!(app.failure_visibility(ctx).is_some());});
        for expected in ["First cause: Unknown / Unknown / Unknown", "FAILURE RECORD NOT PERSISTED"] {assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.text().starts_with(expected))));}
        std::fs::remove_file(root).unwrap();
    }
    #[test]
    fn picker_modes_gates_and_headless_geometry_v033() {
        let ctx=egui::Context::default();let mut app=App::with_exporter_worker(&ctx,|_,_|Ok(()),Worker::with_factory(Fake(Arc::new(AtomicBool::new(true)))));app.fake_only=true;
        assert!(!app.picker_simulation);assert!(!app.picker_connect_ready());app.connect_picker();assert!(app.source.is_none());
        app.picker_simulation=true;assert!(app.picker_connect_ready());assert!(app.source.is_none());
        for gate in 0..7 {match gate {0=>app.busy=true,1=>app.closing=true,2=>app.connection_fault=true,3=>app.session.active=true,4=>app.session.restart_disabled=true,5=>app.session.state=camera_lab::session::State::Error,_=>app.session.requested_revision=Some(99)};assert!(!app.picker_connect_ready());app.connect_picker();assert!(app.source.is_none());app.busy=false;app.closing=false;app.connection_fault=false;app.session.active=false;app.session.restart_disabled=false;app.session.state=camera_lab::session::State::Disconnected;app.session.requested_revision=None;}
        app.picker_simulation=false;
        assert_eq!(App::refreshed_selection(Some("port-B"),["port-C","port-B","port-A"].into_iter()),Some(1));
        assert_eq!(App::refreshed_selection(Some("port-A"),["port-C","port-B"].into_iter()),None);
        assert_eq!(App::refreshed_selection(None,["port-C","port-B"].into_iter()),None);
        app.show_frame(&ctx,camera_lab::CameraAdapter::next_frame(&mut camera_lab::Simulation{sequence:1}).unwrap());
        let mut records=Vec::new();
        for (picker,tab,size) in [(true,0,egui::vec2(640.,480.)),(false,2,egui::vec2(1100.,760.)),(false,1,egui::vec2(1100.,760.))] {
            app.picker=picker;app.tab=tab;
            // Warm layout before recording final egui text geometry.
            let input=||egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,size)),..Default::default()};
            let _=ctx.run(input(),|ctx|app.render(ctx));let out=ctx.run(input(),|ctx|app.render(ctx));
            let texts:Vec<_>=out.shapes.iter().filter_map(|s|if let egui::epaint::Shape::Text(t)=&s.shape {Some(serde_json::json!({"text":t.galley.text(),"x":t.pos.x,"y":t.pos.y,"width":t.galley.size().x,"height":t.galley.size().y}))}else{None}).collect();
            if !picker && tab==2 {assert!(texts.iter().any(|t|t["text"]=="Brightness ×"));assert!(texts.iter().any(|t|t["text"]=="1.20"));assert!(!texts.iter().any(|t|t["text"]=="Brightness EV"));assert!((app.recipe.brightness()-1.2).abs()<1e-6);}
            records.push(serde_json::json!({"picker":picker,"tab":tab,"logical_dimensions":[size.x,size.y],"shape_count":out.shapes.len(),"texts":texts,"physical":false}));
        }
        if let Some(path)=std::env::var_os("V033_GEOMETRY") {std::fs::write(path,serde_json::to_vec_pretty(&records).unwrap()).unwrap();}
        app.picker=true;app.picker_simulation=true;app.connect_picker();assert!(matches!(app.source,Some(Source::Simulation)));assert!(!app.picker);
        app.worker.close();
    }
    #[test]
    fn live_tabs_accept_normal_pointer_press_and_release() {
        let ctx=egui::Context::default();let mut app=App::without_scan(&ctx);
        app.fake_only=true;app.picker=false;app.viewport_picker=false;
        let screen=egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1100.,760.));
        for _ in 0..3 {let _=ctx.run(egui::RawInput{screen_rect:Some(screen),..Default::default()},|ctx|app.render(ctx));}
        for tab in 0..5 {
            let point=egui::pos2(53.+98.*tab as f32,16.);
            for pressed in [true,false] {
                let _=ctx.run(egui::RawInput{screen_rect:Some(screen),events:vec![
                    egui::Event::PointerMoved(point),egui::Event::PointerButton{
                        pos:point,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE}],
                    ..Default::default()},|ctx|app.render(ctx));
            }
            assert_eq!(app.tab,tab,"tab {tab} did not accept the pointer click");
        }
        app.worker.close();
    }

    #[test]
    fn live_resize_survives_missing_and_picker_sized_transition_reports() {
        let ctx=egui::Context::default();
        let mut app=App::without_scan(&ctx);app.fake_only=true;
        app.picker=false;app.viewport_picker=false;
        let mut input=egui::RawInput::default();
        input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().inner_rect=Some(
            egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1270.,830.)));
        let out=ctx.run(input,|ctx|app.sync_stage(ctx));
        assert!(sizes(&out).is_empty());
        assert_eq!(app.live_size,egui::vec2(1270.,830.));
        app.picker=true;
        let mut input=egui::RawInput::default();
        input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().inner_rect=Some(
            egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(640.,480.)));
        let out=ctx.run(input,|ctx|app.sync_stage(ctx));
        assert_eq!(sizes(&out),vec![egui::vec2(640.,480.)]);
        assert_eq!(app.live_size,egui::vec2(1270.,830.));
        app.picker=false;
        let out=ctx.run(egui::RawInput::default(),|ctx|app.sync_stage(ctx));
        assert_eq!(sizes(&out),vec![egui::vec2(1270.,830.)]);
        assert_eq!(app.live_size,egui::vec2(1270.,830.));
        app.worker.close();
    }

    #[test]
    fn stage_edges_cleanup_gate_and_initial_close_failure() {
        let ctx=egui::Context::default();let gate=Arc::new(AtomicBool::new(false));
        let mut app=App::with_exporter_worker(&ctx,|_,_|Ok(()),Worker::with_factory(Fake(gate.clone())));app.fake_only=true;
        for size in [egui::vec2(520.,360.),egui::vec2(640.,480.)] {
            let output=ctx.run(egui::RawInput{system_theme:Some(egui::Theme::Light),screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,size)),..Default::default()},|ctx|app.render(ctx));
            assert_eq!(ctx.theme(),egui::Theme::Dark);
            let labels:Vec<_>=output.shapes.iter().filter_map(|s|if let egui::epaint::Shape::Text(t)=&s.shape {Some(t.galley.text())} else {None}).collect();
            for label in ["Select a camera","Connect and open capture window","Refresh","Simulation / demo","No eligible camera found"] {assert!(labels.contains(&label),"missing {label}");}
            assert!(!labels.contains(&"Choose an explicit source"));assert!(app.source.is_none());
        }
        assert!(app.picker && app.can_connect());assert!(sizes(&draw(&mut app,&ctx)).is_empty());
        app.request(Some(Source::Simulation));assert!(!app.picker);assert_eq!(app.primary_state(),"Connecting");
        assert_eq!(sizes(&draw(&mut app,&ctx)),vec![egui::vec2(1100.,760.)]);
        assert!(sizes(&draw(&mut app,&ctx)).is_empty());
        until(&mut app,&ctx,|a|a.displayed_revision==Some(a.revision));assert_eq!(app.primary_state(),"Live");assert!(app.status.is_empty());
        let mut input=egui::RawInput::default();input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().inner_rect=Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1280.,800.)));
        app.change_camera();let out=ctx.run(input,|ctx|app.render(ctx));assert_eq!(sizes(&out),vec![egui::vec2(640.,480.)]);
        assert!(!app.can_connect());assert_eq!(app.primary_state(),"Stopping");
        assert!(sizes(&draw(&mut app,&ctx)).is_empty());gate.store(true,Ordering::SeqCst);
        until(&mut app,&ctx,|a|a.can_connect());assert_eq!(app.live_size,egui::vec2(1280.,800.));
        app.session.restart_disabled=true;assert!(!app.can_connect());app.session.restart_disabled=false;
        app.session.state=camera_lab::session::State::Error;assert!(!app.can_connect());
        app.session.state=camera_lab::session::State::Disconnected;
        app.request(Some(Source::Simulation));let out=draw(&mut app,&ctx);assert_eq!(sizes(&out),vec![egui::vec2(1280.,800.)]);assert!(sizes(&draw(&mut app,&ctx)).is_empty());
        app.worker.close();
        struct Fails;impl Factory for Fails {fn open(&mut self,_:&Request)->Result<Box<dyn Transport>,String>{Err("Could not connect. Check the camera connection.".into())}}
        let mut app=App::with_exporter_worker(&ctx,|_,_|Ok(()),Worker::with_factory(Fails));app.fake_only=true;
        app.request(Some(Source::Simulation));until(&mut app,&ctx,|a|a.session.state==camera_lab::session::State::Error);
        assert_eq!(app.primary_state(),"Disconnected");assert!(app.concise_outcome().1);app.change_camera();draw(&mut app,&ctx);assert!(!app.can_connect());app.worker.close();
        let mut app=App::without_scan(&ctx);app.fake_only=true;app.fake_until=Some(Instant::now());draw(&mut app,&ctx);until(&mut app,&ctx,|a|a.worker.closed());assert!(app.closing);assert_eq!(app.revision,0);
    }
    #[test]
    fn independent_outcomes_and_persistent_fault() {
        let ctx=egui::Context::default();let mut app=App::without_scan(&ctx);app.fake_only=true;
        app.export_full=true;
        app.save_result="Saved image and RAW/metadata sidecars: sample.png".into();
        app.product.restore_result="Waiting for a new matching original Live frame".into();assert!(!app.concise_outcome().0.contains("Live restored"));
        app.product.restore_result="Original applied Live settings restored; new matching frame verified".into();assert_eq!(app.concise_outcome().0,"Full image saved · Live restored");
        app.product.restore_result="Return to Live unverified: failure".into();assert_eq!(app.concise_outcome().0,"Full image saved · return to Live unconfirmed");
        app.outcome_fault=Some("Save failed: primary image saved; sidecars INCOMPLETE".into());assert!(app.concise_outcome().0.contains("Save failed"));
        app.product.restore_result.clear();assert!(app.concise_outcome().0.starts_with("INCOMPLETE"));
        app.terminal_preview_failure=Some("terminal render fault".into());app.settings.exposure_rows=101;app.full_selection(None);assert!(app.concise_outcome().0.starts_with("Preview unavailable"));
        app.terminal_preview_failure=None;app.save_result="Saved image and RAW/metadata sidecars: later.png".into();assert!(app.concise_outcome().0.starts_with("INCOMPLETE"));
        app.source=Some(Source::Simulation);app.revision=8;app.matching_revision=Some(8);app.displayed_revision=Some(8);app.session.requested_revision=Some(8);
        app.session.state=camera_lab::session::State::Streaming;assert_eq!(app.primary_state(),"Live");
        app.session.state=camera_lab::session::State::Disconnected;assert_eq!(app.primary_state(),"Disconnected");
        app.matching_revision=None;app.displayed_revision=None;assert_eq!(app.primary_state(),"Disconnected");
        app.worker.close();
    }
    fn verify(path:&std::path::Path,frame:&Frame,recipe:Recipe) {
        assert_eq!(image::open(path).unwrap().to_rgb8().into_raw(),recipe.render(frame,None).unwrap().2);
        assert_eq!(std::fs::read(format!("{}.raw",path.display())).unwrap(),*frame.raw);
        let meta:serde_json::Value=serde_json::from_slice(&std::fs::read(format!("{}.json",path.display())).unwrap()).unwrap();
        assert_eq!(meta["frame"],serde_json::to_value(&frame.metadata).unwrap());assert_eq!(meta["display_recipe"],serde_json::to_value(recipe).unwrap());
    }
    // Read the actual footer output; querying Live again after render would miss
    // the old entry-time -> upload-time -> earlier-footer-time regression.
    fn preview_footer(out:&egui::FullOutput)->f64 {
        let text=out.shapes.iter().find_map(|s|match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text().contains(" · Preview ")=>Some(t.galley.text()),
            _=>None,
        }).expect("production parity footer");
        text.split(" · Preview ").nth(1).unwrap().split(" FPS").next().unwrap()
            .parse().expect("active Preview must be numeric, not unavailable")
    }
    fn active_preview(app:&mut App,ctx:&egui::Context) {
        let start=Instant::now();let initial=app.measurement.unique_total;
        let mut observations=0;
        while start.elapsed()<Duration::from_millis(2400) {
            let out=draw(app,ctx);
            assert!(preview_footer(&out)>0.,"active unique Preview must stay nonzero");
            observations+=1;
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(observations>2 && app.measurement.unique_total>initial+2);
    }
    // Same production actions and assertions run on host and the single paired Windows FAKE job.
    #[test]
    fn coherent_two_stage_flow() {
        let ctx=egui::Context::default();
        #[cfg(not(windows))]
        let (worker,source,root)=(Worker::with_factory(Fake(Arc::new(AtomicBool::new(true)))),Source::Simulation,std::env::temp_dir().join(format!("measurement-v026-{}",std::process::id())));
        #[cfg(windows)]
        let (worker,source,root)={let root=PathBuf::from(std::env::var_os("INTEGRATED_UX_FAKE_ROOT").expect("explicit private fake root"));(Worker::new(),Source::Fixture{root:root.clone(),fault:camera_lab::continuous_session::Fixture::TypedFrames},root)};
        std::fs::create_dir(&root).unwrap();
        let mut guardian=worker.guardian();let mut app=App::with_recipe_exporter_worker(&ctx,camera_lab::image_export::save_recipe,worker);app.fake_only=true;
        assert!(app.picker && app.can_connect());draw(&mut app,&ctx);assert!(app.source.is_none());
        app.request(Some(source));assert!(!app.picker);draw(&mut app,&ctx);
        until(&mut app,&ctx,|a|a.product.applied.is_some()&&a.displayed_revision==Some(a.revision));assert_eq!(app.primary_state(),"Live");assert!(app.status.is_empty());
        active_preview(&mut app,&ctx);
        app.paused=true;let held=app.frame.clone().unwrap();let _recipe=app.displayed_recipe;let submits=app.gui_submits;let unique=app.measurement.unique_total;
        assert!(held.metadata.measurement.as_ref().is_some_and(|o|o.valid_for(&held.metadata)));
        assert!(held.metadata.simulated);assert!(camera_lab::measurement::timestamp_semantics(&held.metadata).starts_with("SIMULATED"));
        until(&mut app,&ctx,|a|a.product.applied.unwrap().sequence>held.metadata.sequence+3);assert_eq!(app.gui_submits,submits);assert_eq!(app.primary_state(),"Paused (acquisition continues)");
        app.recipe.ev=0.5;app.recipe_changed();until(&mut app,&ctx,|a|a.displayed_recipe==a.recipe);assert_eq!(app.measurement.unique_total,unique);
        let end=Instant::now()+Duration::from_millis(2100);
        while Instant::now()<end {draw(&mut app,&ctx);std::thread::sleep(Duration::from_millis(10));}
        assert_eq!(preview_footer(&draw(&mut app,&ctx)),0.);
        assert_eq!(app.measurement.unique_total,unique,"held recipe and repaints cannot inflate Preview");
        app.paused=false;
        until(&mut app,&ctx,|a|a.measurement.unique_total>unique);
        active_preview(&mut app,&ctx);
        app.paused=true;
        let recipe=app.displayed_recipe;
        let path=root.join("visible.png");app.save_frozen(held.clone(),path.clone());until(&mut app,&ctx,|a|!a.busy);verify(&path,&held,recipe);
        app.paused=false;app.settings.exposure_rows=101;let rev=app.revision;draw(&mut app,&ctx);assert_eq!(app.revision,rev);assert_ne!(app.product.applied.unwrap().settings,app.settings);
        app.request(app.source.clone());until(&mut app,&ctx,|a|a.product.applied.is_some()&&a.displayed_revision==Some(a.revision));assert!(app.save_result.is_empty());
        let original=app.product.applied.unwrap();app.settings.gain_eighths=12;
        let path=root.join("full.tiff");app.begin_full(path.clone());assert!(app.product.busy());assert!(!app.concise_outcome().0.contains("Live restored"));
        until(&mut app,&ctx,|a|!a.product.busy()&&!a.busy&&a.displayed_revision==Some(a.revision));
        verify(&path,app.product.held.as_ref().unwrap(),app.full_recipe);
        let full=app.product.held.as_ref().unwrap();assert!(full.metadata.measurement.as_ref().is_some_and(|o|o.valid_for(&full.metadata)));assert_ne!(full.metadata.acquisition.as_ref().unwrap().session,held.metadata.acquisition.as_ref().unwrap().session);assert_eq!(app.product.applied.unwrap().settings,original.settings);assert_eq!(app.settings.gain_eighths,12);assert_eq!(app.concise_outcome().0,"Full image saved · Live restored");
        let end=Instant::now()+Duration::from_millis(500);while Instant::now()<end {draw(&mut app,&ctx);std::thread::sleep(Duration::from_millis(10));}
        app.request(None);until(&mut app,&ctx,|a|!a.session.active);assert_eq!(app.primary_state(),"Disconnected");
        let output=draw(&mut app,&ctx);assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.text().contains("retained image (historical)"))));
        app.fake_until=Some(Instant::now());draw(&mut app,&ctx);until(&mut app,&ctx,|a|a.worker.closed());let final_state=guardian.wait();assert!(!final_state.active && !final_state.restart_disabled);
        #[cfg(not(windows))] std::fs::remove_dir_all(root).unwrap();
        println!("v026 timing/source capture + unique preview: chooser → Live → PauseSave → ResumeApply → FullSaveRestore → StopClose; exact frozen pixels/RAW/recipe; cleanup collected");
    }
}
