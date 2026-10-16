#![forbid(unsafe_code)]
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(feature = "input-diagnostics")]
mod input_diagnostics;
use camera_lab::{
    Frame,
    controls::{Mode, Settings},
    session::{Request, Source, Worker},
};
use eframe::egui;
use nusb::MaybeFuture;
use camera_lab::display_recipe::Recipe;
mod parity_ui;
mod natural_ui;
mod preferences;
mod image_view;
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    time::Duration,
};
#[cfg(all(test, windows))]
mod gui_continuous_tests;
#[cfg(test)]
mod integrated_ux_tests;
#[cfg(test)]
mod parity_tests;
struct App {
    preferences_path: Option<PathBuf>,
    saved_preferences: preferences::Preferences,
    preferences_checked: std::time::Instant,
    save_folder: Option<PathBuf>,
    save_sequence: u64,
    view: image_view::View,
    fake_only: bool,
    fake_until: Option<std::time::Instant>,
    devices: Vec<nusb::DeviceInfo>,
    selected: Option<usize>,
    picker_simulation: bool,
    other_devices: Vec<nusb::DeviceInfo>,
    picker: bool,
    viewport_picker: bool,
    live_size: egui::Vec2,
    matching_revision: Option<u64>,
    displayed_revision: Option<u64>,
    scan_error: Option<String>,
    outcome_fault: Option<String>,
    connection_fault: bool,
    tab: usize,
    recipe: Recipe,
    displayed_recipe: Recipe,
    full_recipe: Recipe,
    display_size: Option<[usize;2]>,
    cursor: String,
    viewport_rect: egui::Rect,
    estimate_rx: Option<Receiver<Result<Recipe,String>>>,
    estimate_base: Option<Recipe>,
    status: String,
    terminal_preview_failure: Option<String>,
    worker: Worker,
    session: camera_lab::session::Snapshot,
    revision: u64,
    source: Option<Source>,
    settings: Settings,
    intent: camera_lab::startup::Intent,
    requested_intent: camera_lab::startup::Intent,
    product: camera_lab::product::Controller,
    full_destination: Option<PathBuf>,
    save_result: String,
    export_full: bool,
    control_error: Option<String>,
    frame: Option<Frame>,
    frame_received: Option<camera_lab::progress::FrameTiming>,
    texture: Option<egui::TextureHandle>,
    preview: Option<camera_lab::preview_pipeline::Pipeline>,
    preview_generation: Option<camera_lab::preview_pipeline::Generation>,
    gui_submits: u64,
    measurement: camera_lab::measurement::Live,
    paused: bool,
    tx: SyncSender<(Frame, PathBuf, Recipe)>,
    rx: Receiver<Result<String, String>>,
    busy: bool,
    closing: bool,
    close_started: Option<std::time::Instant>,
    scan_rx: Option<Receiver<Result<Vec<nusb::DeviceInfo>, String>>>,
}
impl App {
    fn new(ctx: &egui::Context) -> Self {
        Self::new_with_worker(ctx, Worker::new())
    }
    fn new_with_worker(ctx: &egui::Context, worker: Worker) -> Self {
        let mut app = Self::with_recipe_exporter_worker(ctx, camera_lab::image_export::save_recipe, worker);
        if std::env::args().nth(1).as_deref() == Some("--continuous-fake-gui") {
            app.fake_only = true;
            app.picker = false;
            app.fake_until = Some(std::time::Instant::now() + Duration::from_secs(65));
            #[cfg(windows)]
            if let Some(root) = std::env::args()
                .nth(2)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
            {
                app.settings.mode = Mode::Full;
                app.request(Some(Source::Fixture {
                    root,
                    fault: camera_lab::continuous_session::Fixture::FullFrames,
                }));
            } else {
                app.status = "SIMULATION requires an absolute private fake root".into();
            }
            #[cfg(not(windows))]
            {
                app.status = "Unsupported continuous fixture transport: Windows required".into();
            }
            return app;
        }
        #[cfg(feature = "input-diagnostics")]
        {
            // Diagnostic builds cannot select or open physical hardware.
            app.fake_only = true;
            app.picker_simulation = true;
            app.fake_until = Some(std::time::Instant::now() + Duration::from_secs(180));
            return app;
        }
        #[cfg(not(feature = "input-diagnostics"))]
        {
            app.preferences_path=preferences::Preferences::path();
            if let Some(path)=&app.preferences_path {
                match preferences::Preferences::load(path) {
                    Ok(p)=>app.restore_preferences(p),
                    Err(_)=>app.status="Saved settings could not be loaded; defaults restored.".into(),
                }
            }
            app.scan();
        }
        #[cfg(not(feature = "input-diagnostics"))]
        app
    }
    fn without_scan(ctx: &egui::Context) -> Self {
        Self::with_recipe_exporter_worker(ctx, camera_lab::image_export::save_recipe, Worker::new())
    }
    fn with_exporter(
        ctx: &egui::Context,
        export: impl Fn(&Frame, &std::path::Path) -> Result<(), String> + Send + 'static,
    ) -> Self {
        Self::with_exporter_worker(ctx, export, Worker::new())
    }
    fn with_exporter_worker(
        ctx: &egui::Context,
        export: impl Fn(&Frame, &std::path::Path) -> Result<(), String> + Send + 'static,
        worker: Worker,
    ) -> Self {
        Self::with_recipe_exporter_worker(ctx,move |f,p,_| export(f,p),worker)
    }
    fn with_recipe_exporter_worker(ctx:&egui::Context, export:impl Fn(&Frame,&std::path::Path,Recipe)->Result<(),String> + Send + 'static, worker:Worker)->Self {
        parity_ui::force_dark(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
        let wake = ctx.clone();
        worker.set_wake(move || wake.request_repaint());
        let repaint = ctx.clone();
        let preview = camera_lab::preview_pipeline::Pipeline::new(move || repaint.request_repaint());
        let preview_error = preview.as_ref().err().cloned();
        let (tx, rx) = mpsc::sync_channel::<(Frame, PathBuf, Recipe)>(1);
        let (rt, rr) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while let Ok((f, p, recipe)) = rx.recv() {
                if rt
                    .send(export(&f, &p, recipe).map(|_| format!("Saved image and original RAW/metadata sidecars: {}", p.display())))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            preferences_path:None,saved_preferences:Default::default(),preferences_checked:std::time::Instant::now(),save_folder:None,save_sequence:1,view:Default::default(),
            fake_only: false,
            fake_until: None,
            devices: vec![],
            selected: None,
            picker_simulation: false,
            other_devices: vec![],
            picker: true,
            viewport_picker: true,
            live_size: egui::vec2(1100.,760.),
            matching_revision: None,
            displayed_revision: None,
            scan_error: None,
            outcome_fault: None,
            connection_fault: false,
            tab: usize::MAX,
            recipe:Recipe::default(),displayed_recipe:Recipe::default(),full_recipe:Recipe::default(),
            display_size:None,cursor:String::new(),viewport_rect:egui::Rect::NOTHING,estimate_rx:None,estimate_base:None,
            terminal_preview_failure: preview_error.map(|e| format!("{e}. Reopen the application to restore preview; automatic restart is disabled.")),
            status: String::new(),
            worker,
            session: Default::default(),
            revision: 0,
            source: None,
            settings: Default::default(),
            intent: camera_lab::startup::Intent::Normal,
            requested_intent: camera_lab::startup::Intent::Normal,
            product: Default::default(),
            full_destination: None,
            save_result: String::new(),
            export_full: false,
            control_error: None,
            frame: None,
            frame_received: None,
            texture: None,
            preview: preview.ok(),
            preview_generation: None,
            gui_submits: 0,
            measurement: Default::default(),
            paused: false,
            tx,
            rx: rr,
            busy: false,
            closing: false,
            close_started: None,
            scan_rx: None,
        }
    }
    fn scan(&mut self) {
        if self.fake_only || self.scan_rx.is_some() {
            return;
        }
        self.scan_error=None;
        let (tx, rx) = mpsc::sync_channel(1);
        self.scan_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(
                nusb::list_devices()
                    .wait()
                    .map(|d| d.collect())
                    .map_err(|e| e.to_string()),
            );
        });
    }
    fn identity(d: &nusb::DeviceInfo) -> (String, Vec<u8>, u16, u16, Option<String>) {
        (
            d.bus_id().to_owned(),
            d.port_chain().to_vec(),
            d.vendor_id(),
            d.product_id(),
            d.serial_number().map(str::to_owned),
        )
    }
    fn request(&mut self, source: Option<Source>) {
        if source.is_none() { self.product.stop(); self.full_destination = None; }
        else if self.product.busy() || self.closing { return; }
        else { self.product.start(); }
        self.request_settings(source, self.settings, self.intent);
    }
    fn request_settings(&mut self, source: Option<Source>, settings: Settings, intent: camera_lab::startup::Intent) {
        if self.fake_only && matches!(source, Some(Source::Physical(_))) {
            self.control_error = Some("SIMULATION window cannot use physical transport".into());
            return;
        }
        match self
            .worker
            .request(source.clone().map(|source| Request { source, settings, startup_intent: intent }))
        {
            Ok(id) => {
                if let Some(e)=self.control_error.take() {self.outcome_fault=Some(e);}
                self.matching_revision=None;
                self.status.clear();
                if source.is_some() {
                    self.picker=false;
                    if !self.product.busy() {
                        if self.save_result.starts_with("Saved image") || self.save_result.contains("cancelled") {self.save_result.clear();}
                        if self.product.restore_result.starts_with("Original applied") {self.product.restore_result.clear();self.product.capture_result.clear();}
                    }
                }
                self.requested_intent = intent;
                self.preview_generation = source.as_ref().map(|_| camera_lab::preview_pipeline::Generation { revision: id, settings });
                if let Some(preview) = &self.preview { preview.set_generation(self.preview_generation); }
                self.revision = id;
                self.measurement.reset();
                self.source = source;
                self.control_error = None;
                self.paused = false;
                self.product.requested(id, std::time::Instant::now());
            }
            Err(e) => self.control_error = Some(e),
        }
    }
    fn restore_preferences(&mut self,p:preferences::Preferences) {
        self.settings=p.settings().expect("validated preferences");self.recipe=p.recipe;self.displayed_recipe=p.recipe;self.full_recipe=p.recipe;
        self.live_size=egui::vec2(p.live_size[0],p.live_size[1]);self.save_folder=p.folder.clone();self.save_sequence=p.next_sequence;self.saved_preferences=p;
        if let Some(preview)=&self.preview {preview.set_recipe(self.recipe);}
    }
    fn needs_periodic_updates(&self)->bool {
        self.source.is_some() || self.session.active || self.busy || self.closing || self.scan_rx.is_some() || self.estimate_rx.is_some() || self.fake_until.is_some()
    }
    fn current_preferences(&self)->preferences::Preferences {
        preferences::Preferences{version:1,mode:self.settings.mode.profile().id.into(),exposure:self.settings.exposure_rows,gain:self.settings.gain_eighths,recipe:self.recipe,live_size:[self.live_size.x,self.live_size.y],folder:self.save_folder.clone(),next_sequence:self.save_sequence}
    }
    fn persist_preferences(&mut self,force:bool) {
        let Some(path)=self.preferences_path.clone() else{return;};
        if !force && self.preferences_checked.elapsed()<Duration::from_secs(1){return;}
        self.preferences_checked=std::time::Instant::now();let p=self.current_preferences();
        if p==self.saved_preferences{return;}
        match p.save(&path) {
            Ok(())=>self.saved_preferences=p,
            Err(_)=>self.status="Settings could not be saved. Check folder permissions.".into(),
        }
    }
    fn save_dialog(&self,full:bool)->rfd::FileDialog {
        let p=self.current_preferences();
        let mut dialog=rfd::FileDialog::new().set_title(if full {"Capture Full"}else{"Save image"})
            .add_filter("PNG",&["png"]).add_filter("TIFF",&["tiff","tif"])
            .set_file_name(p.suggested_name(full));
        if let Some(folder)=self.save_folder.as_ref().filter(|p|p.is_dir()){dialog=dialog.set_directory(folder);}
        dialog
    }
    fn reset_preferences(&mut self,ctx:&egui::Context) {
        self.settings=Settings::default();self.recipe=Recipe::default();self.live_size=egui::vec2(1100.,760.);
        self.save_folder=None;self.save_sequence=1;self.view=Default::default();self.intent=camera_lab::startup::Intent::Normal;
        if !self.picker {ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(self.live_size));}
        self.status="Defaults restored. Apply camera settings to update acquisition.".into();
    }
    fn capture(&mut self) {
        if self.busy || self.closing || self.product.busy() {
            return;
        }
        // Freeze both displayed RAW and displayed recipe before the modal dialog.
        let recipe=self.displayed_recipe;
        if let Some(f) = self.frame.clone()
            && let Some(p) = self.save_dialog(false).save_file()
        {
            self.export_full=false;
            self.save_with_recipe(f, p, recipe);
        } else { self.save_result = "Save cancelled".into(); }
    }
    fn save_frozen(&mut self, f: Frame, p: PathBuf) {
            if self.busy || self.closing { self.save_result = "Save unavailable while saving or closing".into(); return; }
            self.export_full=false;
            self.save_with_recipe(f,p,self.displayed_recipe);
    }
    fn save_with_recipe(&mut self,f:Frame,p:PathBuf,recipe:Recipe) {
            if self.busy || self.closing {return;}
            let folder=p.parent().map(std::path::Path::to_path_buf);
            match self.tx.try_send((f, p, recipe)) {
                Ok(()) => {
                    self.save_folder=folder;self.save_sequence=self.save_sequence.saturating_add(1);
                    self.busy = true;
                    self.save_result = "Saving frozen RAW + software recipe…".into();
                }
                Err(e) => self.save_result = format!("Export queue error: {e}"),
            }
    }
    fn capture_full(&mut self) {
        if self.closing || self.busy || self.product.busy() || self.product.applied.is_none() { return; }
        self.full_recipe=self.recipe;
        let path = self.save_dialog(true).save_file();
        self.full_selection(path);
    }
    fn full_selection(&mut self, path: Option<PathBuf>) {
        if let Some(path) = path { self.begin_full(path); }
        else { self.save_result = "Full capture cancelled before acquisition change".into(); }
    }
    fn begin_full(&mut self, path: PathBuf) {
        if self.closing || self.busy { return; }
        self.full_recipe=self.recipe;
        let now = std::time::Instant::now();
        match self.product.begin(now) {
            Ok(full) => {
                self.export_full=true;
                if self.save_result.starts_with("Saved image") || self.save_result.contains("cancelled") {self.save_result.clear();}
                self.full_destination = Some(path);
                if self.product.applied.is_some_and(|a| a.settings != full.settings) {
                    self.request_settings(self.source.clone(), full.settings, full.intent);
                    if let Some(e) = self.control_error.clone() { self.product.fail(&e); }
                }
            }
            Err(e) => self.control_error = Some(e),
        }
    }
    #[cfg(test)]
    fn show_frame(&mut self, ctx: &egui::Context, f: Frame) {
        let (width, height, pixels) = match f.preview_rgb(1024) {
            Ok(p) => p,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        let im = egui::ColorImage::from_rgb([width as usize, height as usize], &pixels);
        self.upload_frame(ctx, f, im, std::time::Instant::now());
    }
    fn upload_frame(&mut self, ctx: &egui::Context, f: Frame, im: egui::ColorImage, now: std::time::Instant) {
        self.display_size=Some(im.size);
        if let Some(t) = self.texture.as_mut() {
            t.set(im, egui::TextureOptions::LINEAR);
        } else {
            self.texture = Some(ctx.load_texture("preview", im, egui::TextureOptions::LINEAR));
        }
        self.frame_received = Some(camera_lab::progress::FrameTiming::received_at(
            &f,
            now,
        ));
        if self.source.is_some() && f.metadata.acquisition.as_ref().is_some_and(|a|a.settings_generation==self.revision) {self.measurement.submit(&f,now);}
        self.frame = Some(f);
        self.gui_submits += 1;
    }
    fn render(&mut self, ctx: &egui::Context) {
        self.render_at(ctx, std::time::Instant::now);
    }
    fn render_at(&mut self, ctx: &egui::Context, mut clock: impl FnMut() -> std::time::Instant) {
        #[cfg(feature = "input-diagnostics")]
        input_diagnostics::events(ctx);
        let now = clock();
        if let Some(result) = self.scan_rx.as_ref().and_then(|r| match r.try_recv() {
            Ok(v) => Some(v),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("Enumeration worker disconnected".into()))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }) {
            self.scan_rx = None;
            let identity = self
                .selected
                .and_then(|i| self.devices.get(i))
                .map(Self::identity);
            match result {
                Ok(devices) => {
                    let (supported,other)=devices.into_iter().partition(|d|d.vendor_id()==0x0547 && d.product_id()==0x4d33);
                    self.devices = supported;self.other_devices=other;
                    self.selected = Self::refreshed_selection(identity,self.devices.iter().map(Self::identity));
                }
                Err(e) => {self.selected=None;self.scan_error=Some(format!("Could not refresh cameras. Try Refresh. {e}"));},
            }
        }
        match self.rx.try_recv() {
            Ok(r) => {
                self.busy = false;
                self.save_result = r.unwrap_or_else(|e| format!("Save failed: {e}"));
                if !self.save_result.starts_with("Saved image") {self.outcome_fault=Some(self.save_result.clone());}
            }
            Err(mpsc::TryRecvError::Disconnected) if self.busy => {
                self.busy = false;
                self.save_result = "Export worker disconnected; file completion unproved".into();
                self.outcome_fault=Some(self.save_result.clone());
            }
            _ => {}
        }
        // Close has precedence over frame-driven automatic restoration.
        if (ctx.input(|i| i.viewport().close_requested()) || self.fake_until.is_some_and(|t| now >= t)) && !self.closing {
            self.closing = true;
            self.product.stop(); self.full_destination = None;
            self.preview_generation = None;
            if let Some(preview) = &self.preview { preview.close(); }
            self.worker.close();
        }
        self.session = self.worker.snapshot();
        if self.session.restart_disabled || self.session.state == camera_lab::session::State::Error {
            self.connection_fault=true;
            self.outcome_fault=Some(self.session.message.clone());
            self.product.fail(&self.session.message);
        }
        if self.product.tick(now) { self.request(None); }
        if let Some((id, f)) = self.session.frame.take()
            && id == self.revision && !self.closing
        {
            if let Some(generation) = self.preview_generation {
                if f.validate().is_ok() && (f.metadata.simulated || self.session.applied_revision==Some(id)) {
                    self.matching_revision=Some(id);self.status.clear();
                }
                self.measurement.receive(&f,clock());
                let intent = self.requested_intent;
                if let Some(restore) = self.product.observe(&f, id, generation.settings, intent, now) {
                    if let (Some(path), Some(held)) = (self.full_destination.take(), self.product.held.clone()) {
                        self.save_with_recipe(held, path, self.full_recipe);
                    }
                    // An already-Full capture needs another sequence, not another owner.
                    if generation.settings != restore.settings {
                        self.request_settings(self.source.clone(), restore.settings, restore.intent);
                        if let Some(e) = self.control_error.clone() { self.product.fail(&e); }
                    }
                }
                if !self.paused && let Some(preview) = &self.preview {
                    preview.submit(camera_lab::preview_pipeline::Key { generation, sequence: f.metadata.sequence }, f);
                }
            }
        }
        if let Some(preview) = &self.preview {
            if let Some(e) = preview.error() {
                if preview.terminal_failure() {
                    self.terminal_preview_failure = Some(format!("{e}. Reopen the application to restore preview; automatic restart is disabled."));
                } else { self.status = e; }
            }
        }
        if (!self.paused || self.recipe!=self.displayed_recipe) && !self.closing {
            if let Some(image) = self.preview.as_ref().and_then(|p| p.take()) {
                let matches=if image.held {self.frame.as_ref().is_some_and(|f|f.metadata==image.raw.metadata && std::sync::Arc::ptr_eq(&f.raw,&image.raw.raw))} else {Some(image.key.generation)==self.preview_generation};
                if matches && image.recipe==self.recipe {
                    if !image.held {self.displayed_revision=Some(image.key.generation.revision);}
                    self.displayed_recipe=image.recipe;
                    self.upload_frame(ctx, image.raw, image.image, clock());
                }
            }
        }
        if (ctx.input(|i| i.viewport().close_requested())
            || self.fake_until.is_some_and(|t| now >= t))
            && !self.closing
        {
            self.closing = true;
            self.preview_generation = None;
            if let Some(preview) = &self.preview { preview.close(); }
            self.worker.close();
            self.session = self.worker.snapshot();
        }
        if self.closing {
            let started = self.close_started.get_or_insert(now);
            let deadline = now.saturating_duration_since(*started) >= Duration::from_secs(70);
            if deadline {
                self.status = "Close deadline: acquisition cleanup/export completion unproved; inspect evidence before retry".into();
            }
            if self.worker.closed() && !self.busy {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
        // Observe after upload: an entry timestamp precedes the submitted frame and
        // would falsely trip Rolling's genuine clock-regression guard each update.
        self.parity_ui(ctx,clock());
        self.persist_preferences(self.closing);
        if self.needs_periodic_updates() {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if self.preferences_path.is_some() && self.current_preferences()!=self.saved_preferences {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.render(ctx);
    }
}
fn main() -> eframe::Result {
    // Explicit support command: enumeration only; no Worker, USB open or helper launch.
    if std::env::args().nth(1).as_deref()==Some("--startup-report") {
        let result=(|| -> Result<(),String> {
            let destination=std::env::args().nth(2).ok_or("Expected a new JSON output path")?;
            let devices=nusb::list_devices().wait().map_err(|e|e.to_string())?;
            let cameras:Vec<_>=devices.filter(|d|d.vendor_id()==0x0547 && d.product_id()==0x4d33).map(|d| {
                #[cfg(windows)] let driver=d.driver().map(str::to_owned);
                #[cfg(not(windows))] let driver:Option<String>=None;
                serde_json::json!({"product":d.product_string(),"usb_id":"0547:4d33","installed_driver":driver,
                    "driver_connection_error":camera_lab::driver_policy::connection_error(&d)})
            }).collect();
            let helper=std::env::current_exe().map_err(|e|e.to_string())?.with_file_name("continuous-helper.exe");
            let report=serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,
                "driver_policy":"preserve-installed-driver/v1","vendor_sdk_backend_available":false,
                "helper_present":helper.is_file(),"cameras":cameras,"usb_opens":0,"helper_starts":0,"driver_changes":0});
            let file=std::fs::OpenOptions::new().write(true).create_new(true).open(destination).map_err(|e|e.to_string())?;
            serde_json::to_writer_pretty(file,&report).map_err(|e|e.to_string())
        })();
        if let Err(e)=result {eprintln!("Startup report failed: {e}");std::process::exit(1);}
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--physical-adapter-test") {
        #[cfg(feature = "input-diagnostics")]
        {
            eprintln!("Physical adapter access is disabled in input diagnostic builds");
            std::process::exit(2);
        }
        #[cfg(not(feature = "input-diagnostics"))]
        if let Err(e) = physical_adapter_test() {
            eprintln!("Physical adapter test failed: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--list-usb") {
        match nusb::list_devices().wait() {
            Ok(devices) => {
                for d in devices {
                    println!(
                        "{:04x}:{:04x} product={:?} serial={:?} id={:?}",
                        d.vendor_id(),
                        d.product_id(),
                        d.product_string(),
                        d.serial_number(),
                        d.id()
                    );
                }
            }
            Err(e) => {
                eprintln!("USB enumeration failed: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let worker = Worker::new();
    let mut guardian = worker.guardian();
    // Safe Rust unwind contract: discard App/GUI state; never reuse it after
    // unwind. Only the separately owned Worker continues collection.
    // Abort, OOM and external kill cannot be caught.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        eframe::run_native(
            concat!("Select a camera — KohdaLab Camera v",env!("CARGO_PKG_VERSION")),
            eframe::NativeOptions {
                // Present ready camera frames without waiting for display refresh.
                // Repainting remains driven by readiness and the bounded status timer.
                vsync: false,
                viewport: egui::ViewportBuilder::default()
                    .with_icon(application_icon())
                    .with_inner_size([640., 480.])
                    .with_min_inner_size([520., 360.]),
                ..Default::default()
            },
            Box::new(move |cc| Ok(Box::new(App::new_with_worker(&cc.egui_ctx, worker)))),
        )
    }));
    if !matches!(&result, Ok(Ok(()))) {
        guardian.gui_failed();
    }
    guardian.wait(); // after teardown, never in painting or startup
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn physical_adapter_test() -> Result<(), String> {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(80));
        eprintln!("Adapter test deadline; inspect child cleanup before further writes");
        std::process::exit(124);
    });
    let mode = std::env::args()
        .nth(2)
        .ok_or("Use --physical-adapter-test small|full|stop")?;
    if !matches!(mode.as_str(), "small" | "full" | "stop") {
        return Err("Unknown adapter test mode".into());
    }
    let devices: Vec<_> = nusb::list_devices()
        .wait()
        .map_err(|e| e.to_string())?
        .filter(|d| d.vendor_id() == 0x0547 && d.product_id() == 0x4d33 && d.port_chain() == [2, 1])
        .collect();
    if devices.len() != 1 {
        return Err("Expected unique explicitly tested physical port 2.1".into());
    }
    let mut adapter =
        camera_lab::physical_preview::PhysicalPreview::start(&devices[0], mode == "full")?;
    println!("session={}", adapter.directory.display());
    let mut exporter = None;
    let mut deliveries = 0u64;
    let mut stop_sent = false;
    loop {
        match adapter.poll() {
            Ok((frame, status)) => {
                if let Some(frame) = frame {
                    deliveries += 1;
                    if frame.metadata.simulated {
                        return Err("Unexpected simulation".into());
                    }
                    if exporter.is_none() {
                        let directory = adapter.directory.clone();
                        exporter = Some(std::thread::spawn(move || -> Result<(), String> {
                            for extension in ["png", "tiff"] {
                                let name = format!("physical.{extension}");
                                camera_lab::export(&frame, &directory.join(&name))?;
                                let bundle = directory.join(format!("{name}.capture"));
                                if image::open(bundle.join(&name))
                                    .map_err(|e| e.to_string())?
                                    .to_rgb8()
                                    .as_raw()
                                    != frame.legacy_display_rgb().as_ref()
                                {
                                    return Err("Physical RGB export mismatch".into());
                                }
                                if std::fs::read(bundle.join("raw.bin"))
                                    .map_err(|e| e.to_string())?
                                    != *frame.raw
                                {
                                    return Err("Original Bayer bytes mismatch".into());
                                }
                                let metadata: camera_lab::Metadata = serde_json::from_slice(
                                    &std::fs::read(bundle.join("metadata.json"))
                                        .map_err(|e| e.to_string())?,
                                )
                                .map_err(|e| e.to_string())?;
                                if metadata != frame.metadata {
                                    return Err("Metadata mismatch".into());
                                }
                            }
                            Ok(())
                        }));
                    }
                    if mode == "stop" && !stop_sent {
                        adapter.stop()?;
                        stop_sent = true;
                    }
                }
                if let Some(status) = status {
                    println!("{status}");
                    break;
                }
            }
            Err(error) if stop_sent => {
                if !adapter.directory.join("capture/CLEANUP_OK").exists() {
                    return Err(error);
                }
                println!("Requested stop recovered with verified cleanup");
                break;
            }
            Err(error) => return Err(error),
        }
        std::thread::sleep(Duration::from_millis(33));
    }
    exporter
        .ok_or("No physical preview frames received")?
        .join()
        .map_err(|_| "Exporter panicked")??;
    println!(
        "preview_deliveries={deliveries}; physical RGB PNG/TIFF decode equality, original Bayer raw and metadata equality passed. Acquisition counts are in child metrics."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
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
}

// Use the same project artwork for native windows and the packaged executable.
fn application_icon() -> egui::IconData {
    let image = image::load_from_memory(include_bytes!("../assets/kohdalab-camera.png"))
        .expect("embedded application icon must be valid PNG")
        .into_rgba8();
    let (width, height) = image.dimensions();
    egui::IconData { rgba: image.into_raw(), width, height }
}
