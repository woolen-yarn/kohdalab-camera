use super::*;
#[test]
fn actual_worker_stall_faults_lease_and_collects_without_painter() {
    use crate::continuous_session::{ContinuousSession, Fixture};
    struct FactoryOnce(std::path::PathBuf);
    struct Stall {
        inner: ContinuousTransport,
        stalled: bool,
    }
    impl Factory for FactoryOnce {
        fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
            Ok(Box::new(Stall {
                inner: ContinuousTransport::new(
                    ContinuousSession::start_fixture(&self.0, Fixture::Normal).unwrap(),
                    format!("SIMULATION evidence={}", self.0.display()),
                ),
                stalled: false,
            }))
        }
    }
    impl Transport for Stall {
        fn lifecycle(&self) -> Option<Lifecycle> {
            self.inner.lifecycle()
        }
        fn stop(&mut self) -> Result<(), String> {
            self.inner.stop()
        }
        fn poll(&mut self) -> Result<Event, String> {
            if self.inner.sequence >= 2 && !self.stalled {
                self.stalled = true;
                std::thread::sleep(Duration::from_secs(3));
            }
            self.inner.poll()
        }
    }
    let root = std::path::PathBuf::from(std::env::var_os("GUI_CONTINUOUS_FAKE_ROOT").unwrap())
        .join("worker-stall");
    std::fs::create_dir(&root).unwrap();
    let worker = Worker::with_factory(FactoryOnce(root.clone()));
    worker
        .request(Some(Request {
            startup_intent: crate::startup::Intent::Normal,
            source: Source::Simulation,
            settings: Settings::default(),
        }))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(35);
    let mut final_state = worker.snapshot();
    while !final_state.lifecycle.as_ref().is_some_and(|l| l.collected) && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(100));
        final_state = worker.snapshot();
    }
    assert!(final_state.lifecycle.as_ref().unwrap().collected);
    assert!(final_state.restart_disabled);
    assert!(final_state.message.contains("Lifecycle owner unresponsive"));
    assert!(
        worker
            .request(Some(Request {
                startup_intent: crate::startup::Intent::Normal,
                source: Source::Simulation,
                settings: Settings::default()
            }))
            .is_err()
    );
    worker.close();
    while !worker.closed() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(33));
    }
    assert!(worker.closed());
    println!(
        "SIMULATION actual Worker stall: {:?}; {}",
        final_state.lifecycle, final_state.message
    );
}
