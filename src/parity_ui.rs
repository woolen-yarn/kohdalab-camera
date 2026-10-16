use super::*;

pub fn aspect_fit(area:egui::Rect,size:egui::Vec2)->egui::Rect {
    let scale=(area.width()/size.x).min(area.height()/size.y).max(0.);
    egui::Rect::from_center_size(area.center(),size*scale)
}
pub const TABS:[&str;5]=["Camera","Acquisition","Image","Export","Advanced"];
pub fn force_dark(ctx:&egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut v=egui::Visuals::dark();
    v.panel_fill=egui::Color32::from_rgb(17,19,24);
    v.window_fill=egui::Color32::from_rgb(21,24,30);
    v.selection.bg_fill=egui::Color32::from_rgb(25,130,145);
    ctx.set_visuals_of(egui::Theme::Dark,v);
}
impl App {
    fn estimate_scene(&mut self,wb:bool) {
        if self.estimate_rx.is_some() {return;}
        let Some(f)=self.frame.clone() else {self.status="No held valid frame for scene estimate".into();return;};
        let recipe=self.recipe;self.estimate_base=Some(recipe);let (tx,rx)=mpsc::sync_channel(1);self.estimate_rx=Some(rx);
        std::thread::spawn(move || {let _=tx.send(recipe.estimate(&f,wb));});
    }
    pub(super) fn recipe_changed(&mut self) {
        if let Some(p)=&self.preview {
            p.set_recipe(self.recipe);
            if self.paused || self.source.is_none() {
                if let Some(f)=self.frame.clone() {p.submit_held(f);}
                return;
            }
            if let (Some(g),Some(f))=(self.preview_generation,self.frame.clone()) {
                p.submit(camera_lab::preview_pipeline::Key{generation:g,sequence:f.metadata.sequence},f);
            }
        }
    }
    pub(super) fn parity_ui(&mut self,ctx:&egui::Context,now:std::time::Instant) {
        if let Some(r)=self.estimate_rx.as_ref().and_then(|r| match r.try_recv() {Ok(v)=>Some(v),Err(mpsc::TryRecvError::Disconnected)=>Some(Err("Scene estimate worker disconnected".into())),_=>None}) {
            self.estimate_rx=None;
            if self.estimate_base.take()!=Some(self.recipe) {self.status="Scene estimate discarded because display controls changed".into();}
            else {match r {Ok(recipe)=>{self.recipe=recipe;self.recipe_changed();self.status="Scene estimate applied (software only; not calibration)".into();},Err(e)=>self.status=e}}
        }
        force_dark(ctx);
        self.sync_stage(ctx);
        self.failure_visibility(ctx);
        if self.picker {
            self.selector_ui(ctx);
            return;
        }
        ctx.input_mut(|input| {
            for (i,key) in [egui::Key::F1,egui::Key::F2,egui::Key::F3,egui::Key::F4,egui::Key::F5].into_iter().enumerate() {
                if input.key_pressed(key) {
                    input.consume_key(egui::Modifiers::NONE,key);
                    self.tab=i;
                }
            }
        });
        let toolbar=egui::TopBottomPanel::top("tabs").show(ctx,|ui| {
            ui.horizontal(|ui| {
                for (i,label) in TABS.iter().enumerate() {
                    let response=ui.add_sized([90.,30.],egui::Button::new(*label).selected(self.tab==i)).on_hover_text(format!("{} · F{}",label,i+1));
                    #[cfg(feature = "input-diagnostics")]
                    input_diagnostics::button(ctx,label,&response);
                    if response.clicked() {self.tab=i;}
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {ui.weak("KohdaLab Camera");});
            });
        }).response.rect;
        egui::TopBottomPanel::bottom("compact-status").exact_height(42.).show(ctx,|ui| {
            ui.spacing_mut().item_spacing.y=2.;
            ui.horizontal(|ui| {
                let dimensions=self.frame.as_ref().map(|f|format!("{} × {}",f.metadata.width,f.metadata.height)).unwrap_or_else(||"— × —".into());
                let state=self.primary_state();
                let simulated=self.frame.as_ref().is_some_and(|f|f.metadata.simulated);
                let retained=self.primary_state()=="Disconnected" && self.frame.is_some();
                let context=match (simulated,retained) {
                    (true,true)=>" · SIMULATION · retained image (historical)",
                    (true,false)=>" · SIMULATION",
                    (false,true)=>" · retained image (historical)",
                    (false,false)=>"",
                };
                let (capture,age,sim)=self.measurement.capture(now);
                let (preview,preview_age)=self.measurement.preview(now);
                #[cfg(test)] ctx.data_mut(|d|d.insert_temp(egui::Id::new("footer-preview-rate"),preview));
                let rate=|v:Option<f64>|v.map(|v|format!("{v:.1}")).unwrap_or_else(||"—".into());
                let age_text=|v:Option<Duration>|v.map(|d|format!(" · age {:.1}s",d.as_secs_f64())).unwrap_or_default();
                ui.add(egui::Label::new(format!("{dimensions} · {state}{context} · {}Capture {} FPS",if sim {"SIM "} else {""},rate(capture))).truncate()).on_hover_text(format!("Capture: complete frames accepted at the source{}; unavailable when stale. Preview: {} FPS{}, excluding held adjustments and repaints. Neither measures physical display refresh.",age_text(age),rate(preview),age_text(preview_age)));
            });
            ui.horizontal(|ui| {
                let width=ui.available_width();
                ui.add_sized([320_f32.min(width/2.),ui.text_style_height(&egui::TextStyle::Body)],egui::Label::new(&self.cursor).truncate()).on_hover_text(&self.cursor);
                if !self.cursor.is_empty() {ui.label("·");}
                let (result,failed)=self.concise_outcome();
                let text=egui::RichText::new(&result);let text=if failed {text.color(egui::Color32::LIGHT_RED)} else {text};
                ui.add(egui::Label::new(text).truncate()).on_hover_text(self.outcome_detail());
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(egui::Color32::BLACK)).show(ctx,|ui| {
            self.viewport_rect=ui.max_rect();self.cursor.clear();
            if (self.source.is_none() || (self.matching_revision==Some(self.revision) && self.displayed_revision==Some(self.revision))) && let (Some(t),Some(f),Some(pixels))=(&self.texture,&self.frame,&self.display_pixels) {
                let rect=aspect_fit(ui.max_rect(),t.size_vec2());
                ui.put(rect,egui::Image::new(t).fit_to_exact_size(rect.size()));
                if let Some(pos)=ctx.pointer_hover_pos().filter(|p|rect.contains(*p)) {
                    let x=(((pos.x-rect.left())/rect.width()*pixels.size[0] as f32).floor() as usize).min(pixels.size[0]-1);
                    let y=(((pos.y-rect.top())/rect.height()*pixels.size[1] as f32).floor() as usize).min(pixels.size[1]-1);
                    let (w,h)=self.displayed_recipe.dimensions(f.metadata.width,f.metadata.height);
                    let stride=w.max(h).div_ceil(1024);
                    let (sx,sy)=self.displayed_recipe.source(x as u32*stride,y as u32*stride,f.metadata.width,f.metadata.height);
                    let c=self.displayed_recipe.pixel(if self.displayed_recipe.version==1 {f.legacy_rgb(sx,sy)}else{f.rgb(sx,sy)}.unwrap_or([0;3]));
                    self.cursor=format!("Cursor ({sx}, {sy}) · RGB ({}, {}, {})",c[0],c[1],c[2]);
                }
            } else {ui.centered_and_justified(|ui| {ui.label(self.primary_state());});}
        });
        if self.tab<5 && !self.picker {
            let before=self.recipe;let mut open=true;
            egui::Window::new(TABS[self.tab]).id(egui::Id::new("settings-overlay"))
                .open(&mut open).fixed_pos(egui::pos2(toolbar.left()+8.,toolbar.bottom()+8.))
                .default_width(340.).max_height((ctx.screen_rect().height()-toolbar.height()-80.).max(100.))
                .constrain_to(if self.has_failure_panel() {egui::Rect::from_min_max(egui::pos2(0.,toolbar.bottom()+4.),egui::pos2(ctx.screen_rect().right(),ctx.screen_rect().bottom()-42.))} else {ctx.screen_rect()})
                .resizable(false).collapsible(false)
                .show(ctx,|ui| {
                    ui.set_width(340.);
                    egui::ScrollArea::vertical().max_height((ctx.screen_rect().height()-toolbar.height()-110.).max(80.)).show(ui,|ui| {
                        ui.set_width(340.);
                        match self.tab {0=>self.camera_settings(ui),1=>self.acquisition_settings(ui),2=>self.image_settings(ui),3=>self.export_settings(ui),_=>self.advanced_settings(ui,now)}
                    });
                });
            if !open {self.tab=usize::MAX;}
            if before!=self.recipe {self.recipe_changed();}
        }
    }
    fn camera_settings(&mut self,ui:&mut egui::Ui) {
        ui.heading("Device");
        let device=match &self.source {Some(Source::Physical(d))=>d.product_string().unwrap_or("Supported camera"),Some(_)=>"SIMULATION",None=>"Disconnected"};
        ui.add(egui::Label::new(device).truncate()).on_hover_text(device);
        ui.horizontal(|ui| {
            if self.source.is_some() && self.primary_state()!="Disconnected" {if ui.add_enabled(!self.closing,egui::Button::new("Disconnect")).clicked() {self.request(None);}}
            else if ui.add_enabled(!self.closing,egui::Button::new("Reconnect")).clicked() {self.change_camera();}
            if ui.add_enabled(!self.closing,egui::Button::new("Change camera")).clicked() {self.change_camera();}
        });
        if ui.add_enabled(!self.closing && !self.connection_fault && self.session.state==camera_lab::session::State::Streaming && self.source.is_some() && self.matching_revision==Some(self.revision) && !self.product.busy(),egui::Button::new(if self.paused {"Resume"} else {"Pause"})).on_hover_text("Pause freezes the displayed image. Camera acquisition continues.").clicked() {self.paused=!self.paused;}
    }
    fn acquisition_settings(&mut self,ui:&mut egui::Ui) {
        ui.add_enabled_ui(!self.closing,|ui| {
            let mut full=self.settings.mode==Mode::Full;
            if ui.checkbox(&mut full,"Full sensor live mode · 3.1 MP").changed() {self.settings.mode=if full {Mode::Full} else {Mode::FullFieldSkip};}
            ui.small(if full {"Full mode prioritizes pixels; use balanced Live mode for highest preview rate."} else {"Balanced Live mode prioritizes responsive preview."});
            ui.spacing_mut().slider_width=(ui.available_width()-60.).max(80.);
            ui.label("Exposure · sensor rows");ui.add(egui::Slider::new(&mut self.settings.exposure_rows,1..=self.settings.mode.profile().exposure_max));
            ui.label("Analog gain ×");let mut gain=f64::from(self.settings.gain_eighths)/8.;
            if ui.add(egui::Slider::new(&mut gain,1.0..=4.0).step_by(0.125)).changed() {self.settings.gain_eighths=(gain*8.).round() as u16;}
            if ui.add_enabled(!self.connection_fault&&self.source.is_some()&&self.matching_revision==Some(self.revision)&&!self.product.busy()&&self.settings.validate().is_ok(),egui::Button::new("Apply camera settings")).clicked() {self.request(self.source.clone());}
            let pending=self.product.applied.is_none_or(|a|a.settings!=self.settings || a.intent!=self.intent);
            ui.small(if pending {"Changes pending"} else {"Applied"});
        });
    }
    fn image_settings(&mut self,ui:&mut egui::Ui) {
        ui.spacing_mut().slider_width=(ui.available_width()-60.).max(80.);
        ui.label("Brightness ×");let mut brightness=self.recipe.brightness();
        if ui.add(egui::Slider::new(&mut brightness,0.0625..=16.).logarithmic(true).fixed_decimals(2)).changed() {self.recipe.set_brightness(brightness);}
        if ui.add_enabled(self.estimate_rx.is_none(),egui::Button::new("Auto brightness")).on_hover_text("Bounded software held-scene estimate; not calibration.").clicked() {self.estimate_scene(false);}
        ui.checkbox(&mut self.recipe.monochrome,"Monochrome");
        ui.label("Gamma");ui.add(egui::Slider::new(&mut self.recipe.gamma,0.25..=4.));
        ui.checkbox(&mut self.recipe.mirror,"Mirror");ui.checkbox(&mut self.recipe.flip,"Flip");
        egui::ComboBox::from_label("Rotation").selected_text(format!("{}°",u16::from(self.recipe.quarter_turns)*90)).show_ui(ui,|ui| {for i in 0..4 {ui.selectable_value(&mut self.recipe.quarter_turns,i,format!("{}°",u16::from(i)*90));}});
        if ui.add_enabled(self.estimate_rx.is_none(),egui::Button::new("Set white balance")).on_hover_text("Bounded software held-scene estimate; not calibration.").clicked() {self.estimate_scene(true);}
        if ui.button("Reset white balance").clicked() {self.recipe.wb=[1.;3];}
        ui.collapsing("Software display presets",|ui| {
            if ui.button("Reset display · 1.20×").clicked() {self.recipe=Recipe::default();}
            if ui.button("Neutral · 1.00× / gamma 1 / 0°").clicked() {self.recipe=Recipe::neutral();}
            if ui.button("Reference appearance").on_hover_text("Software +1.5 EV / gamma 2.2 / 180°").clicked() {self.recipe=Recipe::reference();}
            ui.small("Software presets; not sensor calibration.");
        });
    }
    fn export_settings(&mut self,ui:&mut egui::Ui) {
        if ui.add_enabled(self.frame.is_some()&&(self.source.is_none()&&!self.session.active || self.matching_revision==Some(self.revision)&&self.displayed_revision==Some(self.revision))&&!self.busy&&!self.closing&&!self.product.busy(),egui::Button::new("Capture image…")).clicked() {self.capture();}
        if ui.add_enabled(self.product.applied.is_some()&&!self.busy&&!self.closing&&!self.product.busy(),egui::Button::new("Capture full sensor image… · 3.1 MP")).on_hover_text("Captures a new Full frame, then automatically returns to the applied Live settings.").clicked() {self.capture_full();}
        ui.weak("Save as PNG or TIFF.").on_hover_text("The selected frame is held while the save dialog is open. Original sensor TIFF (Bayer sources), RAW and metadata are saved alongside it. Existing files are never overwritten; incomplete saves are reported.");
        if self.busy || self.product.busy() {if ui.button("Stop / Disconnect").clicked() {self.request(None);}}
    }
    fn advanced_settings(&mut self,ui:&mut egui::Ui,now:std::time::Instant) {
        ui.label(concat!("KohdaLab Camera · ",env!("CARGO_PKG_VERSION")));
        for (i,c) in ["Red WB","Green WB","Blue WB"].iter().enumerate() {ui.add(egui::Slider::new(&mut self.recipe.wb[i],0.25..=4.).text(*c));}
        if ui.button("Neutral display").clicked() {self.recipe=Recipe::neutral();}
        if ui.button("Reference appearance · +1.5 EV / gamma 2.2 / 180°").clicked() {self.recipe=Recipe::reference();}
        if ui.button("Reset display").clicked() {self.recipe=Recipe::default();}
        if let Some(f)=&self.frame {
            ui.label(camera_lab::measurement::timestamp_semantics(&f.metadata));
        }
        ui.collapsing("Diagnostics",|ui| {
        if let Some(t)=&self.frame_received {ui.label(format!("Texture update age: {:.1} s",t.age_at(now).as_secs_f64()));}
        if let Some(f)=&self.frame {
            if let Some(m)=&f.metadata.measurement {ui.label(format!("UTC Unix ms {:?}; source accepted total {}; sequence {}; local source sample {} ns; rolling {}/{} ns",m.host_accepted_completion_unix_ms,m.accepted_total,m.sequence,m.source_sample_ns,m.window_frames,m.window_ns));}
        }
        if let Some(d)=self.selected.and_then(|i|self.devices.get(i)) {ui.label(format!("{:04x}:{:04x} serial={:?} bus={} ports={:?}",d.vendor_id(),d.product_id(),d.serial_number(),d.bus_id(),d.port_chain()));}

        ui.small("Color and optical calibration are not available.");
        ui.horizontal(|ui| {ui.selectable_value(&mut self.intent,camera_lab::startup::Intent::Normal,"Normal startup");ui.selectable_value(&mut self.intent,camera_lab::startup::Intent::Diagnostics,"Long Diagnostics");});
        ui.label("Diagnostics intentionally runs the unchanged three-pattern checks. Select then Apply acquisition settings.");
        ui.label(format!("{:?}: {}",self.session.state,self.session.message));ui.label(&self.status);ui.label(self.outcome_detail());
        if let Some(p)=&self.preview {ui.label(format!("Bounded pipeline: {:?}",p.counters()));}
        ui.label(format!("Unique preview camera frames this request: {}",self.measurement.unique_total));
        ui.label(format!("All GUI texture submits (includes recipe refresh): {} (software only; not physical display FPS)",self.gui_submits));
        if let Some((_,d))=&self.session.diagnostics {ui.label(format!("Completed diagnostic interval rate (historical, not Live FPS): {:?}",d.accepted_fps(3)));}
        if let Some(f)=&self.frame {ui.label(format!("Displayed {} frame {} · {}",if f.metadata.simulated {"SIMULATION"} else {"PHYSICAL"},f.metadata.sequence,f.metadata.source));
            if let Some(a)=&f.metadata.acquisition {ui.label(format!("{:?}",a));}
        }
        if let Some(l)=&self.session.lifecycle {ui.label(format!("STOP {} · release {} · lock removed {} · collected {} · native exit {:?} · proof {}",l.stop_observed,l.release_committed,l.marker_deleted,l.collected,l.exit,l.proof_flags));}
        });
    }
}
