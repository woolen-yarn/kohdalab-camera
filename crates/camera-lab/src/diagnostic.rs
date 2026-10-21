//! Pure production diagnostic processing. No IO or release authority.
use serde::Serialize;
use std::io;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Origin { Backend, Parent, Helper, Unknown }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Stage { Identity, Open, Descriptor, Claim, Sensor, Init, Stream, Cleanup, Clock, Initialization, Owner, Shutdown, Channel, Terminal, Supervisor, Unknown }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Reason { OperationFailed, Deadline, Unresponsive, Lost, Malformed, SupervisorFault, Unknown }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Cause { pub origin:Origin, pub stage:Stage, pub reason:Reason, pub category:Option<u32> }
impl Cause {
 pub const fn parent(stage:Stage,reason:Reason)->Self {Self{origin:Origin::Parent,stage,reason,category:None}}
 pub const fn unknown()->Self {Self{origin:Origin::Unknown,stage:Stage::Unknown,reason:Reason::Unknown,category:None}}
}
/// Versioned fixed sensor vocabulary. Category is meaningful only for Backend/Sensor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SensorFailure { ReadTransport=1, ReadLength=2, ReadStatus=3, IdentityMismatch=4, WriteTransport=5, WriteStatus=6, ReadbackMismatch=7 }
impl SensorFailure {
 pub fn text(self)->&'static str {match self {
 Self::ReadTransport=>"@SN1 read transport", Self::ReadLength=>"@SN2 read length",
 Self::ReadStatus=>"@SN3 read status", Self::IdentityMismatch=>"@SN4 sensor identity mismatch",
 Self::WriteTransport=>"@SN5 write transport", Self::WriteStatus=>"@SN6 write status",
 Self::ReadbackMismatch=>"@SN7 readback mismatch"}}
 fn from_category(n:u32)->Option<Self> {Some(match n {1=>Self::ReadTransport,2=>Self::ReadLength,3=>Self::ReadStatus,4=>Self::IdentityMismatch,5=>Self::WriteTransport,6=>Self::WriteStatus,7=>Self::ReadbackMismatch,_=>return None})}
}
/// Fixed vocabulary only; arbitrary diagnostic prose never participates.
pub fn summary(cause: Option<Cause>) -> String {
 let c=cause.unwrap_or(Cause::unknown());
 let reason=match c.reason {Reason::OperationFailed=>"Operation failed",Reason::Deadline=>"Deadline exceeded",Reason::Unresponsive=>"Unresponsive",Reason::Lost=>"Lost",Reason::Malformed=>"Malformed",Reason::SupervisorFault=>"Supervisor fault",Reason::Unknown=>"Unknown"};
 let detail=if c.origin==Origin::Backend && c.stage==Stage::Sensor {c.category.and_then(SensorFailure::from_category).map(|v|v.text().split_once(' ').unwrap().1)}else{None};
 format!("{:?} / {:?} / {}{}",c.origin,c.stage,reason,detail.map(|v|format!(" / {v}")).unwrap_or_default())
}
#[derive(Clone, Copy, Debug)]
pub enum ParentReport { OwnerUnresponsive, ChallengeLost, ChallengeMalformed, ChallengeUnanswered, ChallengeStalled }
impl ParentReport {
 pub fn cause(self)->Cause { match self {
 Self::ChallengeLost=>Cause::parent(Stage::Channel,Reason::Lost),
 Self::ChallengeMalformed=>Cause::parent(Stage::Channel,Reason::Malformed),
 _=>Cause::parent(Stage::Owner,Reason::Unresponsive) } }
 pub fn text(self)->&'static str {match self {
 Self::OwnerUnresponsive=>"Lifecycle owner unresponsive",Self::ChallengeLost=>"Parent challenge channel lost",
 Self::ChallengeMalformed=>"Invalid helper challenge",Self::ChallengeUnanswered=>"Lifecycle owner challenge unanswered",
 Self::ChallengeStalled=>"Lifecycle owner stalled during challenge"}}
}
/// Minimal report mutation shared by native supervisor/evaluate and host tests.
pub fn report_fault(acc:&mut Accumulator, first:&mut Option<String>, cause:Cause, text:impl Into<String>) {
 acc.retain(cause); if first.is_none(){*first=Some(text.into());}
}
pub fn report_parent(acc:&mut Accumulator, first:&mut Option<String>, report:ParentReport) {
 report_fault(acc,first,report.cause(),report.text());
}
/// Actual native terminal reader report order; backend precedes helper, prior cause wins.
pub fn report_terminal(acc:&mut Accumulator,first:&mut Option<String>,packet:&[u8;128])->io::Result<()> {
 let text=match acc.terminal(packet) {Ok(v)=>v,Err(e)=>{
 report_fault(acc,first,Cause::parent(Stage::Terminal,Reason::Malformed),"Malformed terminal");return Err(e)}};
 if let Some(text)=text {report_fault(acc,first,backend_code(&text),format!("Backend: {text}"));}
 if let Some(f)=decode_fault(packet[69]) {report_fault(acc,first,Cause{origin:Origin::Helper,stage:Stage::Supervisor,reason:Reason::SupervisorFault,category:Some(packet[69].into())},format!("Helper supervisor: {f:?}"));}
 Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Terminal { CleanCancellation, FailedClean, Unproved }
// Prefixes are emitted only at explicit backend operation sites. No prose inference.
pub fn backend_code(text:&str)->Cause {
 let token=text.split_once(' ').map(|v|v.0);
 let category=match token {Some("@SN1")=>Some(1),Some("@SN2")=>Some(2),Some("@SN3")=>Some(3),Some("@SN4")=>Some(4),Some("@SN5")=>Some(5),Some("@SN6")=>Some(6),Some("@SN7")=>Some(7),_=>None};
 if category.is_some() {return Cause{origin:Origin::Backend,stage:Stage::Sensor,reason:Reason::OperationFailed,category};}
 let stage=match token {
 Some("@ID")=>Stage::Identity,Some("@OP")=>Stage::Open,Some("@DS")=>Stage::Descriptor,
 Some("@CL")=>Stage::Claim,Some("@SN")=>Stage::Sensor,Some("@IN")=>Stage::Init,
 Some("@ST")=>Stage::Stream,Some("@CU")=>Stage::Cleanup,_=>return Cause::unknown()};
 Cause{origin:Origin::Backend,stage,reason:Reason::OperationFailed,category:None}
}
pub fn backend_text(code:&str,error:String)->String {
 if error==crate::physical::CANCELLED || backend_code(&error).origin==Origin::Backend {error} else {format!("{code} {error}")}
}
#[derive(Clone, Debug, Default)]
pub struct Accumulator {pub first:Option<Cause>,pub backend:Option<Cause>,pub helper:Option<crate::continuous::Fault>,pub proof:Option<u8>,pub text:Option<String>}
impl Accumulator {
 pub fn retain(&mut self,cause:Cause) {if self.first.is_none(){self.first=Some(cause);}}
 pub fn terminal(&mut self,packet:&[u8;128])->std::io::Result<Option<String>> {
  let text=match terminal_error(packet) {Ok(v)=>v,Err(e)=>{self.retain(Cause::parent(Stage::Terminal,Reason::Malformed));return Err(e)}};
  if let Some(helper)=decode_fault(packet[69]) {self.helper=Some(helper);}self.proof=Some(packet[68]);if let Some(text)=text {self.text=Some(text.to_owned());}
  // Backend operation precedes the terminal supervisor observation. Existing parent cause wins.
  if let Some(text)=text {let cause=backend_code(text);self.backend=Some(cause);self.retain(cause);}
  if self.helper.is_some() {self.retain(Cause{origin:Origin::Helper,stage:Stage::Supervisor,reason:Reason::SupervisorFault,category:Some(u32::from(packet[69]))});}
  Ok(text.map(str::to_owned))
 }
}
pub fn decode_fault(code: u8) -> Option<crate::continuous::Fault> {
    use crate::continuous::Fault::*;
    [
        Protocol,
        Clock,
        Lease,
        Initialization,
        NoTransfer,
        NoFrame,
        OwnerUnresponsive,
        Drain,
        Cleanup,
        Receipt,
        Exit,
        Reap,
        Unlock,
    ]
    .get(code.wrapping_sub(1) as usize)
    .copied()
}
pub fn terminal_error(packet: &[u8; 128]) -> io::Result<Option<&str>> {
    let n = packet[70] as usize;
    if n > 57
        || packet[71 + n..].iter().any(|b| *b != 0)
        || packet[69] > 13
        || packet[68] & !31 != 0
        || (packet[68] & 8 != 0
            && (packet[68] & 7 != 7 || packet[68] & 16 != 0 || packet[69] != 0 || n != 0))
    {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
    }
    let text = std::str::from_utf8(&packet[71..71 + n]).map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
    Ok(if text.is_empty() { None } else { Some(text) })
}
pub fn write_terminal_error(packet: &mut [u8; 128], error: Option<&str>) {
    if let Some(error) = error {
        let mut n = error.len().min(57);
        while !error.is_char_boundary(n) {
            n -= 1;
        }
        packet[70] = n as u8;
        packet[71..71 + n].copy_from_slice(&error.as_bytes()[..n]);
    }
}

/// Inert observations supplied by the native owner, not cleanup authorization.
#[derive(Clone,Default)]
pub struct Facts {
 pub starting:bool,pub phase:u8,pub stop_observed:bool,pub release_committed:bool,
 pub marker_deleted:bool,pub collected:bool,pub exit:Option<u32>,pub proof_flags:u8,
 pub terminal:Option<Terminal>,pub helper_fault:Option<crate::continuous::Fault>,
 pub backend_error:Option<String>,pub parent_error:Option<String>,
}
/// Production projection preserves exact observations, including all typed terminal values.
pub fn lifecycle(facts:&Facts, causes:&Accumulator)->crate::session::Lifecycle {
 crate::session::Lifecycle {
  starting:facts.starting,phase:facts.phase,stop_observed:facts.stop_observed,
  release_committed:facts.release_committed,marker_deleted:facts.marker_deleted,
  collected:facts.collected,exit:facts.exit,proof_flags:causes.proof.unwrap_or(facts.proof_flags),
  unproved:facts.terminal==Some(Terminal::Unproved),terminal_kind:facts.terminal,
  helper_fault:causes.helper.or(facts.helper_fault),
  backend_error:causes.text.clone().or_else(||facts.backend_error.clone()),parent_error:facts.parent_error.clone(),
  first_cause:causes.first,backend_cause:causes.backend,
 }
}
/// Shared production transport error/collection dispatch. Native frame handling follows this.
pub fn transport_event(l:&crate::session::Lifecycle,reported:&mut bool,label:&str)->Option<crate::session::Event> {
 if !*reported && (l.parent_error.is_some() || l.unproved) {
  *reported=true;
  return Some(crate::session::Event::Lifecycle{message:crate::session::continuous_failure_message(l.parent_error.as_deref().unwrap_or("Cleanup unproved"),label),unproved:true});
 }
 if l.collected {return Some(crate::session::Event::Done{clean:l.exit==Some(0)&&l.marker_deleted&&l.release_committed&&l.terminal_kind==Some(Terminal::CleanCancellation)});}
 None
}

#[cfg(test)] mod tests {
 use super::*;
 #[test] fn v036_codec_validation_allowlist_and_immutable_unknown() {
  let codes=[("@ID x",Stage::Identity),("@OP x",Stage::Open),("@DS x",Stage::Descriptor),("@CL x",Stage::Claim),("@SN x",Stage::Sensor),("@IN x",Stage::Init),("@ST x",Stage::Stream),("@CU x",Stage::Cleanup)];
  for (code,stage) in codes {assert_eq!(backend_code(code).stage,stage);}
  for unknown in ["", "@OP", "@OPX path", "open failed", "@OP\npath"] {assert_eq!(backend_code(unknown),Cause::unknown());}
  let mut p=[0;128];write_terminal_error(&mut p,Some(&"é".repeat(40)));assert_eq!(terminal_error(&p).unwrap().unwrap().len(),56);
  let mut acc=Accumulator::default();acc.terminal(&p).unwrap();let first=acc.first;
  let mut later=[0;128];later[69]=4;acc.terminal(&later).unwrap();assert_eq!(acc.first,first);assert_eq!(acc.helper,Some(crate::continuous::Fault::Initialization));
  for (offset,value) in [(70,58),(69,14),(68,32),(127,1),(68,8)] {
   let mut p=[0;128];p[offset]=value;let mut acc=Accumulator::default();assert!(acc.terminal(&p).is_err());assert_eq!(acc.first,Some(Cause::parent(Stage::Terminal,Reason::Malformed)));
  }
  let mut p=[0;128];p[70]=1;p[71]=255;assert!(terminal_error(&p).is_err());
  let mut p=[0;128];p[68]=15;assert!(terminal_error(&p).is_ok());
  assert_eq!(backend_text("@IN",crate::physical::CANCELLED.into()),crate::physical::CANCELLED);
  assert_eq!(backend_text("@IN","@ST original".into()),"@ST original");
 }
}

