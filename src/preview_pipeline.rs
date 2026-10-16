//! Bounded presentation only. No acquisition or lifecycle capabilities.
use crate::{Frame, controls::Settings};
use eframe::egui;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation {
    pub revision: u64,
    pub settings: Settings,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub generation: Generation,
    pub sequence: u64,
}
pub struct Image {
    pub held: bool,
    pub recipe: crate::display_recipe::Recipe,
    pub key: Key,
    pub raw: Frame,
    pub image: egui::ColorImage,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Counters {
    pub raw_produced: u64,
    pub raw_consumed: u64,
    pub raw_superseded: u64,
    pub rgb_produced: u64,
    pub rgb_consumed: u64,
    pub rgb_superseded: u64,
    pub stale: u64,
    pub failures: u64,
}
#[derive(Default)]
struct State {
    generation: Option<Generation>,
    recipe: crate::display_recipe::Recipe,
    processing_epoch: u64,
    last_sequence: Option<u64>,
    input: Option<(Key, Frame, crate::display_recipe::Recipe, u64, bool)>,
    output: Option<Image>,
    closed: bool,
    error: Option<String>,
    counters: Counters,
}
#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}
pub struct Pipeline(Arc<Shared>);
impl Pipeline {
    pub fn new(wake_gui: impl Fn() + Send + 'static) -> Result<Self, String> {
        Self::with_recipe_converter(wake_gui, |f,recipe| {
            let (w,h,pixels)=recipe.render(f,Some(1024))?;
            Ok(egui::ColorImage::from_rgb([w as usize,h as usize],&pixels))
        })
    }
    fn with_converter(
        wake_gui: impl Fn() + Send + 'static,
        convert: impl Fn(&Frame) -> Result<egui::ColorImage, String> + Send + 'static,
    ) -> Result<Self, String> {
        Self::with_recipe_converter(wake_gui, move |f,_| convert(f))
    }
    fn with_recipe_converter(wake_gui: impl Fn() + Send + 'static, convert: impl Fn(&Frame,crate::display_recipe::Recipe)->Result<egui::ColorImage,String> + Send + 'static)->Result<Self,String> {
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        std::thread::Builder::new().name("preview-image".into()).spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| loop {
                let (key, raw, recipe, epoch, held) = {
                    let mut s = worker.state.lock().unwrap_or_else(|e| e.into_inner());
                    while !s.closed && s.input.is_none() {
                        s = worker.wake.wait(s).unwrap_or_else(|e| e.into_inner());
                    }
                    if s.closed { return; }
                    s.counters.raw_consumed += 1;
                    s.input.take().unwrap()
                };
                let result = convert(&raw,recipe);
                let mut s = worker.state.lock().unwrap_or_else(|e| e.into_inner());
                if s.closed || (!held && s.generation != Some(key.generation)) || s.processing_epoch != epoch {
                    s.counters.stale += 1;
                    continue;
                }
                match result {
                    Ok(image) => {
                        if s.output.replace(Image { key, raw, image, recipe, held }).is_some() {
                            s.counters.rgb_superseded += 1;
                        }
                        s.counters.rgb_produced += 1;
                    }
                    Err(e) => { s.counters.failures += 1; s.error = Some(e); }
                }
                drop(s);
                wake_gui();
            }));
            if outcome.is_err() {
                let mut s = worker.state.lock().unwrap_or_else(|e| e.into_inner());
                s.closed = true;
                s.input = None;
                s.output = None;
                s.counters.failures += 1;
                s.error = Some("Preview image worker panicked; acquisition supervision remains independent".into());
                drop(s);
                // Do not invoke a possibly panicking callback twice without containment.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(wake_gui));
            }
        }).map_err(|e| format!("Preview image worker start failed: {e}"))?;
        Ok(Self(shared))
    }
    pub fn set_generation(&self, generation: Option<Generation>) {
        let mut s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if s.generation != generation {
            s.processing_epoch += 1;
            s.generation = generation;
            s.last_sequence = None;
            if s.input.take().is_some() { s.counters.raw_superseded += 1; }
            if s.output.take().is_some() { s.counters.rgb_superseded += 1; }
        }
        self.0.wake.notify_one();
    }
    pub fn set_recipe(&self, recipe: crate::display_recipe::Recipe) {
        let mut s=self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if s.recipe!=recipe {s.recipe=recipe;s.processing_epoch+=1;s.last_sequence=None;
            if s.input.take().is_some() {s.counters.raw_superseded+=1;}
            if s.output.take().is_some() {s.counters.rgb_superseded+=1;}
        }
    }
    pub fn submit(&self, key: Key, raw: Frame) -> bool {
        if raw.validate().is_err() || raw.metadata.acquisition.as_ref().is_some_and(|a|
            a.settings().ok() != Some(key.generation.settings) || a.request_revision != Some(key.generation.revision)
                || a.settings_generation != key.generation.revision) {
            return false;
        }
        let mut s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if s.closed || s.generation != Some(key.generation)
            || key.sequence != raw.metadata.sequence
            || s.last_sequence.is_some_and(|last| key.sequence <= last) { return false; }
        s.last_sequence = Some(key.sequence);
        s.counters.raw_produced += 1;
        let recipe=s.recipe;let epoch=s.processing_epoch;
        if s.input.replace((key, raw, recipe, epoch, false)).is_some() { s.counters.raw_superseded += 1; }
        self.0.wake.notify_one();
        true
    }
    /// Re-render an explicitly held image only. It never confirms acquisition.
    /// The GUI additionally requires exact equality with its still-held RAW metadata.
    pub fn submit_held(&self,raw:Frame)->bool {
        if raw.validate().is_err() {return false;}
        let mut s=self.0.state.lock().unwrap_or_else(|e|e.into_inner());
        if s.closed {return false;}
        let generation=raw.metadata.acquisition.as_ref().and_then(|a|a.settings().ok().map(|settings|Generation{revision:a.settings_generation,settings})).unwrap_or(Generation{revision:0,settings:Settings::default()});
        let key=Key{generation,sequence:raw.metadata.sequence};let recipe=s.recipe;let epoch=s.processing_epoch;
        s.counters.raw_produced+=1;
        if s.input.replace((key,raw,recipe,epoch,true)).is_some() {s.counters.raw_superseded+=1;}
        self.0.wake.notify_one();true
    }
    pub fn take(&self) -> Option<Image> {
        let mut s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        let image = s.output.take();
        if image.is_some() { s.counters.rgb_consumed += 1; }
        image
    }
    pub fn counters(&self) -> Counters {
        self.0.state.lock().unwrap_or_else(|e| e.into_inner()).counters
    }
    pub fn error(&self) -> Option<String> {
        self.0.state.lock().unwrap_or_else(|e| e.into_inner()).error.take()
    }
    pub fn terminal_failure(&self) -> bool {
        let s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        s.closed && s.counters.failures > 0
    }
    pub fn close(&self) {
        let mut s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        s.closed = true;
        if s.input.take().is_some() { s.counters.raw_superseded += 1; }
        if s.output.take().is_some() { s.counters.rgb_superseded += 1; }
        self.0.wake.notify_one();
    }
}
impl Drop for Pipeline { fn drop(&mut self) { self.close(); } }

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};
    #[test]
    fn recipe_epoch_rejects_inflight_even_when_acquisition_identity_is_same() {
        let (started,start)=mpsc::sync_channel(1);let (release,gate)=mpsc::sync_channel(1);let (done,completion)=mpsc::sync_channel(2);
        let p=Pipeline::with_recipe_converter(move || {done.try_send(()).unwrap();},move |f,r| {
            if r.ev==0. {started.send(()).unwrap();gate.recv_timeout(Duration::from_secs(3)).unwrap();}
            let (w,h,rgb)=r.render(f,Some(1024))?;Ok(egui::ColorImage::from_rgb([w as usize,h as usize],&rgb))
        }).unwrap();
        p.set_recipe(crate::display_recipe::Recipe::legacy());let g=generation(1);p.set_generation(Some(g));assert!(p.submit(key(g,1),frame(1)));start.recv_timeout(Duration::from_secs(3)).unwrap();
        let r=crate::display_recipe::Recipe{ev:1.,..crate::display_recipe::Recipe::legacy()};p.set_recipe(r);assert!(p.submit(key(g,1),frame(1)));release.send(()).unwrap();
        completion.recv_timeout(Duration::from_secs(3)).unwrap();let result=p.take().unwrap();assert_eq!(result.recipe,r);assert_eq!(result.image.pixels[0],egui::Color32::from_rgb(20,30,40));assert_eq!(p.counters().stale,1);
    }
    fn generation(revision: u64) -> Generation { Generation { revision, settings: Settings::default() } }
    fn frame(sequence: u64) -> Frame {
        Frame { raw: Arc::new(vec![0, 10, 20, 30]), metadata: crate::Metadata {
            acquisition: None,
source: "independent preview fixture".into(), simulated: true, sequence,
            measurement: None, unix_millis: 0, width: 2, height: 2, format: "BayerGRBG8".into(),
        } }
    }
    fn key(g: Generation, sequence: u64) -> Key { Key { generation: g, sequence } }
    fn image(f: &Frame) -> Result<egui::ColorImage, String> {
        let (w,h,rgb) = f.legacy_preview_rgb(1024)?;
        Ok(egui::ColorImage::from_rgb([w as usize,h as usize], &rgb))
    }
    #[test]
    fn overload_retains_latest_one_and_inflight_one_and_matching_raw() {
        let (started, start) = mpsc::sync_channel(1);
        let (release, gate) = mpsc::sync_channel(1);
        let (done, completion) = mpsc::sync_channel(8);
        let p = Pipeline::with_converter(move || { done.try_send(()).unwrap(); }, move |f| {
            if f.metadata.sequence == 1 { started.send(()).unwrap(); gate.recv_timeout(Duration::from_secs(3)).unwrap(); }
            image(f)
        }).unwrap();
        let g = generation(1); p.set_generation(Some(g));
        assert!(p.submit(key(g,1), frame(1)));
        start.recv_timeout(Duration::from_secs(3)).unwrap();
        for n in 2..=100 { assert!(p.submit(key(g,n), frame(n))); }
        {
            let s = p.0.state.lock().unwrap();
            assert_eq!(s.input.as_ref().unwrap().0.sequence, 100);
            assert!(s.output.is_none());
            // Only 4 bytes pending plus the one in-flight RAW; no frame queue.
            assert_eq!(s.input.as_ref().unwrap().1.raw.len(),4);
            assert_eq!(s.counters.raw_superseded,98);
        }
        release.send(()).unwrap();
        for _ in 0..2 { completion.recv_timeout(Duration::from_secs(3)).unwrap(); }
        let decoded = p.take().unwrap();
        assert_eq!(decoded.key.sequence,100);
        assert_eq!(decoded.raw.metadata.sequence,100);
        assert_eq!(decoded.image.pixels[0],egui::Color32::from_rgb(10,15,20));
        let c=p.counters();
        assert_eq!((c.raw_produced,c.raw_consumed,c.raw_superseded),(100,2,98));
        assert_eq!((c.rgb_produced,c.rgb_consumed,c.rgb_superseded),(2,1,1));
        assert!(!p.submit(key(g,99),frame(99)));
    }
    #[test]
    fn generation_and_profile_switch_discard_inflight_completion() {
        let (started,start)=mpsc::sync_channel(1);
        let (release,gate)=mpsc::sync_channel(1);
        let (done,completion)=mpsc::sync_channel(2);
        let p=Pipeline::with_converter(move || { done.try_send(()).unwrap(); },move |f| {
            if f.metadata.sequence==9 { started.send(()).unwrap();gate.recv_timeout(Duration::from_secs(3)).unwrap(); }
            image(f)
        }).unwrap();
        let a=generation(1);let mut b=generation(2);b.settings.mode=crate::controls::Mode::Full;
        p.set_generation(Some(a));p.submit(key(a,9),frame(9));
        start.recv_timeout(Duration::from_secs(3)).unwrap();
        p.set_generation(Some(b));
        assert!(!p.submit(key(a,10),frame(10)));
        assert!(p.submit(key(b,1),frame(1)));
        release.send(()).unwrap();completion.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(p.take().unwrap().key,key(b,1));assert_eq!(p.counters().stale,1);
    }
    #[test]
    fn revision_only_and_paused_inflight_completion() {
        let (started,start)=mpsc::sync_channel(1);
        let (release,gate)=mpsc::sync_channel(1);
        let (done,completion)=mpsc::sync_channel(2);
        let p=Pipeline::with_converter(move || {done.try_send(()).unwrap();},move |f| {
            if f.metadata.sequence==9 {started.send(()).unwrap();gate.recv_timeout(Duration::from_secs(3)).unwrap();}
            image(f)
        }).unwrap();
        let a=generation(1);let b=generation(2);assert_eq!(a.settings,b.settings);
        p.set_generation(Some(a));assert!(p.submit(key(a,9),frame(9)));
        start.recv_timeout(Duration::from_secs(3)).unwrap();
        p.set_generation(Some(b));release.send(()).unwrap();
        assert!(p.submit(key(b,1),frame(1)));
        completion.recv_timeout(Duration::from_secs(3)).unwrap();
        // Simulated pause: do not consume; generation change must clear completed slot.
        assert_eq!(p.counters().stale,1);assert_eq!(p.counters().rgb_consumed,0);
        let c=generation(3);p.set_generation(Some(c));assert!(p.take().is_none());
        assert!(!p.submit(key(b,2),frame(2)));assert!(p.submit(key(c,1),frame(1)));
        completion.recv_timeout(Duration::from_secs(3)).unwrap();assert_eq!(p.take().unwrap().key,key(c,1));
    }
    #[test]
    fn stop_close_do_not_wait_for_conversion_and_never_publish_after_close() {
        let (started,start)=mpsc::sync_channel(1);
        let (release,gate)=mpsc::sync_channel(1);
        let (finished,finish)=mpsc::sync_channel(1);
        let p=Pipeline::with_converter(|| {},move |f| {
            started.send(()).unwrap();gate.recv_timeout(Duration::from_secs(3)).unwrap();
            finished.send(()).unwrap();image(f)
        }).unwrap();
        let g=generation(1);p.set_generation(Some(g));p.submit(key(g,1),frame(1));
        start.recv_timeout(Duration::from_secs(3)).unwrap();
        p.set_generation(None);p.close();assert!(p.take().is_none());
        assert!(!p.submit(key(g,2),frame(2)));
        release.send(()).unwrap();finish.recv_timeout(Duration::from_secs(3)).unwrap();
        // Wait for predicate, not a scheduler-dependent fixed sleep.
        let deadline=std::time::Instant::now()+Duration::from_secs(3);
        while p.counters().stale==0 { assert!(std::time::Instant::now()<deadline);std::thread::yield_now(); }
        assert!(p.take().is_none());
    }
    #[test]
    fn repeated_idle_arrival_wakes_and_monotonic_sequence_checks() {
        let (done,completion)=mpsc::sync_channel(1);
        let p=Pipeline::new(move || { done.try_send(()).unwrap(); }).unwrap();
        let g=generation(1);p.set_generation(Some(g));
        for n in 1..=100 {
            assert!(p.submit(key(g,n),frame(n)));
            completion.recv_timeout(Duration::from_secs(3)).unwrap();
            assert_eq!(p.take().unwrap().key.sequence,n);
            assert!(!p.submit(key(g,n),frame(n)));
        }
        assert_eq!(p.counters().rgb_consumed,100);
    }
    #[test]
    fn panic_is_sticky_and_wakes_gui_without_lifecycle_dependency() {
        let (done,completion)=mpsc::sync_channel(1);
        let p=Pipeline::with_converter(move || { done.try_send(()).unwrap(); }, |_| panic!("injected preview failure")).unwrap();
        let g=generation(1);p.set_generation(Some(g));p.submit(key(g,1),frame(1));
        completion.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(p.error().unwrap().contains("panicked"));assert_eq!(p.counters().failures,1);
        assert!(!p.submit(key(g,2),frame(2)));assert!(p.take().is_none());
    }
}
