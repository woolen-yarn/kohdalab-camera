//! Shared production initialization recipe. One execution precedes Live.
//! Values and order are extracted from the v013 production trial, not timings
//! from the public performance study. Legacy and continuous owners use this API.
use crate::{controls::Settings, progress::Stage};
use std::time::Duration;

pub trait Backend {
    fn checkpoint(&mut self) -> Result<(), String>;
    fn write(&mut self, address: u16, value: u16) -> Result<(), String>;
    fn checked(&mut self, address: u16, value: u16) -> Result<(), String>;
    fn read(&mut self, address: u16) -> Result<u16, String>;
    fn bridge(&mut self, value: u16) -> Result<(), String>;
    fn settle(&mut self, delay: Duration) -> Result<(), String>;
    fn progress(&mut self, stage: Stage);
    /// Implementations must retain the complete existing framing, known-pattern,
    /// raw, lossless decode-equality and metadata assertions.
    fn pattern(&mut self, index: usize, name: &str, duration: Duration) -> Result<(), String>;
}

/// The legacy CLI performs this same bridge preparation before its finite
/// trial. Continuous owns it inside the guarded cleanup lifetime, once only.
pub fn initialize_continuous(b: &mut impl Backend, settings: Settings) -> Result<(), String> {
    initialize_continuous_intent(b, settings, crate::startup::Intent::Diagnostics)
}

pub fn initialize_continuous_intent(b: &mut impl Backend, settings: Settings, intent: crate::startup::Intent) -> Result<(), String> {
    settings.validate()?;
    prepare_continuous_transport(b, &mut false)?;
    initialize_intent(b, settings, intent)
}

/// Marks an attempted vendor operation before dispatch, including failed transfers.
pub fn prepare_continuous_transport(b: &mut impl Backend, attempted: &mut bool) -> Result<(), String> {
    for value in [1, 0, 1] {
        b.checkpoint()?;
        *attempted = true;
        b.bridge(value)?;
    }
    Ok(())
}

/// Observed by the finite CLI before dispatch, including failed transfers.
/// Completion is independent of attempt; an external identity read is vendor I/O.
#[derive(Default, Clone, Debug)]
pub struct CallerPreparation {
    pub bridge_attempts: usize,
    pub bridge_completed: usize,
    pub identity_attempted: bool,
    pub failure: Option<String>,
    pub stop_attempted: bool,
}
impl CallerPreparation {
    pub fn attempted(&self) -> bool { self.bridge_attempts != 0 || self.identity_attempted }
    pub fn dispatch_bridge(&mut self, operation: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
        self.bridge_attempts += 1;
        let result = operation().map_err(|e| crate::diagnostic::backend_text("@IN", e));
        if result.is_ok() { self.bridge_completed += 1; }
        else { self.failure = result.as_ref().err().cloned(); }
        result
    }
    pub fn dispatch_identity<T>(&mut self, operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        self.identity_attempted = true;
        let result = operation();
        if let Err(error) = &result { self.failure = Some(error.clone()); }
        result
    }
}

/// Physical startup uses this state inside its cleanup lifetime. Other callers
/// retain initialize_continuous_intent's existing prepared initialization API.
#[derive(Default)]
pub(crate) struct IdentityGate {
    pub attempted: bool,
    pub verified: bool,
    stop_attempted: bool,
}
impl IdentityGate {
    pub fn start(&mut self, b: &mut impl Backend, continuous: bool, caller: &CallerPreparation) -> Result<(), String> {
        if continuous {
            prepare_continuous_transport(b, &mut self.attempted)
                .map_err(|e| crate::diagnostic::backend_text("@IN", e))?;
        } else {
            self.attempted = caller.attempted();
            self.stop_attempted = caller.stop_attempted;
            if let Some(error) = &caller.failure { return Err(error.clone()); }
        }
        b.checkpoint()?;
        self.attempted = true;
        if b.read(0).map_err(|e| crate::diagnostic::backend_text("@SN", e))? != 0x1621 {
            return Err(crate::diagnostic::SensorFailure::IdentityMismatch.text().into());
        }
        self.verified = true;
        Ok(())
    }
    /// Best effort STOP cannot qualify cleanup or replace the first cause.
    pub fn finish_unverified(&self, b: &mut impl Backend, first: Result<(), String>) -> Result<(), String> {
        if self.attempted && !self.verified && !self.stop_attempted {
            let _ = b.bridge(1);
        }
        first
    }
}

pub fn initialize(b: &mut impl Backend, settings: Settings) -> Result<(), String> {
    initialize_intent(b, settings, crate::startup::Intent::Diagnostics)
}

