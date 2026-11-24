use super::*;
struct Inject {
    panic: bool,
}
impl Factory for Inject {
    fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
        unreachable!()
    }
    fn open_generation(&mut self, _: &Request, _: u64) -> Result<Box<dyn Transport>, String> {
        if self.panic {
            panic!("injected pre-owner panic");
        }
        Err("Continuous acquisition failed: backend preopen. Evidence: bus=SENTINEL_BUS ports=SENTINEL_PORT /Users/SENTINEL_USER/private".into())
    }
}
fn req() -> Request {
    Request {
        source: Source::Simulation,
        settings: Settings::default(),
        startup_intent: crate::startup::Intent::Normal,
    }
}
fn wait(w: &Worker, p: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let s = w.snapshot();
        if p(&s) {
            return s;
        }
        assert!(Instant::now() < end, "{}", s.message);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn worker_production_open_error_excludes_identity_both_records() {
    let root = std::env::temp_dir().join(format!("v035-identity-{}", std::process::id()));
    let w = Worker::with_factory_recorder(
        Inject { panic: false },
        crate::failure_record::Recorder::at(root.clone()),
    );
    w.request(Some(req())).unwrap();
    let s = wait(&w, |s| s.first_failure.is_some());
    assert!(s.first_failure.unwrap().contains("SENTINEL_BUS"));
    w.close();
    wait(&w, |_| w.closed());
    let mut guardian = w.guardian();
    guardian.wait();
    for name in ["first.json", "collection.json"] {
        let bytes = std::fs::read_to_string(root.join("slot-00").join(name)).unwrap();
        for sentinel in ["SENTINEL_BUS", "SENTINEL_PORT", "SENTINEL_USER", "/Users/"] {
            assert!(!bytes.contains(sentinel));
        }
        let v: serde_json::Value = serde_json::from_str(&bytes).unwrap();
        assert_eq!(v["first_failure"]["code"], "OpenGeneration");
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn worker_physical_style_lifecycle_message_safe_records() {
    struct F;
    struct T(bool);
    impl Factory for F {
        fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
            Ok(Box::new(T(false)))
        }
    }
    impl Transport for T {
        fn stop(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn lifecycle(&self) -> Option<Lifecycle> {
            Some(Lifecycle {
                helper_fault: Some(crate::continuous::Fault::Initialization),
                backend_error: Some("preopen SENTINEL_BUS /Users/SENTINEL_USER".into()),
                parent_error: Some("SENTINEL_PORT".into()),
                terminal_kind: Some(crate::diagnostic::Terminal::Unproved),
                ..Default::default()
            })
        }
        fn poll(&mut self) -> Result<Event, String> {
            if self.0 {
                return Ok(Event::Done { clean: false });
            }
            self.0 = true;
            Ok(Event::Lifecycle {
                message: continuous_failure_message(
                    "preopen IO timeout",
                    "bus=SENTINEL_BUS ports=SENTINEL_PORT /Users/SENTINEL_USER/private",
                ),
                unproved: true,
            })
        }
    }
    let root = std::env::temp_dir().join(format!("v035-lifecycle-{}", std::process::id()));
    let w = Worker::with_factory_recorder(F, crate::failure_record::Recorder::at(root.clone()));
    w.request(Some(req())).unwrap();
    let s = wait(&w, |s| s.first_failure.is_some());
    assert!(s.first_failure.unwrap().contains("SENTINEL_BUS"));
    w.close();
    wait(&w, |_| w.closed());
    let mut g = w.guardian();
    g.wait();
    for name in ["first.json", "collection.json"] {
        let raw = std::fs::read_to_string(root.join("slot-00").join(name)).unwrap();
        for value in ["SENTINEL_BUS", "SENTINEL_PORT", "SENTINEL_USER", "/Users/"] {
            assert!(!raw.contains(value));
        }
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["first_failure"]["code"], "Unknown");
        assert_eq!(v["helper_fault"], "Initialization");
        assert_eq!(v["terminal_kind"], "Unproved");
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn worker_open_panic_publishes_full_failure_and_failed_persistence() {
    let root = std::env::temp_dir().join(format!("v035-panic-{}", std::process::id()));
    std::fs::write(&root, b"blocked").unwrap();
    let w = Worker::with_factory_recorder(
        Inject { panic: true },
        crate::failure_record::Recorder::at(root.clone()),
    );
    w.request(Some(req())).unwrap();
    let s = wait(&w, |_| w.closed());
    assert!(!s.active);
    assert!(s.restart_disabled);
    assert!(
        s.first_failure
            .as_deref()
            .unwrap()
            .contains("worker panicked")
    );
    assert!(s.first_failure_original_bytes.unwrap() > 0);
    assert!(s.persistence.starts_with("FAILURE RECORD NOT PERSISTED"));
    assert!(s.frame.is_none());
    assert_eq!(s.owner_revision, Some(1));
    let mut guardian = w.guardian();
    guardian.wait();
    std::fs::remove_file(root).unwrap();
}
#[test]
fn request_none_clears_facts_owner_remains_revision_bound() {
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
            Ok(Event::Waiting)
        }
    }
    let mut e = Engine::new(F);
    e.request(1, Some(req()));
    e.snapshot.settings_digest = Some("digest-A".into());
    e.tick();
    let mut b = req();
    b.settings.exposure_rows += 1;
    e.request(2, Some(b.clone()));
    e.tick();
    assert_eq!(e.snapshot.owner_revision, Some(1));
    assert_eq!(e.snapshot.owner_settings, Some(req().settings));
    assert_eq!(
        e.snapshot.owner_settings_digest.as_deref(),
        Some("digest-A")
    );
    assert_eq!(e.snapshot.requested_revision, Some(2));
    assert_eq!(e.snapshot.requested, Some(b.settings));
    let b_json: serde_json::Value =
        serde_json::from_slice(&crate::failure_record::record(&e.snapshot, "test")).unwrap();
    assert_eq!(b_json["owner_revision"], 1);
    assert_eq!(b_json["requested_revision"], 2);
    assert_eq!(b_json["owner_settings_digest"], "digest-A");
    #[cfg(not(windows))]
    assert!(b_json["settings_digest"].is_null());
    #[cfg(windows)]
    {
        let canonical = format!(
            "settings-v2-geometry1:{}:{}:{}",
            b.settings.mode.profile().id,
            b.settings.exposure_rows,
            b.settings.gain_eighths
        );
        let expected = capability_platform::digest(canonical.as_bytes())
            .expect("Windows digest success required in canonical-B test")
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>();
        assert_eq!(b_json["settings_digest"], expected);
    }
    e.request(3, None);
    assert!(e.snapshot.requested.is_none());
    assert!(e.snapshot.requested_revision.is_none());
    assert!(e.snapshot.settings_digest.is_none());
    assert!(e.snapshot.startup_intent.is_none());
    assert_eq!(e.snapshot.owner_revision, Some(1));
    assert_eq!(
        e.snapshot.owner_settings_digest.as_deref(),
        Some("digest-A")
    );
    let stopped: serde_json::Value =
        serde_json::from_slice(&crate::failure_record::record(&e.snapshot, "test")).unwrap();
    for field in [
        "requested_settings",
        "requested_revision",
        "settings_digest",
        "startup_intent",
    ] {
        assert!(stopped[field].is_null(), "{field}");
    }
    assert_eq!(stopped["owner_revision"], 1);
    assert_eq!(stopped["owner_settings_digest"], "digest-A");
    assert_eq!(settings_digest_with(req().settings, |_| Err(())), None);
    assert_eq!(
        settings_digest_with(req().settings, |bytes| {
            assert!(
                std::str::from_utf8(bytes)
                    .unwrap()
                    .starts_with("settings-v2-geometry1:")
            );
            Ok([7; 32])
        }),
        Some("07".repeat(32))
    );
    #[cfg(not(windows))]
    {
        let mut e = Engine::new(F);
        e.request(1, Some(req()));
        assert!(e.snapshot.settings_digest.is_none());
    }
}
