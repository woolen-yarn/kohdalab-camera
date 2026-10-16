//! Two immutable records per process; no frames, environment or device identity.
use crate::session::Snapshot;
use std::{path::PathBuf, sync::{Arc,Mutex,mpsc},thread};
const CANDIDATE:&str=concat!("kohdalab-camera-",env!("CARGO_PKG_VERSION"));
const TEXT:usize=2048;
const BYTES:usize=65536;
pub fn bounded(s:&str)->String {let mut end=s.len().min(TEXT);while !s.is_char_boundary(end){end-=1;} s[..end].to_owned()}
#[cfg(test)]
fn text(s:&str)->serde_json::Value {serde_json::json!({"text":bounded(s),"original_bytes":s.len(),"retained_bytes":bounded(s).len(),"truncated":s.len()>TEXT})}
fn settings(s:crate::controls::Settings)->serde_json::Value { serde_json::json!({"mode":s.mode.profile().id,"geometry":s.dimensions(),"exposure_rows":s.exposure_rows,"gain_eighths":s.gain_eighths}) }
pub(crate) fn record(s:&Snapshot,kind:&str)->Vec<u8> {
    let l=s.lifecycle.as_ref();
    let value=serde_json::json!({"schema":3,"candidate":CANDIDATE,"kind":kind,
        "source_identity":include_str!("../RECORDER-SOURCE-ID" ).trim(),
        "runtime_identity":"unavailable in record; use reviewed package binding; runtime not attested",
        "first_failure":s.first_failure.as_ref().map(|_|serde_json::json!({"cause":s.first_cause,"code":s.first_failure_code.map(|v|format!("{v:?}")),"detail":"original UI text excluded from records; see safe lifecycle facts"})),
        "latest_message":{"state":format!("{:?}",s.state),"active":s.active,"restart_disabled":s.restart_disabled},
        "owner_revision":s.owner_revision,"owner_settings":s.owner_settings.map(settings),"owner_startup_intent":s.owner_startup_intent.map(|v|format!("{v:?}")),"owner_settings_digest":s.owner_settings_digest,
        "requested_revision":s.requested_revision,"applied_revision":s.applied_revision,
        "requested_settings":s.requested.map(settings),"applied_settings":s.applied.map(settings),
        "startup_intent":s.startup_intent.map(|v|format!("{v:?}")),
        "settings_digest":s.settings_digest,"settings_digest_absence":"null means no current request, digest unavailable on this host, or hashing failed",
        "backend_error":l.map(|l|serde_json::json!({"present":l.backend_error.is_some(),"text":"excluded; untrusted backend text may contain identity"})),
        "backend_error_limit_bytes":57,"backend_truncation":"unknown: existing protocol carries no truncation bit; text at limit may be truncated",
        "parent_error":l.map(|l|serde_json::json!({"present":l.parent_error.is_some(),"text":"excluded; use helper_fault and lifecycle facts"})),
        "backend_cause":l.and_then(|l|l.backend_cause),
        "helper_fault":l.and_then(|l|l.helper_fault).map(|v|format!("{v:?}")),
        "terminal_kind":l.and_then(|l|l.terminal_kind),
        "phase":l.map(|l|l.phase),"proof_flags":l.map(|l|l.proof_flags),
        "stop_observed":l.map(|l|l.stop_observed),"release_committed":l.map(|l|l.release_committed),
        "marker_present":null,"marker_present_absence":"not probed; retention is not inferred from deletion=false",
        "marker_deleted":l.map(|l|l.marker_deleted),"collected":l.map(|l|l.collected),"exit":l.and_then(|l|l.exit),
        "absent_fields":"null means unavailable or not yet observed; false is only an existing runtime fact",
        "device_identity":"deliberately excluded","pixels":"deliberately excluded"});
    let bytes=serde_json::to_vec_pretty(&value).expect("bounded JSON");
    assert!(bytes.len()<=BYTES);bytes
}
fn reserve(base:&std::path::Path)->std::io::Result<PathBuf> {
    std::fs::create_dir_all(base)?;
    for slot in 0..32 {let path=base.join(format!("slot-{slot:02}"));match std::fs::create_dir(&path) {Ok(())=>return Ok(path),Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>{},Err(e)=>return Err(e)}}
    Err(std::io::Error::other("32-record-session quota exhausted; retained records require review"))
}
fn write(root:&std::path::Path,name:&str,bytes:&[u8])->std::io::Result<()> {
    use std::io::Write;
    if bytes.len()>BYTES {return Err(std::io::Error::other("record exceeds byte limit"));}
    std::fs::create_dir_all(root)?;
    let mut f=std::fs::OpenOptions::new().write(true).create_new(true).open(root.join(name))?;
    f.write_all(bytes)?;f.sync_all()
}
pub struct Recorder {tx:Option<mpsc::SyncSender<(&'static str,Vec<u8>)>>,status:Arc<Mutex<String>>,join:Option<thread::JoinHandle<()>>,first:bool,final_sent:bool}
impl Recorder {
    pub fn new()->Self {
        // Fixed candidate-specific namespace under executable directory. No export or lock paths.
        let root=std::env::current_exe().map(|p|p.parent().unwrap().join(CANDIDATE));
        match root {Ok(root)=>Self::at(root),Err(_)=>Self::unavailable("runtime directory unavailable; no fallback writes")}
    }
    fn unavailable(reason:&str)->Self {Self{tx:None,status:Arc::new(Mutex::new(format!("FAILURE RECORD NOT PERSISTED: {reason}"))),join:None,first:false,final_sent:false}}
    pub fn at(root:PathBuf)->Self {
        let (tx,rx)=mpsc::sync_channel::<(&'static str,Vec<u8>)>(2);
        let status=Arc::new(Mutex::new("Failure persistence pending".into()));let out=status.clone();
        let join=thread::Builder::new().name(format!("failure-recorder-{CANDIDATE}")).spawn(move ||{let mut destination=None;for (name,bytes) in rx {
            let result=(|| {if destination.is_none(){destination=Some(reserve(&root)?);}write(destination.as_ref().unwrap(),name,&bytes)})();
            let mut s=out.lock().unwrap();
            if let Err(e)=result {*s=format!("FAILURE RECORD NOT PERSISTED: {e}");}
            else if !s.starts_with("FAILURE") {*s=format!("Failure record synced: {}",destination.as_ref().unwrap().display());}
        }});
        match join {Ok(join)=>Self {tx:Some(tx),status,join:Some(join),first:false,final_sent:false},Err(_)=>Self::unavailable("recorder thread unavailable; no writes")}
    }
    pub fn observe(&mut self,s:&mut Snapshot) {
        if s.first_failure.is_some() {
            let kind=if !self.first {self.first=true;Some("first.json")} else if !s.active && !self.final_sent {self.final_sent=true;Some("collection.json")} else {None};
            if let (Some(kind),Some(tx))=(kind,self.tx.as_ref()) {if tx.try_send((kind,record(s,kind))).is_err() {*self.status.lock().unwrap()="FAILURE RECORD NOT PERSISTED: bounded queue unavailable".into();}}
            s.persistence=self.status.lock().unwrap().clone();
        }
    }
    // Called on acquisition thread after collection, never on UI; drain two writes.
    pub fn finish(&mut self,s:&mut Snapshot) {self.observe(s);self.tx.take();if let Some(j)=self.join.take(){if j.join().is_err(){*self.status.lock().unwrap()="FAILURE RECORD NOT PERSISTED: recorder panicked".into();}}if self.first{s.persistence=self.status.lock().unwrap().clone();}}
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn v038_candidate_label(){
 let value:serde_json::Value=serde_json::from_slice(&record(&Snapshot::default(),"first.json")).unwrap();
 assert_eq!(value["candidate"],CANDIDATE);assert_eq!(value["schema"],3);assert!(value["first_failure"].is_null());
 }
 #[test] fn first_survives_cleanup_and_bounds_and_write_failure(){
 let root=std::env::temp_dir().join(format!("v035-recorder-{}",std::process::id()));std::fs::create_dir_all(&root).unwrap();
 let mut r=Recorder::at(root.join("ok"));let mut s=Snapshot::default();s.first_failure=Some("preopen backend failure".into());s.message="protocol failure".into();s.active=true;r.observe(&mut s);
 s.message="Cleanup unproved".into();s.active=false;r.finish(&mut s);
 let a:serde_json::Value=serde_json::from_slice(&std::fs::read(root.join("ok/slot-00/first.json")).unwrap()).unwrap();assert!(a["first_failure"]["detail"].is_string());assert_eq!(a["latest_message"]["active"],true);
 let b:serde_json::Value=serde_json::from_slice(&std::fs::read(root.join("ok/slot-00/collection.json")).unwrap()).unwrap();assert_eq!(a["first_failure"],b["first_failure"]);assert_eq!(b["latest_message"]["active"],false);
 s.message="é".repeat(10000);s.first_failure=Some(bounded(&s.message));s.first_failure_original_bytes=Some(s.message.len());let bytes=record(&s,"bounds");assert!(bytes.len()<BYTES);let value:serde_json::Value=serde_json::from_slice(&bytes).unwrap();assert!(value["first_failure"]["detail"].is_string());assert_eq!(text(&s.message)["truncated"],true);
 std::fs::write(root.join("blocked"),b"file").unwrap();let mut r=Recorder::at(root.join("blocked"));r.observe(&mut s);r.finish(&mut s);assert!(s.persistence.starts_with("FAILURE RECORD NOT PERSISTED"));let quota=root.join("quota");for _ in 0..32 {reserve(&quota).unwrap();}assert!(reserve(&quota).is_err());
 assert!(write(&root,"oversize",&vec![0;BYTES+1]).is_err());
 let mut unavailable=Recorder::unavailable("injected missing runtime directory");unavailable.observe(&mut s);unavailable.finish(&mut s);assert!(s.persistence.contains("missing runtime directory"));
 std::fs::remove_dir_all(root).unwrap();
 }
}