pub fn initialize_intent(b: &mut impl Backend, settings: Settings, intent: crate::startup::Intent) -> Result<(), String> {
    settings.validate()?;
    b.progress(Stage::Configuration);
    // Datasheet pp16,31: explicit reset then release; 20 ms engineering allowance.
    b.write(0x0d, 1)?;
    b.settle(Duration::from_millis(20))?;
    b.checked(0x0d, 0)?;
    b.checked(7, 0)?; // p15 chip disable during configuration.
    // p12 reset defaults, p15 window size = count - 1; fixed even origins.
    for (address, value) in settings.mode.profile().window() {
        b.checked(address, value)?;
    }
    b.checked(5, 142)?;
    b.checked(6, 25)?;
    b.checked(8, 0)?;
    b.checked(9, 100)?; // p15 exposure in rows, engineering choice.
    b.checked(0x0a, settings.clock())?; // p16 opposite sampling edge; controlled pattern-corruption hypothesis.
    b.checked(0x1e, 0x8040)?; // p12 recommended fixed-bit value, continuous.
    b.checked(0x20, 0x2000)?; // reset default, suppress bad frames.
    b.checked(0x21, 0)?;
    let address_modes = crate::sensor_contract::Geometry::for_mode(settings.mode)?.address_modes;
    b.checked(0x22, address_modes[0])?;
    b.checked(0x23, address_modes[1])?;
    b.checked(0x35, 8)?; // p17 unity gain.
    if intent == crate::startup::Intent::Diagnostics {
    b.checked(0x32, 0x0554)?; // p17 alternating 10-bit 155 / 2aa.
    b.checked(7, 0x42)?; // chip enable + test data, p15.
    b.checkpoint()?;
    b.progress(crate::progress::Stage::Pattern155);
    b.pattern(0, "pattern-155", Duration::from_secs(5))?;
    b.checkpoint()?;
    b.bridge(1)?;
    b.checked(7, 0)?;
    // Asymmetric 10-bit value distinguishes the wired eight-bit slice.
    // p17 test data 0x123 << 2; upper eight => 48/b7, low eight => 23/dc.
    b.checked(0x32, 0x048c)?;
    b.checked(7, 0x42)?;
    b.checkpoint()?;
    b.progress(crate::progress::Stage::Pattern123);
    b.pattern(1, "pattern-123", Duration::from_secs(5))?;
    b.checkpoint()?;
    b.bridge(1)?;
    b.checked(7, 0)?;
    // A second documented pattern disambiguates bit significance. No timing sweep.
    b.checked(0x32, 0)?;
    b.checked(7, 0x42)?;
    b.checkpoint()?;
    b.progress(crate::progress::Stage::PatternZero);
    b.pattern(2, "pattern-zero", Duration::from_secs(5))?;
    b.checkpoint()?;
    b.bridge(1)?;
    b.checked(7, 0)?;
    b.progress(crate::progress::Stage::Controls);
    // p15 integration in rows; p17 unity / 2x analog gain; restore each.
    b.checked(9, 50)?;
    b.checked(9, 100)?;
    b.checked(0x35, 16)?;
    for address in 0x2b..=0x2e {
        b.checkpoint()?;
        if b.read(address)? != 16 {
            return Err("Per-channel gain readback failed".into());
        }
    }
    b.checked(0x35, 8)?;
    }
    // Requested values apply after unchanged pattern/control checks.
    b.checked(8, 0)?;
    b.checked(9, settings.exposure_rows)?;
    b.checked(0x35, settings.gain_eighths)?;
    for address in 0x2b..=0x2e {
        b.checkpoint()?;
        if b.read(address)? != settings.gain_eighths {
            return Err("Requested per-channel gain readback failed".into());
        }
    }
    b.checked(7, 2)?; // return to live sensor data, p15.
    b.checkpoint()?;
    b.progress(crate::progress::Stage::Live);
    Ok(())
}

