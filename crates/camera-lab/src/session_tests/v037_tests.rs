use super::*;
use crate::diagnostic::{self as d, Cause, ParentReport as P};
struct F(usize);
struct T {
    case: usize,
    step: u8,
    a: d::Accumulator,
    f: d::Facts,
    reported: bool,
}
impl Factory for F {
    fn open(&mut self, _: &Request) -> Result<Box<dyn Transport>, String> {
        Ok(Box::new(T {
            case: self.0,
            step: 0,
            a: Default::default(),
            f: Default::default(),
            reported: false,
        }))
    }
}
impl Transport for T {
    fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn lifecycle(&self) -> Option<Lifecycle> {
        Some(d::lifecycle(&self.f, &self.a))
    }
    fn poll(&mut self) -> Result<Event, String> {
        if self.step == 0 {
            if self.case >= 6 {
                let reports = [
                    P::OwnerUnresponsive,
                    P::OwnerUnresponsive,
                    P::ChallengeLost,
                    P::ChallengeUnanswered,
                    P::ChallengeStalled,
                    P::ChallengeMalformed,
                ];
                let r = reports[self.case - 6];
                d::report_parent(&mut self.a, &mut self.f.parent_error, r);
                // The evaluate path and supervisor both use this exact owner report mutation.
                if self.case == 6 || self.case == 7 {
                    d::report_parent(&mut self.a, &mut self.f.parent_error, P::OwnerUnresponsive);
                }
            } else {
                if self.case == 3 {
                    d::report_parent(&mut self.a, &mut self.f.parent_error, P::OwnerUnresponsive);
                }
                let mut p = [0; 128];
                p[68] = 16;
                if [0, 2, 3, 5].contains(&self.case) {
                    d::write_terminal_error(
                        &mut p,
                        Some(if self.case == 5 {
                            "raw SENTINEL_BUS /Users/SENTINEL_USER"
                        } else {
                            "@OP SENTINEL_BUS /Users/SENTINEL_USER"
                        }),
                    );
                }
                if [1, 2, 3].contains(&self.case) {
                    p[69] = 4;
                }
                if self.case == 4 {
                    p[70] = 58;
                }
                let _ = d::report_terminal(&mut self.a, &mut self.f.parent_error, &p);
            }
        } else {
            let mut p = [0; 128];
            p[68] = 16;
            p[69] = 5;
            d::report_terminal(&mut self.a, &mut self.f.parent_error, &p).unwrap();
            self.f.collected = true;
            self.f.terminal = Some(d::Terminal::FailedClean);
            self.f.exit = Some(2);
        }
        self.step += 1;
        Ok(d::transport_event(
            &d::lifecycle(&self.f, &self.a),
            &mut self.reported,
            "SENTINEL_PORT",
        )
        .unwrap_or(Event::Waiting))
    }
}
#[test]
fn v037_real_reports_worker_both_records() {
    let helper = Cause {
        origin: d::Origin::Helper,
        stage: d::Stage::Supervisor,
        reason: d::Reason::SupervisorFault,
        category: Some(4),
    };
    let expected = [
        d::backend_code("@OP x"),
        helper,
        d::backend_code("@OP x"),
        P::OwnerUnresponsive.cause(),
        Cause::parent(d::Stage::Terminal, d::Reason::Malformed),
        Cause::unknown(),
        P::OwnerUnresponsive.cause(),
        P::OwnerUnresponsive.cause(),
        P::ChallengeLost.cause(),
        P::ChallengeUnanswered.cause(),
        P::ChallengeStalled.cause(),
        P::ChallengeMalformed.cause(),
    ];
    for (case, cause) in expected.into_iter().enumerate() {
        let root = std::env::temp_dir().join(format!("v037-reports-{}-{case}", std::process::id()));
        let w = Worker::with_factory_recorder(
            F(case),
            crate::failure_record::Recorder::at(root.clone()),
        );
        w.request(Some(Request {
            source: Source::Simulation,
            settings: Settings::default(),
            startup_intent: crate::startup::Intent::Normal,
        }))
        .unwrap();
        let end = Instant::now() + Duration::from_secs(5);
        while w.snapshot().first_failure.is_none() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(w.snapshot().first_cause, Some(cause));
        w.close();
        while !w.closed() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut g = w.guardian();
        g.wait();
        if [0, 2, 3].contains(&case) {
            assert_eq!(
                w.snapshot().lifecycle.as_ref().unwrap().backend_cause,
                Some(d::backend_code("@OP x"))
            );
            assert!(
                w.snapshot()
                    .lifecycle
                    .as_ref()
                    .unwrap()
                    .backend_error
                    .as_ref()
                    .unwrap()
                    .contains("SENTINEL_BUS")
            );
        }
        assert_eq!(w.snapshot().first_cause, Some(cause));
        assert_eq!(
            w.snapshot().lifecycle.unwrap().helper_fault,
            Some(crate::continuous::Fault::NoTransfer)
        );
        let mut first = None;
        for name in ["first.json", "collection.json"] {
            let raw = std::fs::read_to_string(root.join("slot-00").join(name)).unwrap();
            for text in ["SENTINEL_BUS", "SENTINEL_PORT", "SENTINEL_USER", "/Users/"] {
                assert!(!raw.contains(text));
            }
            let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(v["first_failure"]["cause"], serde_json::json!(cause));
            if let Some(ref f) = first {
                assert_eq!(&v["first_failure"], f);
            } else {
                first = Some(v["first_failure"].clone());
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
