//! User preferences only; never stores device ownership or reconnect authority.
use camera_lab::{controls::{Mode,Settings},display_recipe::Recipe};
use serde::{Deserialize,Serialize};
use std::{io::Write,path::{Path,PathBuf}};
use tempfile::NamedTempFile;
pub const SCHEMA_VERSION: u32 = 2;
pub const APPLICATION: &str = "kohdalab-camera";
pub const DEFAULT_LIVE_SIZE: [f32; 2] = [1100., 760.];

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub application:String,pub version:u32,pub mode:String,pub exposure:u16,pub gain:u16,
    pub recipe:Recipe,pub live_size:[f32;2],pub folder:Option<PathBuf>,pub next_sequence:u64,
}
impl Default for Preferences {
    fn default()->Self {let s=Settings::default();Self{application:APPLICATION.into(),version:SCHEMA_VERSION,mode:s.mode.profile().id.into(),exposure:s.exposure_rows,gain:s.gain_eighths,recipe:Recipe::default(),live_size:DEFAULT_LIVE_SIZE,folder:None,next_sequence:1}}
}
impl Preferences {
    pub fn settings(&self)->Result<Settings,String> {Settings{mode:Mode::parse(&self.mode)?,exposure_rows:self.exposure,gain_eighths:self.gain}.validate()}
    pub fn validate(&self)->Result<(),String> {
        self.settings()?;self.recipe.validate()?;
        if self.application!=APPLICATION || self.version!=SCHEMA_VERSION || !self.live_size[0].is_finite() || !self.live_size[1].is_finite() || !(900.0..=8192.).contains(&self.live_size[0]) || !(620.0..=8192.).contains(&self.live_size[1]) || self.folder.as_ref().is_some_and(|p|!p.is_absolute()) || self.next_sequence==0 {return Err("Invalid saved preferences".into());}Ok(())
    }
    pub fn load(path:&Path)->Result<Self,String> {
        use std::io::Read;
        let file=match std::fs::File::open(path){Ok(f)=>f,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Self::default()),Err(e)=>return Err(e.to_string())};
        let mut data=Vec::new();file.take(65537).read_to_end(&mut data).map_err(|e|e.to_string())?;
        if data.len()>65536{return Err("Saved preferences too large".into());}
        let p:Self=serde_json::from_slice(&data).map_err(|e|e.to_string())?;p.validate()?;Ok(p)
    }
    pub fn save(&self,path:&Path)->Result<(),String> {
        self.validate()?;
        if path.exists() {Self::load(path).map_err(|e|format!("Refusing to overwrite invalid saved preferences: {e}"))?;}
        let parent=path.parent().ok_or("Missing preference directory")?;
        std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;
        // Fully write and sync a sibling first. `replace` never deletes the old
        // file before the new one is ready, including on Windows.
        let data=serde_json::to_vec_pretty(self).map_err(|e|e.to_string())?;
        let tmp=path.with_extension(format!("{}.tmp",std::process::id()));
        let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(|e|e.to_string())?;
        if let Err(error)=file.write_all(&data).and_then(|_|file.sync_all()) {drop(file);let _=std::fs::remove_file(&tmp);return Err(error.to_string());}
        // Windows cannot reliably replace a destination while this handle is open.
        drop(file);
        if let Err(error)=replace(&tmp,path) {let _=std::fs::remove_file(&tmp);return Err(error.to_string());}
        Ok(())
    }
    pub fn ensure_default(path:&Path)->Result<bool,String> {
        if path.exists() { return Ok(false); }
        let parent=path.parent().ok_or("Missing preference directory")?;
        std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;
        let data=serde_json::to_vec_pretty(&Self::default()).map_err(|e|e.to_string())?;
        let mut temporary=NamedTempFile::new_in(parent).map_err(|e|e.to_string())?;
        temporary.write_all(&data).and_then(|_|temporary.as_file_mut().sync_all()).map_err(|e|e.to_string())?;
        match temporary.persist_noclobber(path) {
            Ok(_) => Ok(true),
            Err(error) if error.error.kind()==std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(error) => Err(error.error.to_string()),
        }
    }
    pub fn path()->Option<PathBuf> {
        #[cfg(windows)] {std::env::var_os("USERPROFILE").map(|p|PathBuf::from(p).join(".kohdalab/camera/config.json"))}
        #[cfg(not(windows))] {std::env::var_os("HOME").map(|p|PathBuf::from(p).join(".kohdalab/camera/config.json"))}
    }
    pub fn suggested_name(&self,full:bool)->String {
        let stem=if full {"full"}else{"capture"};
        for offset in 0..1000 {let name=format!("{stem}_{:04}.png",self.next_sequence.saturating_add(offset));
            if self.folder.as_ref().is_none_or(|d|!["", ".raw", ".json", ".original-RAW.tiff", ".capture"].iter().any(|suffix|d.join(format!("{name}{suffix}")).exists())) {return name;}}
        format!("{stem}_{}.png",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos())
    }
}
#[cfg(not(windows))]
fn replace(source:&Path,destination:&Path)->std::io::Result<()> {std::fs::rename(source,destination)}
#[cfg(windows)]
fn replace(source:&Path,destination:&Path)->std::io::Result<()> {
    capability_platform::replace_file(source,destination)
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn bounded_validated_roundtrip_and_collision() {
        let root=std::env::temp_dir().join(format!("camera-prefs-{}",std::process::id()));std::fs::create_dir_all(&root).unwrap();let path=root.join("settings.json");
        assert_eq!(Preferences::load(&path).unwrap(),Preferences::default());
        let mut p=Preferences{folder:Some(root.clone()),next_sequence:11,..Default::default()};p.save(&path).unwrap();p.next_sequence=12;p.save(&path).unwrap();assert_eq!(Preferences::load(&path).unwrap(),p);
        std::fs::write(root.join("capture_0012.png.raw"),b"occupied").unwrap();assert_eq!(p.suggested_name(false),"capture_0013.png");
        let mut bad=p.clone();bad.exposure=0;assert!(bad.save(&path).is_err());assert_eq!(Preferences::load(&path).unwrap(),p);
        std::fs::write(&path,vec![b' ';65537]).unwrap();assert!(Preferences::load(&path).is_err());std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn save_refuses_foreign_file_without_changing_it() {
        let root=std::env::temp_dir().join(format!("camera-prefs-foreign-{}",std::process::id()));std::fs::create_dir_all(&root).unwrap();let path=root.join("config.json");
        let foreign=b"{\"application\":\"kohdalab-iv\"}".to_vec();std::fs::write(&path,&foreign).unwrap();
        assert!(Preferences::default().save(&path).is_err());assert_eq!(std::fs::read(&path).unwrap(),foreign);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn ensure_default_creates_once_and_preserves_existing_or_invalid() {
        let root=std::env::temp_dir().join(format!("camera-prefs-first-{}",std::process::id()));std::fs::create_dir_all(&root).unwrap();let path=root.join("config.json");
        assert!(Preferences::ensure_default(&path).unwrap()); let first=std::fs::read(&path).unwrap();
        assert!(!Preferences::ensure_default(&path).unwrap()); assert_eq!(std::fs::read(&path).unwrap(),first);
        let foreign=br#"{"application":"foreign"}"#.to_vec(); std::fs::write(&path,&foreign).unwrap();
        assert!(!Preferences::ensure_default(&path).unwrap()); assert_eq!(std::fs::read(&path).unwrap(),foreign); std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn ensure_default_concurrent_initialization_publishes_complete_json() {
        let root=std::env::temp_dir().join(format!("camera-prefs-race-{}",std::process::id()));std::fs::create_dir_all(&root).unwrap();let path=root.join("config.json");
        let a={let p=path.clone();std::thread::spawn(move||Preferences::ensure_default(&p).unwrap())};
        let b={let p=path.clone();std::thread::spawn(move||Preferences::ensure_default(&p).unwrap())};
        assert_eq!(a.join().unwrap() as u8+b.join().unwrap() as u8,1); assert!(Preferences::load(&path).is_ok()); std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn ensure_default_reports_unusable_parent() {
        let root=std::env::temp_dir().join(format!("camera-prefs-unusable-{}",std::process::id())); std::fs::write(&root,b"file").unwrap();
        assert!(Preferences::ensure_default(&root.join("child/config.json")).is_err()); std::fs::remove_file(root).unwrap();
    }
}
