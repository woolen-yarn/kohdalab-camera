use crate::{
    acquisition_recipe::{self, Backend},
    controls::{Mode, Settings},
    progress::Stage,
};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Write(u16, u16),
    Checked(u16, u16),
    Read(u16),
    Bridge(u16),
    Settle(Duration),
    Pattern(usize, String, Duration),
}
struct Recorded {
    ops: Vec<Op>,
    gain: u16,
    stop: usize,
    bad_gain: bool,
}
impl Recorded {
    fn record(&mut self, op: Op) -> Result<(), String> {
        self.checkpoint()?;
        self.ops.push(op);
        Ok(())
    }
}
impl Backend for Recorded {
    fn checkpoint(&mut self) -> Result<(), String> {
        if self.ops.len() >= self.stop {
            Err("injected STOP".into())
        } else {
            Ok(())
        }
    }
    fn write(&mut self, a: u16, v: u16) -> Result<(), String> {
        self.record(Op::Write(a, v))
    }
    fn checked(&mut self, a: u16, v: u16) -> Result<(), String> {
        self.record(Op::Checked(a, v))?;
        if a == 0x35 {
            self.gain = v;
        }
        Ok(())
    }
    fn read(&mut self, a: u16) -> Result<u16, String> {
        self.record(Op::Read(a))?;
        Ok(if self.bad_gain { 99 } else { self.gain })
    }
    fn bridge(&mut self, v: u16) -> Result<(), String> {
        self.record(Op::Bridge(v))
    }
    fn settle(&mut self, d: Duration) -> Result<(), String> {
        self.record(Op::Settle(d))
    }
    fn progress(&mut self, _: Stage) {}
    fn pattern(&mut self, i: usize, n: &str, d: Duration) -> Result<(), String> {
        self.record(Op::Pattern(i, n.into(), d))
    }
}
// Literal v013 register order/values. This golden is not derived from the new
// implementation at test runtime. Every profile substitutes only its existing
// window, clock and requested exposure/gain fields.
fn expected(settings: Settings) -> Vec<Op> {
    use Op::*;
    let p = settings.mode.profile();
    let mut v = vec![
        Write(0x0d, 1),
        Settle(Duration::from_millis(20)),
        Checked(0x0d, 0),
        Checked(7, 0),
        Checked(1, 20),
        Checked(2, 32),
        Checked(3, 0x05ff),
        Checked(4, 0x07ff),
        Checked(5, 142),
        Checked(6, 25),
        Checked(8, 0),
        Checked(9, 100),
        Checked(0x0a, p.clock),
        Checked(0x1e, 0x8040),
        Checked(0x20, 0x2000),
        Checked(0x21, 0),
        Checked(0x22, if settings.mode == Mode::FullFieldSkip {1} else {0}),
        Checked(0x23, if settings.mode == Mode::FullFieldSkip {1} else {0}),
        Checked(0x35, 8),
        Checked(0x32, 0x0554),
        Checked(7, 0x42),
        Pattern(0, "pattern-155".into(), Duration::from_secs(5)),
        Bridge(1),
        Checked(7, 0),
        Checked(0x32, 0x048c),
        Checked(7, 0x42),
        Pattern(1, "pattern-123".into(), Duration::from_secs(5)),
        Bridge(1),
        Checked(7, 0),
        Checked(0x32, 0),
        Checked(7, 0x42),
        Pattern(2, "pattern-zero".into(), Duration::from_secs(5)),
        Bridge(1),
        Checked(7, 0),
        Checked(9, 50),
        Checked(9, 100),
        Checked(0x35, 16),
    ];
    v.extend((0x2b..=0x2e).map(Read));
    v.extend([
        Checked(0x35, 8),
        Checked(8, 0),
        Checked(9, settings.exposure_rows),
        Checked(0x35, settings.gain_eighths),
    ]);
    v.extend((0x2b..=0x2e).map(Read));
    v.push(Checked(7, 2));
    v
}
#[test]
fn actual_shared_production_recipe_keeps_all_registers_and_three_integrity_gates() {
    for mode in Mode::ALL {
        for (exposure_rows, gain_eighths) in [(100, 8), (1, 32)] {
            let settings = Settings {
                mode,
                exposure_rows,
                gain_eighths,
            };
            let mut b = Recorded {
                ops: vec![],
                gain: 8,
                stop: usize::MAX,
                bad_gain: false,
            };
            acquisition_recipe::initialize(&mut b, settings).unwrap();
            assert_eq!(b.ops, expected(settings));
        }
    }
}
#[test]
fn stop_at_every_shared_preparation_operation_suppresses_all_later_ops() {
    let settings = Settings::default();
    let golden = expected(settings);
    for stop in 0..golden.len() {
        let mut b = Recorded {
            ops: vec![],
            gain: 8,
            stop,
            bad_gain: false,
        };
        assert_eq!(
            acquisition_recipe::initialize(&mut b, settings).unwrap_err(),
            "injected STOP"
        );
        assert_eq!(b.ops, golden[..stop]);
    }
}

// The v020 golden above is preserved. Normal removes its contiguous diagnostic
// block only: pattern setup through final diagnostic unity-gain restoration.
fn normal_expected(settings: Settings) -> Vec<Op> {
    let old = expected(settings);
    assert_eq!(old[19], Op::Checked(0x32, 0x0554));
    assert_eq!(old[41], Op::Checked(0x35, 8));
    old[..19].iter().chain(&old[42..]).cloned().collect()
}

#[test]
fn normal_exact_trace_and_stop_every_operation() {
    use crate::startup::Intent;
    for mode in Mode::ALL {
        for (exposure_rows, gain_eighths) in [(100,8),(7,24)] {
            let settings = Settings {mode,exposure_rows,gain_eighths};
            let mut golden = vec![Op::Bridge(1),Op::Bridge(0),Op::Bridge(1)];
            golden.extend(normal_expected(settings));
            for stop in 0..=golden.len() {
                let mut b = Recorded {ops:vec![],gain:8,stop,bad_gain:false};
                let r = acquisition_recipe::initialize_continuous_intent(&mut b,settings,Intent::Normal);
                assert_eq!(b.ops,golden[..stop]);
                // Final checkpoint also honors STOP after the last write.
                assert!(r.is_err());
            }
            let mut b = Recorded {ops:vec![],gain:8,stop:usize::MAX,bad_gain:false};
            acquisition_recipe::initialize_continuous_intent(&mut b,settings,Intent::Normal).unwrap();
            assert_eq!(b.ops,golden);
        }
    }
}

#[test]
fn normal_bad_requested_gain_and_settings_fail_before_live() {
    use crate::startup::Intent;
    let mut b = Recorded {ops:vec![],gain:8,stop:usize::MAX,bad_gain:true};
    assert!(acquisition_recipe::initialize_continuous_intent(&mut b,Settings::default(),Intent::Normal).unwrap_err().contains("Requested per-channel"));
    assert!(!b.ops.contains(&Op::Checked(7,2)));
    for (exposure_rows,gain_eighths) in [(0,8),(100,0),(100,99)] {
        let mut b = Recorded {ops:vec![],gain:8,stop:usize::MAX,bad_gain:false};
        assert!(acquisition_recipe::initialize_continuous_intent(&mut b,Settings {exposure_rows,gain_eighths,..Default::default()},Intent::Normal).is_err());
        assert!(b.ops.is_empty());
    }
}