#[cfg(test)] mod v037_tests {
 use super::*;
 #[test] fn v037_parent_report_races_and_first_unknown() {
  for r in [ParentReport::OwnerUnresponsive,ParentReport::ChallengeLost,ParentReport::ChallengeMalformed,ParentReport::ChallengeUnanswered,ParentReport::ChallengeStalled] {
   for reverse in [false,true] {
    let mut a=Accumulator::default();let mut first=None;
    let reports=if reverse {[ParentReport::OwnerUnresponsive,r]}else{[r,ParentReport::OwnerUnresponsive]};
    for report in reports {report_parent(&mut a,&mut first,report);}
    let expected=reports[0].cause();assert_eq!(a.first,Some(expected));
    let mut p=[0;128];p[69]=4;report_terminal(&mut a,&mut first,&p).unwrap();assert_eq!(a.first,Some(expected));assert_eq!(first.as_deref(),Some(reports[0].text()));
   }
  }
  let mut a=Accumulator::default();let mut first=None;report_fault(&mut a,&mut first,Cause::unknown(),"raw path");report_parent(&mut a,&mut first,ParentReport::OwnerUnresponsive);assert_eq!(a.first,Some(Cause::unknown()));assert_eq!(summary(a.first),"Unknown / Unknown / Unknown");
 }
}

