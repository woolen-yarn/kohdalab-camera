use super::*;
struct Make;
struct Fake {
    generation: u64,
    settings: Settings,
    stopped: bool,
    intent: crate::startup::Intent,
}
impl Factory for Make {
    fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
        unreachable!()
    }
    fn open_generation(&mut self, r: &Request, g: u64) -> Result<Box<dyn Transport>, String> {
        match r.source {
            Source::Simulation => Ok(Box::new(Sim {
                simulation: Simulation { sequence: 0 },
                stopped: false,
            })),
            Source::HostFake => {
                assert_eq!(g, 3);
                Ok(Box::new(Fake {
                    generation: g,
                    settings: r.settings,
                    stopped: false,
                    intent: r.startup_intent,
                }))
            }
            _ => panic!("no device access"),
        }
    }
}
impl Transport for Fake {
    fn stop(&mut self) -> Result<(), String> {
        self.stopped = true;
        Ok(())
    }
    fn poll(&mut self) -> Result<Event, String> {
        if self.stopped {
            return Ok(Event::Done { clean: true });
        }
        let mut f = Simulation { sequence: 0 }.next_frame()?;
        let a =
            crate::sensor_contract::Acquisition::new(self.settings, [3; 32], self.generation, 1)?
                .with_startup(self.intent);
        let (w, h) = self.settings.dimensions();
        f.raw = Arc::new(vec![0; usize::from(w) * usize::from(h)]);
        f.metadata.width = w.into();
        f.metadata.height = h.into();
        f.metadata.format = "BayerGRBG8".into();
        f.metadata.sequence = 1;
        f.metadata.acquisition = Some(a);
        Ok(Event::Frame(f))
    }
}
#[test]
fn worker_simulation_none_fake_generation3() {
    let w = Worker::with_factory(Make);
    let r = |source| Request {
        source,
        settings: Settings::default(),
        startup_intent: crate::startup::Intent::Normal,
    };
    assert_eq!(w.request(Some(r(Source::Simulation))).unwrap(), 1);
    let wait = |p: &dyn Fn(&Snapshot) -> bool| {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let s = w.snapshot();
            if p(&s) {
                return s;
            }
            assert!(Instant::now() < end, "{}", s.message);
            thread_yield();
        }
    };
    wait(&|s| s.frame.is_some());
    assert_eq!(w.request(None).unwrap(), 2);
    wait(&|s| !s.active);
    assert_eq!(w.request(Some(r(Source::HostFake))).unwrap(), 3);
    let s = wait(&|s| s.frame.is_some());
    let (revision, f) = s.frame.unwrap();
    assert_eq!(revision, 3);
    assert_eq!(f.metadata.acquisition.unwrap().settings_generation, 3);
    w.close();
    wait(&|s| !s.active);
}
fn thread_yield() {
    std::thread::sleep(Duration::from_millis(2));
}
#[test]
fn first_backend_error_survives_failed_cleanup() {
    struct F;
    struct T(u8);
    impl Factory for F {
        fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
            Ok(Box::new(T(0)))
        }
    }
    impl Transport for T {
        fn stop(&mut self) -> Result<(), String> {
            Err("later STOP failure".into())
        }
        fn poll(&mut self) -> Result<Event, String> {
            self.0 += 1;
            if self.0 == 1 {
                Err("preopen backend failure".into())
            } else {
                Ok(Event::Done { clean: false })
            }
        }
    }
    let root = std::env::temp_dir().join(format!("v034-engine-record-{}", std::process::id()));
    let mut recorder = crate::failure_record::Recorder::at(root.clone());
    let mut e = Engine::new(F);
    e.request(
        3,
        Some(Request {
            source: Source::Simulation,
            settings: Settings::default(),
            startup_intent: crate::startup::Intent::Normal,
        }),
    );
    e.tick();
    e.tick();
    assert_eq!(
        e.snapshot.first_failure.as_deref(),
        Some("preopen backend failure")
    );
    assert_eq!(e.snapshot.owner_revision, Some(3));
    recorder.observe(&mut e.snapshot);
    e.request(4, None);
    e.tick();
    assert!(e.snapshot.restart_disabled);
    assert!(!e.snapshot.active);
    assert_eq!(
        e.snapshot.first_failure.as_deref(),
        Some("preopen backend failure")
    );
    recorder.finish(&mut e.snapshot);
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(root.join("slot-00").join(name)).unwrap()).unwrap()
    };
    let first = read("first.json");
    let final_record = read("collection.json");
    assert_eq!(first["first_failure"]["code"], "TransportPoll");
    assert_eq!(first["first_failure"], final_record["first_failure"]);
    assert_eq!(final_record["owner_revision"], 3);
    assert_eq!(final_record["latest_message"]["restart_disabled"], true);
    std::fs::remove_dir_all(root).unwrap();
}
