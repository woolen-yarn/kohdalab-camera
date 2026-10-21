//! User preferences only; never stores device ownership or reconnect authority.
use camera_lab::{controls::{Mode,Settings},display_recipe::Recipe};
use serde::{Deserialize,Serialize};
use std::path::{Path,PathBuf};
pub const SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_LIVE_SIZE: [f32; 2] = [1100., 760.];

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub version:u32,pub mode:String,pub exposure:u16,pub gain:u16,
    pub recipe:Recipe,pub live_size:[f32;2],pub folder:Option<PathBuf>,pub next_sequence:u64,
}
impl Default for Preferences {
    fn default()->Self {let s=Settings::default();Self{version:SCHEMA_VERSION,mode:s.mode.profile().id.into(),exposure:s.exposure_rows,gain:s.gain_eighths,recipe:Recipe::default(),live_size:DEFAULT_LIVE_SIZE,folder:None,next_sequence:1}}
}
impl Preferences {
    pub fn settings(&self)->Result<Settings,String> {Settings{mode:Mode::parse(&self.mode)?,exposure_rows:self.exposure,gain_eighths:self.gain}.validate()}
    pub fn validate(&self)->Result<(),String> {
        self.settings()?;self.recipe.validate()?;
        if self.version!=SCHEMA_VERSION || !self.live_size[0].is_finite() || !self.live_size[1].is_finite() || !(900.0..=8192.).contains(&self.live_size[0]) || !(620.0..=8192.).contains(&self.live_size[1]) || self.folder.as_ref().is_some_and(|p|!p.is_absolute()) || self.next_sequence==0 {return Err("Invalid saved preferences".into());}Ok(())
    }
    pub fn load(path:&Path)->Result<Self,String> {
        use std::io::Read;
        let file=match std::fs::File::open(path){Ok(f)=>f,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Self::default()),Err(e)=>return Err(e.to_string())};
        let mut data=Vec::new();file.take(65537).read_to_end(&mut data).map_err(|e|e.to_string())?;
        if data.len()>65536{return Err("Saved preferences too large".into());}
        let p:Self=serde_json::from_slice(&data).map_err(|e|e.to_string())?;p.validate()?;Ok(p)
    }
    pub fn save(&self,path:&Path)->Result<(),String> {
        self.validate()?;let parent=path.parent().ok_or("Missing preference directory")?;
        std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;
        // Single application writer; complete serialization precedes replacement.
        let data=serde_json::to_vec_pretty(self).map_err(|e|e.to_string())?;
        let tmp=path.with_extension(format!("{}.tmp",std::process::id()));
        std::fs::write(&tmp,data).map_err(|e|e.to_string())?;
        std::fs::rename(&tmp,path).map_err(|e|e.to_string())
    }
    pub fn path()->Option<PathBuf> {
        #[cfg(windows)] {std::env::var_os("APPDATA").map(|p|PathBuf::from(p).join("KohdaLab Camera/settings.json"))}
        #[cfg(not(windows))] {std::env::var_os("HOME").map(|p|PathBuf::from(p).join("Library/Application Support/KohdaLab Camera/settings.json"))}
    }
    pub fn suggested_name(&self,full:bool)->String {
        let stem=if full {"full"}else{"capture"};
        for offset in 0..1000 {let name=format!("{stem}_{:04}.png",self.next_sequence.saturating_add(offset));
            if self.folder.as_ref().is_none_or(|d|!["", ".raw", ".json", ".original-RAW.tiff", ".capture"].iter().any(|suffix|d.join(format!("{name}{suffix}")).exists())) {return name;}}
        format!("{stem}_{}.png",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos())
    }
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
}
