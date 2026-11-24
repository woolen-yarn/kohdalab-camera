use super::*;
use std::collections::VecDeque;
#[test]
fn gui_failure_during_active_poll_cannot_be_overwritten_by_old_snapshot() {
    use std::sync::mpsc;
    struct Make(Option<Held>);
    struct Held {
        entered: mpsc::SyncSender<()>,
        release: mpsc::Receiver<()>,
        count: u8,
    }
    impl Factory for Make {
        fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
            Ok(Box::new(self.0.take().unwrap()))
        }
    }
    impl Transport for Held {
        fn stop(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn poll(&mut self) -> Result<Event, String> {
            self.count += 1;
            self.entered.send(()).unwrap();
            self.release.recv().unwrap();
            Ok(if self.count == 2 {
                Event::Done { clean: true }
            } else {
                Event::Waiting
            })
        }
    }
    let (entered, ready) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let worker = Worker::with_factory(Make(Some(Held {
        entered,
        release: released,
        count: 0,
    })));
    let mut guardian = worker.guardian();
    worker
        .request(Some(Request {
            startup_intent: crate::startup::Intent::Normal,
            source: Source::Simulation,
            settings: Settings::default(),
        }))
        .unwrap();
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    guardian.gui_failed();
    release.send(()).unwrap();
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    let observed = worker.snapshot();
    release.send(()).unwrap();
    guardian.wait();
    assert_eq!(observed.state, State::Error);
    assert!(observed.restart_disabled);
}
#[test]
fn held_poll_preserves_ui_origin_and_immediate_close_snapshot() {
    use std::sync::mpsc::{self, Receiver, SyncSender};
    struct HeldFactory(Option<Held>);
    struct Held {
        entered: SyncSender<()>,
        release: Receiver<()>,
        stopped: bool,
    }
    impl Factory for HeldFactory {
        fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
            Ok(Box::new(self.0.take().unwrap()))
        }
    }
    impl Transport for Held {
        fn stop(&mut self) -> Result<(), String> {
            self.stopped = true;
            Ok(())
        }
        fn poll(&mut self) -> Result<Event, String> {
            self.entered.send(()).unwrap();
            self.release.recv().unwrap();
            Ok(if self.stopped {
                Event::Done { clean: true }
            } else {
                Event::Progress(crate::progress::Stage::Controls)
            })
        }
    }
    let (entered, observed) = mpsc::sync_channel(1);
    let (release, held) = mpsc::sync_channel(1);
    let w = Worker::with_factory(HeldFactory(Some(Held {
        entered,
        release: held,
        stopped: false,
    })));
    let origin = Instant::now();
    w.request_at(Some(request(100)), origin).unwrap();
    observed.recv_timeout(Duration::from_secs(3)).unwrap(); // poll held
    let change = origin + Duration::from_secs(4);
    w.request_at(Some(request(50)), change).unwrap();
    assert_eq!(w.snapshot().started, Some(change));
    release.send(()).unwrap(); // obsolete poll cannot replace immediate snapshot
    observed.recv_timeout(Duration::from_secs(3)).unwrap(); // old owner stopping
    assert_eq!(w.snapshot().started, Some(change));
    // Inject an observed owner setting to verify close preserves it until completion.
    let applied = request(100).settings;
    {
        let mut s = w.shared.lock().unwrap();
        s.snapshot.applied = Some(applied);
        s.snapshot.progress = Some(crate::progress::Stage::Live);
        s.snapshot.stage_started = Some(origin);
    }
    let closing = origin + Duration::from_secs(9);
    w.close_at(closing);
    let s = w.snapshot();
    assert_eq!(s.state, State::Stopping);
    assert_eq!(s.started, Some(closing));
    assert_eq!(s.applied, Some(applied));
    assert!(s.progress.is_none());
    assert!(s.stage_started.is_none());
    assert!(s.requested.is_none());
    assert!(s.frame.is_none());
    w.close_at(closing + Duration::from_secs(1));
    assert_eq!(w.snapshot().started, Some(closing));
    release.send(()).unwrap();
    let limit = Instant::now() + Duration::from_secs(3);
    while !w.closed() && Instant::now() < limit {
        std::thread::yield_now();
    }
    assert!(w.closed());
    assert_eq!(w.snapshot().started, Some(closing));
}
#[test]
fn injected_stop_deadline_exact_boundary_retains_owner() {
    let (mut e, opens) = engine(vec![vec![]]);
    let zero = Instant::now();
    e.request_at(1, Some(request(100)), zero);
    e.tick_at(zero);
    e.request_at(2, None, zero + Duration::from_secs(1));
    e.tick_at(zero + Duration::from_secs(2));
    assert_eq!(
        e.active.as_ref().unwrap().stop_at,
        Some(zero + Duration::from_secs(2))
    );
    e.tick_at(zero + Duration::from_secs(67) - Duration::from_nanos(1));
    assert!(!e.blocked);
    e.tick_at(zero + Duration::from_secs(67));
    assert!(e.blocked);
    assert!(e.snapshot.active);
    e.request_at(3, Some(request(50)), zero + Duration::from_secs(68));
    e.tick_at(zero + Duration::from_secs(200));
    assert_eq!(*opens.lock().unwrap(), vec![100]);
    assert_eq!(
        e.active.as_ref().unwrap().stop_at,
        Some(zero + Duration::from_secs(2))
    );
}
struct Script {
    opens: Arc<Mutex<Vec<u16>>>,
    live: Arc<Mutex<usize>>,
    events: VecDeque<VecDeque<Result<Event, String>>>,
}
struct Fake {
    events: VecDeque<Result<Event, String>>,
    live: Arc<Mutex<usize>>,
    stops: Arc<Mutex<usize>>,
}
impl Drop for Fake {
    fn drop(&mut self) {
        *self.live.lock().unwrap() -= 1;
    }
}
impl Transport for Fake {
    fn poll(&mut self) -> Result<Event, String> {
        self.events.pop_front().unwrap_or(Ok(Event::Waiting))
    }
    fn stop(&mut self) -> Result<(), String> {
        *self.stops.lock().unwrap() += 1;
        Ok(())
    }
}
impl Factory for Script {
    fn open(&mut self, r: &Request) -> Result<Box<dyn Transport>, String> {
        assert_eq!(
            *self.live.lock().unwrap(),
            0,
            "overlapping device ownership"
        );
        *self.live.lock().unwrap() += 1;
        self.opens.lock().unwrap().push(r.settings.exposure_rows);
        Ok(Box::new(Fake {
            events: self.events.pop_front().unwrap(),
            live: self.live.clone(),
            stops: Arc::new(Mutex::new(0)),
        }))
    }
}
fn request(rows: u16) -> Request {
    Request {
        startup_intent: crate::startup::Intent::Normal,
        source: Source::Simulation,
        settings: Settings {
            exposure_rows: rows,
            ..Default::default()
        },
    }
}
fn frame() -> Frame {
    Simulation { sequence: 0 }.next_frame().unwrap()
}
fn engine(events: Vec<Vec<Result<Event, String>>>) -> (Engine<Script>, Arc<Mutex<Vec<u16>>>) {
    let opens = Arc::new(Mutex::new(vec![]));
    (
        Engine::new(Script {
            opens: opens.clone(),
            live: Arc::new(Mutex::new(0)),
            events: events.into_iter().map(VecDeque::from).collect(),
        }),
        opens,
    )
}
#[test]
fn progress_coalesces_and_stale_revision_cannot_certify_settings() {
    use crate::progress::Stage;
    let (mut e, opens) = engine(vec![
        vec![
            Ok(Event::Progress(Stage::Pattern155)),
            Ok(Event::Progress(Stage::Identity)),
            Ok(Event::Progress(Stage::Live)),
            Ok(Event::Done { clean: true }),
        ],
        vec![
            Ok(Event::Progress(Stage::Controls)),
            Ok(Event::Frame(frame())),
        ],
    ]);
    let zero = Instant::now();
    e.request_at(1, Some(request(100)), zero);
    e.tick_at(zero);
    e.tick_at(zero + Duration::from_secs(3));
    assert_eq!(e.snapshot.progress, Some(Stage::Pattern155));
    assert_eq!(
        e.snapshot.stage_started,
        Some(zero + Duration::from_secs(3))
    );
    e.tick_at(zero + Duration::from_secs(4));
    assert_eq!(e.snapshot.progress, Some(Stage::Pattern155)); // regression ignored
    for id in 2..=9 {
        e.request_at(id, Some(request(id as u16)), zero + Duration::from_secs(5));
    }
    e.tick_at(zero + Duration::from_secs(6)); // stale Live
    assert_eq!(e.snapshot.progress, None);
    assert_eq!(e.snapshot.requested, Some(request(9).settings));
    assert!(e.snapshot.applied.is_none());
    e.tick_at(zero + Duration::from_secs(7));
    e.tick_at(zero + Duration::from_secs(8));
    e.tick_at(zero + Duration::from_secs(10));
    assert_eq!(*opens.lock().unwrap(), vec![100, 9]);
    assert_eq!(e.snapshot.progress, Some(Stage::Controls));
    assert_eq!(e.snapshot.started, Some(zero + Duration::from_secs(5)));
    assert_eq!(
        e.snapshot.stage_started,
        Some(zero + Duration::from_secs(10))
    );
    assert!(e.snapshot.applied.is_none()); // protocol cannot certify application
    e.tick_at(zero + Duration::from_secs(11));
    assert_eq!(e.snapshot.state, State::Streaming);
}
#[test]
fn terminal_presentation_distinguishes_end_cancel_and_error() {
    for (cancel, error, clean) in [
        (false, false, true),
        (true, false, true),
        (false, true, true),
        (false, false, false),
    ] {
        let (mut e, _) = engine(vec![vec![
            if error {
                Err("fixture failure".into())
            } else {
                Ok(Event::Waiting)
            },
            Ok(Event::Done { clean }),
        ]]);
        let now = Instant::now();
        e.request_at(1, Some(request(100)), now);
        e.tick_at(now);
        e.tick_at(now + Duration::from_secs(2));
        if cancel {
            e.request_at(2, None, now + Duration::from_secs(3));
            assert_eq!(e.snapshot.started, Some(now + Duration::from_secs(3)));
        }
        e.tick_at(now + Duration::from_secs(4));
        assert!(!e.snapshot.active);
        assert!(e.snapshot.applied.is_none());
        assert_eq!(
            e.snapshot.state,
            if error || !clean {
                State::Error
            } else {
                State::Disconnected
            }
        );
        assert!(e.snapshot.message.contains(if !clean {
            "Cleanup unproved"
        } else if error {
            "fixture failure"
        } else if cancel {
            "cancelled"
        } else {
            "Simulation ended"
        }));
        e.tick_at(now + Duration::from_secs(100));
        assert!(!e.snapshot.active); // no automatic trial loop
    }
}
#[test]
fn repeated_requests_wait_for_cleanup_and_coalesce_stale_frames() {
    let (mut e, opens) = engine(vec![
        vec![
            Ok(Event::Frame(frame())),
            Ok(Event::Waiting),
            Ok(Event::Frame(frame())),
            Ok(Event::Done { clean: true }),
        ],
        vec![Ok(Event::Frame(frame()))],
    ]);
    e.request(1, Some(request(100)));
    e.tick();
    e.tick();
    assert_eq!(e.snapshot.state, State::Streaming);
    assert!(e.accepts(1));
    let captured = e.snapshot.frame.take().unwrap().1;
    let original = captured.metadata.clone();
    for id in 2..100 {
        let mut wanted = request(id as u16);
        wanted.settings.mode = if id % 2 == 0 {
            crate::controls::Mode::Full
        } else {
            crate::controls::Mode::FullFieldSkip
        };
        e.request(id, Some(wanted));
    }
    assert_eq!(e.snapshot.state, State::SettingsPending);
    assert!(!e.accepts(1));
    e.tick(); // Waiting; old child still owns device.
    assert_eq!(*opens.lock().unwrap(), vec![100]);
    e.tick(); // Stale old frame must not become visible.
    assert!(e.snapshot.frame.is_none());
    e.tick(); // Done; never open in same tick.
    assert_eq!(*opens.lock().unwrap(), vec![100]);
    e.tick();
    e.tick();
    assert_eq!(*opens.lock().unwrap(), vec![100, 99]);
    assert!(!e.accepts(1));
    assert!(e.accepts(99));
    assert_eq!(
        e.active.as_ref().unwrap().request.settings.exposure_rows,
        99
    );
    assert!(e.snapshot.applied.is_none()); // Simulation never claims hardware application.
    assert_eq!(
        e.active.as_ref().unwrap().request.settings.mode,
        crate::controls::Mode::FullFieldSkip
    );
    assert_eq!(captured.metadata, original); // Frozen capture remains original.
}
#[test]
fn every_profile_replacement_rejects_stale_owner_frame() {
    for mode in crate::controls::Mode::ALL {
        let (mut e, opens) = engine(vec![
            vec![
                Ok(Event::Frame(frame())),
                Ok(Event::Frame(frame())),
                Ok(Event::Done { clean: true }),
            ],
            vec![Ok(Event::Waiting)],
        ]);
        e.request(1, Some(request(100)));
        e.tick();
        e.tick();
        let retained = e.snapshot.frame.take().unwrap().1;
        let original = retained.metadata.clone();
        let mut next = request(101);
        next.settings.mode = mode;
        e.request(2, Some(next));
        e.tick();
        assert!(e.snapshot.frame.is_none());
        assert!(!e.accepts(1));
        assert_eq!(*opens.lock().unwrap(), vec![100]);
        e.tick();
        e.tick();
        assert_eq!(e.active.as_ref().unwrap().request.settings.mode, mode);
        assert_eq!(retained.metadata, original);
    }
}
#[test]
fn disconnect_cancels_pending_open_and_can_reconnect_after_cleanup() {
    let (mut e, opens) = engine(vec![
        vec![Ok(Event::Waiting), Ok(Event::Done { clean: true })],
        vec![Ok(Event::Frame(frame()))],
    ]);
    e.request(1, Some(request(100)));
    e.tick();
    e.request(2, Some(request(50)));
    e.request(3, None);
    e.tick();
    e.tick();
    e.tick();
    assert_eq!(e.snapshot.state, State::Disconnected);
    assert_eq!(*opens.lock().unwrap(), vec![100]);
    e.request(4, Some(request(30)));
    e.tick();
    e.tick();
    assert!(e.accepts(4));
    assert_eq!(*opens.lock().unwrap(), vec![100, 30]);
}
#[test]
fn process_failure_preserves_owner_until_reaped_and_blocks_unproved_cleanup() {
    for clean in [true, false] {
        let (mut e, opens) = engine(vec![
            vec![
                Err("process/USB failure".into()),
                Ok(Event::Waiting),
                Ok(Event::Done { clean }),
            ],
            vec![],
        ]);
        e.request(1, Some(request(100)));
        e.tick();
        e.tick();
        assert_eq!(e.snapshot.state, State::Error);
        assert!(e.snapshot.active);
        e.request(2, Some(request(50)));
        e.tick();
        assert_eq!(*opens.lock().unwrap(), vec![100]);
        e.tick();
        e.tick();
        if clean {
            assert_eq!(*opens.lock().unwrap(), vec![100, 50]);
        } else {
            assert_eq!(e.snapshot.state, State::Error);
            e.request(3, Some(request(25)));
            e.tick();
            assert_eq!(*opens.lock().unwrap(), vec![100]);
        }
    }
}
#[test]
fn stop_is_sent_once_and_worker_close_waits_without_blocking_caller() {
    struct SlowFactory {
        live: Arc<Mutex<usize>>,
        stops: Arc<Mutex<usize>>,
    }
    struct Slow {
        live: Arc<Mutex<usize>>,
        stops: Arc<Mutex<usize>>,
        remaining: u8,
    }
    impl Factory for SlowFactory {
        fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
            let mut live = self.live.lock().unwrap();
            assert_eq!(*live, 0);
            *live = 1;
            Ok(Box::new(Slow {
                live: self.live.clone(),
                stops: self.stops.clone(),
                remaining: 4,
            }))
        }
    }
    impl Drop for Slow {
        fn drop(&mut self) {
            *self.live.lock().unwrap() = 0;
        }
    }
    impl Transport for Slow {
        fn stop(&mut self) -> Result<(), String> {
            *self.stops.lock().unwrap() += 1;
            Ok(())
        }
        fn poll(&mut self) -> Result<Event, String> {
            if *self.stops.lock().unwrap() == 0 {
                return Ok(Event::Waiting);
            }
            if self.remaining > 0 {
                self.remaining -= 1;
                Ok(Event::Waiting)
            } else {
                Ok(Event::Done { clean: true })
            }
        }
    }
    let live = Arc::new(Mutex::new(0));
    let stops = Arc::new(Mutex::new(0));
    let w = Worker::with_factory(SlowFactory {
        live: live.clone(),
        stops: stops.clone(),
    });
    w.request(Some(request(100))).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while *live.lock().unwrap() == 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(*live.lock().unwrap(), 1);
    w.close();
    assert!(!w.closed());
    assert!(w.request(Some(request(50))).is_err());
    while !w.closed() && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(w.closed());
    assert_eq!(*live.lock().unwrap(), 0);
    assert_eq!(*stops.lock().unwrap(), 1);
}
#[test]
fn real_adapter_cancel_exit_marker_reap_preserves_latest_or_blocks() {
    struct ChildFactory {
        root: std::path::PathBuf,
        opens: Arc<Mutex<usize>>,
        unclean: bool,
    }
    impl Factory for ChildFactory {
        fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
            let mut opens = self.opens.lock().unwrap();
            *opens += 1;
            if *opens > 1 {
                return Ok(Box::new(Sim {
                    simulation: Simulation { sequence: 0 },
                    stopped: false,
                }));
            }
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "physical_preview::protocol_tests::child",
                    "--nocapture",
                ])
                .env("CAMERA_LAB_PROTOCOL_TEST", &self.root);
            if self.unclean {
                command.env("CAMERA_LAB_NO_CLEANUP", "1");
            }
            Ok(Box::new(Physical {
                preview: PhysicalPreview::test_child(command.spawn().unwrap(), self.root.clone()),
                error: None,
                progress: None,
            }))
        }
    }
    for (index, unclean, disconnect) in [(0, false, false), (1, true, false), (2, false, true)] {
        let root =
            std::env::temp_dir().join(format!("adapter-cancel-{}-{index}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let opens = Arc::new(Mutex::new(0));
        let mut e = Engine::new(ChildFactory {
            root: root.clone(),
            opens: opens.clone(),
            unclean,
        });
        e.request(1, Some(request(100)));
        e.tick();
        for id in 2..30 {
            e.request(id, Some(request(id as u16)));
        }
        if disconnect {
            e.request(30, None);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while e.snapshot.active && Instant::now() < deadline {
            e.tick();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!e.snapshot.active);
        assert_eq!(*opens.lock().unwrap(), 1, "no replacement on reap tick");
        e.tick();
        assert_eq!(
            *opens.lock().unwrap(),
            if unclean || disconnect { 1 } else { 2 }
        );
        if unclean {
            assert!(e.blocked);
            assert_eq!(e.snapshot.state, State::Error);
        } else {
            assert_ne!(e.snapshot.state, State::Error);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn panicked_worker_is_terminal_and_rejects_new_requests() {
    struct Panics;
    impl Factory for Panics {
        fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
            panic!("injected acquisition failure");
        }
    }
    let w = Worker::with_factory(Panics);
    w.request(Some(request(100))).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !w.closed() && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(w.closed());
    assert_eq!(w.snapshot().state, State::Error);
    assert!(w.request(Some(request(50))).is_err());
}
