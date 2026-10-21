#![forbid(unsafe_code)]
//! Focused native FAKE-only startup checks. No physical entry or enumeration.
#[cfg(windows)]
fn main() {
    use camera_lab::{continuous_session::{ContinuousSession,Fixture,Terminal}, controls::Settings, startup::{Intent,DiagnosticStatus}};
    use std::{path::PathBuf,time::{Instant,Duration}};
    let args:Vec<_>=std::env::args().collect();
    assert_eq!(args.len(),3);
    let (fixture,intent,early,unproved)=match args[1].as_str() {
        "normal"=>(Fixture::TypedFrames,Intent::Normal,false,false),
        "diagnostics"=>(Fixture::TypedFrames,Intent::Diagnostics,false,false),
        "early-stop"=>(Fixture::BootstrapStop,Intent::Normal,true,false),
        "drain"=>(Fixture::DrainFail,Intent::Normal,false,true),
        "cleanup"=>(Fixture::CleanupFail,Intent::Normal,false,true),
        _=>panic!("unknown fake-only case"),
    };
    let root=PathBuf::from(&args[2]);std::fs::create_dir(&root).unwrap();
    let at=Instant::now();
    let session=ContinuousSession::start_fixture_intent(&root,fixture,Settings::default(),7,intent).unwrap();
    if early {session.stop();}
    let mut frozen=None;
    let mut first_ns=None;
    loop {
        let s=session.poll();
        assert_eq!(s.startup_intent,Some(intent));
        if let Some(f)=&s.frame {
            first_ns.get_or_insert(at.elapsed().as_nanos());
            if fixture==Fixture::TypedFrames {
                let a=f.metadata.acquisition.as_ref().unwrap();
                assert_eq!(a.startup_intent,Some(intent));assert_eq!(a.diagnostic_status,Some(intent.completed()));
                assert_eq!(a.settings_generation,7);f.validate().unwrap();
                if frozen.is_none() {frozen=Some(f.clone());}
            }
            // Give the independent native observer time to retain this helper.
            // This fixture delay is not included in startup latency.
            if at.elapsed().as_nanos().saturating_sub(first_ns.unwrap()) >= 500_000_000 {session.stop();}
        }
        if s.collected {
            eprintln!("collected terminal={:?} exit={:?} first_error={:?} helper_fault={:?} flags={} frames={}",s.terminal,s.exit,s.first_error,s.helper_fault,s.proof_flags,s.frames);
            assert_eq!(s.exit,Some(0));
            if unproved {
                assert!(!s.marker_deleted);assert!(!s.release_committed);assert_ne!(s.terminal,Some(Terminal::CleanCancellation));
                // Never restart this failed owner; retained fake marker is evidence.
                assert!(std::fs::read_dir(&root).unwrap().any(|p|p.unwrap().path().extension().is_some_and(|e|e=="lock")));
            } else {
                assert!(s.marker_deleted);assert!(s.release_committed);assert_eq!(s.terminal,Some(Terminal::CleanCancellation));
            }
            if early {assert_eq!(s.frames,0);}
            if intent==Intent::Normal {
                assert_eq!(s.diagnostic_status(),Some(DiagnosticStatus::NotRun));
                if let Some(d)=s.diagnostics {for i in 0..3 {assert!(d.phase(i).is_none());}}
            } else {
                assert_eq!(s.diagnostic_status(),Some(DiagnosticStatus::Passed));
                for i in 0..3 {assert!(s.diagnostics.unwrap().phase(i).is_some());}
            }
            if let Some(f)=frozen {
                assert!(f.raw.iter().enumerate().all(|(i,b)|*b==(i as u64+f.metadata.sequence) as u8));
                camera_lab::export(&f,&root.join("frozen.png")).unwrap();
                assert_eq!(std::fs::read(root.join("frozen.png.capture/raw.bin")).unwrap(),*f.raw);
                let m:camera_lab::Metadata=serde_json::from_slice(&std::fs::read(root.join("frozen.png.capture/metadata.json")).unwrap()).unwrap();assert_eq!(m,f.metadata);
            }
            println!("{}",serde_json::json!({"case":args[1],"physical":false,"startup_intent":intent,"diagnostic_status":s.diagnostic_status(),"helper_exit":s.exit,"collected":s.collected,"marker_deleted":s.marker_deleted,"release_committed":s.release_committed,"terminal":format!("{:?}",s.terminal),"first_matching_ns":first_ns,"diagnostics":s.diagnostics.map(|d|d.phases()),"no_restart_attempted":true}));
            break;
        }
        assert!(at.elapsed()<Duration::from_secs(50),"finite fake deadline; no kill/retry");
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[cfg(not(windows))]
fn main() {panic!("Windows FAKE-only fixture");}