#[cfg(test)]
mod v040_tests {
    use super::*;
    use crate::diagnostic::SensorFailure as SF;
    #[derive(Default)]
    struct Recording {
        ops: Vec<String>, checkpoints: usize, cancel_at: Option<usize>,
        bridge_calls: usize, fail_bridge: Option<usize>, identity_error: Option<String>,
        mismatch: bool, gain: u16, identity_reply: Option<Vec<u8>>,
    }
    impl Backend for Recording {
        fn checkpoint(&mut self)->Result<(),String>{
            let n=self.checkpoints; self.checkpoints+=1;
            if self.cancel_at==Some(n){Err(crate::physical::CANCELLED.into())}else{Ok(())}
        }
        fn bridge(&mut self,v:u16)->Result<(),String>{
            let n=self.bridge_calls;self.bridge_calls+=1;self.ops.push(format!("B{v}"));
            if self.fail_bridge==Some(n){Err("@IN preparation transport".into())}else{Ok(())}
        }
        fn read(&mut self,a:u16)->Result<u16,String>{
            self.ops.push(format!("R{a}"));
            if a==0 {if let Some(bytes)=&self.identity_reply{return crate::physical::validate_sensor_read(bytes)} if let Some(e)=&self.identity_error{return Err(e.clone())}return Ok(if self.mismatch{0}else{0x1621})}
            Ok(self.gain)
        }
        fn write(&mut self,a:u16,v:u16)->Result<(),String>{self.ops.push(format!("W{a}:{v}"));Ok(())}
        fn checked(&mut self,a:u16,v:u16)->Result<(),String>{if a==0x35{self.gain=v} self.write(a,v)}
        fn settle(&mut self,d:Duration)->Result<(),String>{assert_eq!(d,Duration::from_millis(20));self.ops.push("settle20".into());Ok(())}
        fn progress(&mut self,_:Stage){}
        fn pattern(&mut self,i:usize,_:&str,d:Duration)->Result<(),String>{assert_eq!(d,Duration::from_secs(5));self.ops.push(format!("P{i}"));Ok(())}
    }
    #[test]
    fn v040_prepare_identity_then_existing_recipe_exactly_once() {
        for intent in [crate::startup::Intent::Normal,crate::startup::Intent::Diagnostics] {
            let settings=Settings{exposure_rows:73,gain_eighths:24,..Default::default()};
            let mut b=Recording::default();let mut gate=IdentityGate::default();
            gate.start(&mut b,true,&CallerPreparation::default()).unwrap();assert!(gate.verified);
            initialize_intent(&mut b,settings,intent).unwrap();
            assert_eq!(&b.ops[..4], &["B1","B0","B1","R0"]);
            assert_eq!(b.ops.iter().filter(|s|s.as_str()=="B0").count(),1);
            let mut old=Recording::default();initialize_continuous_intent(&mut old,settings,intent).unwrap();
            let mut expected=old.ops;expected.insert(3,"R0".into());assert_eq!(b.ops,expected);
            assert!(b.ops.ends_with(&["W7:2".into()]));
            assert!(b.ops.contains(&"W9:73".into()) && b.ops.contains(&"W53:24".into()));
            assert_eq!(b.ops.iter().filter(|s|s.starts_with('P')).count(),if intent==crate::startup::Intent::Diagnostics{3}else{0});
            let before=b.ops.clone();gate.finish_unverified(&mut b, Ok(())).unwrap();assert_eq!(before,b.ops);
            // Finite caller already prepared: this production gate adds no preamble.
            let mut finite=Recording::default();IdentityGate::default().start(&mut finite,false,&CallerPreparation::default()).unwrap();assert_eq!(finite.ops,vec!["R0"]);
        }
    }
    #[test]
    fn v040_identity_failures_preserve_typed_first_cause_stop_only() {
        for error in [SF::ReadTransport,SF::ReadLength,SF::ReadStatus,SF::IdentityMismatch] {
            let mut b=Recording{identity_error:Some(error.text().into()),fail_bridge:Some(3),..Default::default()};
            let mut gate=IdentityGate::default();let result=gate.start(&mut b,true,&CallerPreparation::default());
            let result=gate.finish_unverified(&mut b,result);
            assert_eq!(result.unwrap_err(),error.text());assert!(!gate.verified);
            assert_eq!(b.ops,vec!["B1","B0","B1","R0","B1"]);
        }
        for (reply,error) in [(vec![],SF::ReadLength),(vec![8,0x16],SF::ReadLength),(vec![0,0x16,0x21],SF::ReadStatus),(vec![8,0x16,0x21,0],SF::ReadLength)] {
            let mut b=Recording{identity_reply:Some(reply),..Default::default()};let mut gate=IdentityGate::default();
            let result=gate.start(&mut b,true,&CallerPreparation::default());let result=gate.finish_unverified(&mut b,result);
            assert_eq!(result.unwrap_err(),error.text());assert!(!gate.verified);
            assert_eq!(b.ops,vec!["B1","B0","B1","R0","B1"]);
        }
        let mut b=Recording{mismatch:true,..Default::default()};let mut gate=IdentityGate::default();
        let result=gate.start(&mut b,true,&CallerPreparation::default());assert_eq!(gate.finish_unverified(&mut b,result).unwrap_err(),SF::IdentityMismatch.text());
        assert_eq!(b.ops,vec!["B1","B0","B1","R0","B1"]);
    }
    #[test]
    fn v040_cancel_and_partial_prepare_stop_only_after_attempt() {
        for cancel_at in 0..4 {
            let mut b=Recording{cancel_at:Some(cancel_at),..Default::default()};let mut gate=IdentityGate::default();
            let result=gate.start(&mut b,true,&CallerPreparation::default());let result=gate.finish_unverified(&mut b,result);
            assert_eq!(result.unwrap_err(),crate::physical::CANCELLED);assert!(!gate.verified);
            let mut expected:Vec<String>=["B1","B0","B1"][..cancel_at].iter().map(|s|s.to_string()).collect();
            if cancel_at>0 {expected.push("B1".into())}assert_eq!(b.ops,expected);
        }
        for fail_at in 0..3 {
            let mut b=Recording{fail_bridge:Some(fail_at),..Default::default()};let mut gate=IdentityGate::default();
            let result=gate.start(&mut b,true,&CallerPreparation::default());let result=gate.finish_unverified(&mut b,result);
            assert_eq!(result.unwrap_err(),"@IN preparation transport");assert!(!gate.verified);
            let mut expected:Vec<String>=["B1","B0","B1"][..=fail_at].iter().map(|s|s.to_string()).collect();expected.push("B1".into());assert_eq!(b.ops,expected);
        }
    }
}
