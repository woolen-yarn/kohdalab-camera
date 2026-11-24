use super::*;
#[test]
fn stale_typed_generation_cannot_be_presented() {
    struct F;
    struct T;
    impl Factory for F {
        fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
            Ok(Box::new(T))
        }
    }
    impl Transport for T {
        fn stop(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn poll(&mut self) -> Result<Event, String> {
            let settings = Settings::default();
            let (w, h) = settings.dimensions();
            let a = crate::sensor_contract::Acquisition::new(settings, [2; 32], 2, 1)?
                .with_startup(crate::startup::Intent::Normal);
            Ok(Event::Frame(Frame {
                raw: Arc::new(vec![0; usize::from(w) * usize::from(h)]),
                metadata: crate::Metadata {
                    acquisition: Some(a),
                    source: "host fake".into(),
                    simulated: true,
                    sequence: 1,
                    measurement: None,
                    unix_millis: 0,
                    width: w.into(),
                    height: h.into(),
                    format: "BayerGRBG8".into(),
                },
            }))
        }
    }
    let mut e = Engine::new(F);
    e.request(
        3,
        Some(Request {
            source: Source::HostFake,
            settings: Settings::default(),
            startup_intent: crate::startup::Intent::Normal,
        }),
    );
    e.tick();
    e.tick();
    assert!(e.snapshot.frame.is_none());
    assert_eq!(
        e.snapshot.first_failure.as_deref(),
        Some("Frame settings contract mismatch")
    );
    assert!(e.snapshot.active);
}
