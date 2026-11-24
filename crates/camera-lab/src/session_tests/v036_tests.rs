use super::*;
use crate::diagnostic::{self as d, Accumulator, Cause, Reason, Stage, Terminal};
struct Inject {
    parent: Option<Cause>,
    text: Option<&'static str>,
    terminal: Terminal,
    malformed: bool,
    flags: u8,
    exit: u32,
    release: bool,
    deleted: bool,
}
struct BytesTransport {
    input: Inject,
    causes: Accumulator,
    facts: d::Facts,
    step: u8,
    reported: bool,
}
impl Transport for BytesTransport {
    fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn lifecycle(&self) -> Option<Lifecycle> {
        Some(d::lifecycle(&self.facts, &self.causes))
    }
    fn poll(&mut self) -> Result<Event, String> {
        let mut raw = [0u8; 128];
        raw[68] = self.input.flags;
        if self.step == 0 {
            if let Some(c) = self.input.parent {
                self.causes.retain(c);
            }
            d::write_terminal_error(&mut raw, self.input.text);
            if self.input.malformed {
                raw[70] = 58;
            }
            let _ = self.causes.terminal(&raw);
            self.facts.parent_error = Some(
                self.input
                    .text
                    .unwrap_or("parent SENTINEL_BUS /Users/SENTINEL_USER")
                    .into(),
            );
        } else {
            // A later different helper fault must not rewrite first cause, including unknown.
            raw[68] = 16;
            raw[69] = 4;
            self.causes.terminal(&raw).unwrap();
            self.facts.collected = true;
            self.facts.exit = Some(self.input.exit);
            self.facts.release_committed = self.input.release;
            self.facts.marker_deleted = self.input.deleted;
            self.facts.terminal = Some(self.input.terminal);
        }
        let lifecycle = d::lifecycle(&self.facts, &self.causes);
        self.step += 1;
        Ok(
            d::transport_event(&lifecycle, &mut self.reported, "SENTINEL_PORT")
                .unwrap_or(Event::Waiting),
        )
    }
}
struct InjectFactory(Option<Inject>);
impl Factory for InjectFactory {
    fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
        Ok(Box::new(BytesTransport {
            input: self.0.take().unwrap(),
            causes: Accumulator::default(),
            facts: d::Facts::default(),
            step: 0,
            reported: false,
        }))
    }
}
fn request() -> Request {
    Request {
        source: Source::Simulation,
        settings: Settings::default(),
        startup_intent: crate::startup::Intent::Normal,
    }
}
fn wait(w: &Worker, p: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let s = w.snapshot();
        if p(&s) {
            return s;
        }
        assert!(Instant::now() < deadline, "worker timeout");
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn v036_production_terminal_dispatch_worker_recorder_first_cause() {
    let cases = [
        (
            None,
            Some("@OP SENTINEL_BUS /Users/SENTINEL_USER"),
            d::backend_code("@OP x"),
        ),
        (
            None,
            Some("@DS SENTINEL_BUS /Users/SENTINEL_USER"),
            d::backend_code("@DS x"),
        ),
        (
            Some(Cause::parent(Stage::Initialization, Reason::Deadline)),
            None,
            Cause::parent(Stage::Initialization, Reason::Deadline),
        ),
        (
            Some(Cause::parent(Stage::Owner, Reason::Unresponsive)),
            None,
            Cause::parent(Stage::Owner, Reason::Unresponsive),
        ),
        (
            Some(Cause::parent(Stage::Channel, Reason::Lost)),
            None,
            Cause::parent(Stage::Channel, Reason::Lost),
        ),
        (
            None,
            Some("unknown SENTINEL_BUS /Users/SENTINEL_USER"),
            Cause::unknown(),
        ),
        (
            None,
            None,
            Cause::parent(Stage::Terminal, Reason::Malformed),
        ),
    ];
    for (index, (parent, text, expected)) in cases.into_iter().enumerate() {
        let root = std::env::temp_dir().join(format!("v036-path-{}-{index}", std::process::id()));
        let w = Worker::with_factory_recorder(
            InjectFactory(Some(Inject {
                parent,
                text,
                terminal: Terminal::FailedClean,
                malformed: index == 6,
                flags: 16,
                exit: 2,
                release: true,
                deleted: false,
            })),
            crate::failure_record::Recorder::at(root.clone()),
        );
        w.request(Some(request())).unwrap();
        wait(&w, |s| s.first_failure.is_some());
        w.close();
        wait(&w, |_| w.closed());
        let mut g = w.guardian();
        g.wait();
        let mut first = None;
        for name in ["first.json", "collection.json"] {
            let raw = std::fs::read_to_string(root.join("slot-00").join(name)).unwrap();
            for sentinel in ["SENTINEL_BUS", "SENTINEL_PORT", "SENTINEL_USER", "/Users/"] {
                assert!(!raw.contains(sentinel));
            }
            let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(v["first_failure"]["cause"], serde_json::json!(expected));
            if name == "first.json" {
                assert_eq!(v["proof_flags"], if index == 6 { 0 } else { 16 });
                assert!(v["terminal_kind"].is_null());
                assert!(v["exit"].is_null());
                assert_eq!(v["collected"], false);
                assert_eq!(v["release_committed"], false);
                assert_eq!(v["marker_deleted"], false);
                first = Some(v["first_failure"].clone());
            } else {
                assert_eq!(v["first_failure"], first.clone().unwrap());
                assert_eq!(v["helper_fault"], "Initialization");
                assert_eq!(v["terminal_kind"], "FailedClean");
                assert_eq!(v["exit"], 2);
                assert_eq!(v["proof_flags"], 16);
                assert_eq!(v["release_committed"], true);
                assert_eq!(v["marker_deleted"], false);
                assert_eq!(v["collected"], true);
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
#[expect(
    clippy::field_reassign_with_default,
    reason = "test fixture setup is kept unchanged"
)]
fn v036_all_terminal_facts_to_collection() {
    for (index, (terminal, exit, proof, release, deleted)) in [
        (Terminal::CleanCancellation, 0, 15, true, true),
        (Terminal::FailedClean, 2, 23, true, false),
        (Terminal::Unproved, 3, 16, false, false),
    ]
    .into_iter()
    .enumerate()
    {
        let mut acc = Accumulator::default();
        let mut packet = [0u8; 128];
        packet[68] = proof;
        acc.terminal(&packet).unwrap();
        let l = d::lifecycle(
            &d::Facts {
                terminal: Some(terminal),
                exit: Some(exit),
                release_committed: release,
                marker_deleted: deleted,
                collected: true,
                ..Default::default()
            },
            &acc,
        );
        let mut reported = true;
        assert!(
            matches!(d::transport_event(&l,&mut reported,"inert"),Some(Event::Done{clean}) if clean==(terminal==Terminal::CleanCancellation))
        );
        let root = std::env::temp_dir().join(format!("v036-facts-{}-{index}", std::process::id()));
        let mut recorder = crate::failure_record::Recorder::at(root.clone());
        let mut snapshot = Snapshot::default();
        snapshot.first_failure = Some("inert collection observation".into());
        snapshot.lifecycle = Some(l);
        recorder.observe(&mut snapshot);
        recorder.finish(&mut snapshot);
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("slot-00/collection.json")).unwrap())
                .unwrap();
        assert_eq!(v["terminal_kind"], serde_json::json!(terminal));
        assert_eq!(v["exit"], exit);
        assert_eq!(v["proof_flags"], proof);
        assert_eq!(v["release_committed"], release);
        assert_eq!(v["marker_deleted"], deleted);
        assert_eq!(v["collected"], true);
        assert!(v["marker_present"].is_null());
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn v036_request_hash_dependency_revision_owner_binding() {
    struct Idle;
    impl Transport for Idle {
        fn poll(&mut self) -> Result<Event, String> {
            Ok(Event::Waiting)
        }
        fn stop(&mut self) -> Result<(), String> {
            Ok(())
        }
    }
    struct HashFactory(bool);
    impl Factory for HashFactory {
        fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
            Ok(Box::new(Idle))
        }
        fn settings_digest(&mut self, s: Settings) -> Option<String> {
            settings_digest_with(s, |bytes| {
                assert!(
                    std::str::from_utf8(bytes)
                        .unwrap()
                        .starts_with("settings-v2-geometry1:")
                );
                if self.0 { Ok([9; 32]) } else { Err(()) }
            })
        }
    }
    let mut e = Engine::new(HashFactory(true));
    e.request(1, Some(request()));
    e.tick();
    assert_eq!(e.snapshot.owner_settings_digest, Some("09".repeat(32)));
    e.factory.0 = false;
    let mut b = request();
    b.settings.exposure_rows += 1;
    e.request(2, Some(b.clone()));
    assert_eq!(e.snapshot.requested, Some(b.settings));
    assert_eq!(e.snapshot.requested_revision, Some(2));
    assert!(e.snapshot.settings_digest.is_none());
    assert_eq!(e.snapshot.owner_revision, Some(1));
    assert_eq!(e.snapshot.owner_settings_digest, Some("09".repeat(32)));
    e.request(3, None);
    assert!(e.snapshot.settings_digest.is_none());
    assert!(e.snapshot.requested_revision.is_none());
    assert_eq!(e.snapshot.owner_settings_digest, Some("09".repeat(32)));
}