#[cfg(test)] mod v039_tests {
 use super::*;
 #[test] fn v039_sensor_terminal_classifier_record() {
  use SensorFailure::*;
  for (i,kind) in [ReadTransport,ReadLength,ReadStatus,IdentityMismatch,WriteTransport,WriteStatus,ReadbackMismatch].into_iter().enumerate() {
   let error=backend_text("@IN",kind.text().into());assert_eq!(error,kind.text());
   let mut packet=[0;128];packet[68]=21;
   write_terminal_error(&mut packet,Some(&format!("{error} {}", "PRIVATE/path/é".repeat(20))));
   assert!(packet[70]<=57);let mut a=Accumulator::default();let mut first=None;report_terminal(&mut a,&mut first,&packet).unwrap();
   let cause=a.backend.unwrap();assert_eq!(cause.category,Some(i as u32+1));assert_eq!(cause.stage,Stage::Sensor);
   assert!(summary(Some(cause)).ends_with(kind.text().split_once(' ').unwrap().1));
   let mut s=crate::session::Snapshot::default();s.first_failure=first;s.first_cause=a.first;s.lifecycle=Some(lifecycle(&Facts::default(),&a));
   let bytes=crate::failure_record::record(&s,"collection.json");let raw=String::from_utf8(bytes).unwrap();assert!(!raw.contains("PRIVATE"));
   let value:serde_json::Value=serde_json::from_str(&raw).unwrap();assert_eq!(value["backend_cause"],serde_json::json!(cause));
  }
  for text in ["@SN generic", "@SN read transport timeout", "@SN8 bad", "@SN1", "@SN01 bad", "@SN1x bad", "@SN1\ntext", "sensor read length failed"] {
   let mut p=[0;128];write_terminal_error(&mut p,Some(text));let mut a=Accumulator::default();report_terminal(&mut a,&mut None,&p).unwrap();
   assert_eq!(a.backend.unwrap().category,None);
   assert_eq!(a.backend.unwrap().stage,if text.starts_with("@SN "){Stage::Sensor}else{Stage::Unknown});
  }
  assert_eq!(backend_text("@SN1",crate::physical::CANCELLED.into()),crate::physical::CANCELLED);
  let mut p=[0;128];p[68]=15;let mut a=Accumulator::default();report_terminal(&mut a,&mut None,&p).unwrap();assert_eq!(a.backend,None);
 }
}
