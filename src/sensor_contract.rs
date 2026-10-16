//! Finite public MT9T001 Rev D geometry, not a general register editor.
use crate::controls::{Mode, Settings};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub version: u32,
    pub profile: String,
    pub source_window: [u16; 4], // column, row, width, height
    pub output: [u16; 2],
    pub stride: u16,
    pub payload_bytes: u32,
    pub reduction: String,
    pub skip: [u8; 2],
    pub bin: [u8; 2],
    pub address_modes: [u16; 2],
    pub mapping_version: u32,
    pub cfa: String,
    pub orientation: String,
    pub display_transform: String,
    pub sensor_bits: u8,
    pub transport_bits: u8,
    pub qualification: String,
}
impl Geometry {
    pub fn for_mode(mode: Mode) -> Result<Self, String> {
        if !Mode::ALL.contains(&mode) { return Err("Legacy crop is not a current geometry".into()); }
        let p = mode.profile();
        let skip = mode == Mode::FullFieldSkip;
        Ok(Self {
            version: 1, profile: p.id.into(), source_window: [32,20,2048,1536],
            output: [p.width,p.height], stride: p.width,
            payload_bytes: u32::from(p.width)*u32::from(p.height),
            reduction: if skip { "sensor_readout_pair_skip" } else { "unskipped" }.into(),
            skip: if skip {[2,2]} else {[1,1]}, bin: [1,1],
            address_modes: if skip {[1,1]} else {[0,0]}, mapping_version: 1,
            cfa: "GRBG".into(), orientation: "sensor_unmirrored_expected".into(),
            display_transform: "identity".into(), sensor_bits: 10, transport_bits: 8,
            qualification: "physical_FOV_CFA_color_unverified".into(),
        })
    }
    pub fn validate(&self) -> Result<Mode, String> {
        let mode = Mode::parse(&self.profile)?;
        if *self != Self::for_mode(mode)? { return Err("Inconsistent finite sensor geometry".into()); }
        Ok(mode)
    }
    pub fn sensor_coordinate(&self, x: u16, y: u16) -> Result<(u16,u16),String> {
        self.validate()?;
        if x >= self.output[0] || y >= self.output[1] { return Err("Output coordinate bounds".into()); }
        let map = |v:u16, s:u8| if s == 2 { 4*(v/2)+v%2 } else {v};
        Ok((32+map(x,self.skip[0]),20+map(y,self.skip[1])))
    }
    /// Reset-default D=0 assumption; no calibrated seconds or measured clock.
    pub fn timing(&self, exposure_rows: u16) -> Result<(u32,u64),String> {
        self.validate()?;
        let line = (u32::from(self.output[0])+331+38+142).max(331+316);
        Ok((line,u64::from(u32::from(exposure_rows).max(u32::from(self.output[1])+26))*u64::from(line)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acquisition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup_intent: Option<crate::startup::Intent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_status: Option<crate::startup::DiagnosticStatus>,
    pub version: u32,
    pub geometry: Geometry,
    pub exposure_rows: u16,
    pub gain_eighths: u16,
    pub clock_register: u16,
    pub blanking: [u16; 2],
    pub expected_register_snapshot: Vec<(u16,u16)>,
    pub session: [u8;32],
    pub settings_generation: u64,
    pub request_revision: Option<u64>,
    pub sequence: u64,
}
impl Acquisition {
    pub fn new(settings: Settings, session: [u8;32], generation: u64, sequence: u64) -> Result<Self,String> {
        settings.validate()?;
        let mut registers=settings.mode.profile().window().to_vec();
        let address=if settings.mode==Mode::FullFieldSkip {1} else {0};
        registers.extend([(5,142),(6,25),(8,0),(9,settings.exposure_rows),(0x0a,0x8000),
            (0x1e,0x8040),(0x20,0x2000),(0x21,0),(0x22,address),(0x23,address),(0x35,settings.gain_eighths)]);
        registers.extend((0x2b..=0x2e).map(|a|(a,settings.gain_eighths)));
        Ok(Self {startup_intent:None, diagnostic_status:None, version:1, geometry: Geometry::for_mode(settings.mode)?,
            exposure_rows: settings.exposure_rows, gain_eighths:settings.gain_eighths,
            clock_register: settings.clock(), blanking:[142,25],expected_register_snapshot:registers,session,
            settings_generation:generation,request_revision:None,sequence})
    }
    pub fn with_startup(mut self, intent: crate::startup::Intent) -> Self {
        self.version = 2;
        self.startup_intent = Some(intent);
        self.diagnostic_status = Some(intent.completed());
        self
    }
    pub fn settings(&self) -> Result<Settings,String> {
        let settings = Settings {mode:self.geometry.validate()?, exposure_rows:self.exposure_rows,gain_eighths:self.gain_eighths}.validate()?;
        if !match (self.version, self.startup_intent, self.diagnostic_status) {
            (1, None, None) => true,
            (2, Some(i), Some(s)) => s == i.completed(),
            _ => false,
        } || self.clock_register != settings.clock() || self.blanking != [142,25]
            || self.session == [0;32] || self.settings_generation == 0 || self.sequence == 0 {
            return Err("Invalid acquisition binding".into());
        }
        if self.expected_register_snapshot != Self::new(settings,self.session,self.settings_generation,self.sequence)?.expected_register_snapshot {
            return Err("Inconsistent register snapshot".into());
        }
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_metadata_is_explicit_frozen_and_legacy_unknown() {
        use crate::startup::{Intent,DiagnosticStatus};
        for mode in Mode::ALL { for intent in [Intent::Normal,Intent::Diagnostics] {
            let a=Acquisition::new(Settings {mode,..Default::default()},[3;32],7,8).unwrap().with_startup(intent);
            a.settings().unwrap();
            let bytes=serde_json::to_vec(&a).unwrap();
            let frozen:Acquisition=serde_json::from_slice(&bytes).unwrap();
            assert_eq!(a,frozen);
            for status in [DiagnosticStatus::NotRun,DiagnosticStatus::Passed,DiagnosticStatus::Failed,DiagnosticStatus::Incomplete] {
                let mut bad=a.clone();bad.diagnostic_status=Some(status);
                assert_eq!(bad.settings().is_ok(),status==intent.completed());
            }
            let mut missing=a.clone();missing.startup_intent=None;assert!(missing.settings().is_err());
            let mut unknown:serde_json::Value=serde_json::from_slice(&bytes).unwrap();
            unknown["startup_intent"]="normal/v2".into();
            assert!(serde_json::from_value::<Acquisition>(unknown).is_err());
            let legacy=Acquisition::new(Settings {mode,..Default::default()},[3;32],7,8).unwrap();
            assert_eq!(legacy.startup_intent,None);assert_eq!(legacy.diagnostic_status,None);legacy.settings().unwrap();
        }}
        assert_eq!(Intent::Normal.status(true,false),DiagnosticStatus::NotRun);
        assert_eq!(Intent::Normal.status(false,true),DiagnosticStatus::NotRun);
        assert_eq!(Intent::Diagnostics.status(false,false),DiagnosticStatus::Incomplete);
        assert_eq!(Intent::Diagnostics.status(false,true),DiagnosticStatus::Failed);
    }
    #[test]
    fn independent_coordinate_tags_pairs_endpoints_and_cfa() {
        let g=Geometry::for_mode(Mode::FullFieldSkip).unwrap();
        // Literal public-coordinate expectations, never built with the mapping function.
        let xs:[u16;8]=[32,33,36,37,40,41,2076,2077];
        let ys:[u16;8]=[20,21,24,25,28,29,1552,1553];
        let ox=[0,1,2,3,4,5,1022,1023]; let oy=[0,1,2,3,4,5,766,767];
        let source:Vec<u32>=(0..1556u32).flat_map(|y|(0..2080u32).map(move |x|y*4096+x)).collect();
        for (j,y) in oy.into_iter().enumerate() { for (i,x) in ox.into_iter().enumerate() {
            let (sx,sy)=g.sensor_coordinate(x,y).unwrap();
            assert_eq!(source[usize::from(sy)*2080+usize::from(sx)],u32::from(ys[j])*4096+u32::from(xs[i]));
            assert_eq!((sx%2,sy%2),(x%2,y%2));
        }}
        assert_ne!(g.sensor_coordinate(1,1).unwrap(),(34,22)); // naive every-other breaks CFA
        assert!(g.sensor_coordinate(1024,0).is_err()); assert!(g.sensor_coordinate(0,768).is_err());
        assert_eq!(g.timing(100).unwrap(),(1535,1218790));
        assert_eq!(g.timing(794).unwrap().1,1218790);
        assert_eq!(g.timing(795).unwrap().1,1220325);
        assert_eq!(Geometry::for_mode(Mode::Full).unwrap().timing(100).unwrap(),(2559,3997158));
    }
    #[test]
    fn forged_contracts_and_legacy_ids_reject() {
        let g=Geometry::for_mode(Mode::FullFieldSkip).unwrap();
        for i in 0..10 { let mut bad=g.clone(); match i {
            0=>bad.version=2,1=>bad.profile="xga".into(),2=>bad.source_window[2]=1024,
            3=>bad.cfa="RGGB".into(),4=>bad.orientation="mirror".into(),5=>bad.skip=[2,1],
            6=>bad.bin=[2,2],7=>bad.stride=2048,8=>bad.payload_bytes+=1,_=>bad.source_window[0]=33,
        }; assert!(bad.validate().is_err()); }
        for id in ["xga","xga_crop/v1","full","small","hd720","hd960","xga_full_field_skip/v2"] { assert!(Mode::parse(id).is_err()); }
        assert!(Mode::from_dimensions(1024,768).is_err());
        assert!(Mode::from_dimensions(2048,1536).is_err());
        assert_eq!(Mode::ALL,[Mode::FullFieldSkip,Mode::Full]);
    }
    #[test]
    fn typed_raw_roundtrip_malformed_bytes_and_legacy_reading() {
        use crate::{Frame,Metadata};
        use std::sync::Arc;
        let root=std::env::temp_dir().join(format!("two-mode-roundtrip-{}",std::process::id()));
        std::fs::create_dir(&root).unwrap();
        for mode in Mode::ALL {
            let s=Settings {mode,..Default::default()};
            let a=Acquisition::new(s,[7;32],9,11).unwrap().with_startup(if mode==Mode::FullFieldSkip {crate::startup::Intent::Normal} else {crate::startup::Intent::Diagnostics});
            let f=Frame {raw:Arc::new((0..a.geometry.payload_bytes).map(|i|(i%251) as u8).collect()),metadata:Metadata {
                acquisition:Some(a),source:"independent coordinate fixture; no optical claim".into(),simulated:true,
                sequence:11,measurement: None, unix_millis:0,width:u32::from(s.dimensions().0),height:u32::from(s.dimensions().1),format:"BayerGRBG8".into(),
            }};
            f.validate().unwrap();
            crate::export(&f,&root.join(format!("{}.png",mode.profile().name))).unwrap();
            let bundle=root.join(format!("{}.png.capture",mode.profile().name));
            assert_eq!(std::fs::read(bundle.join("raw.bin")).unwrap(),*f.raw);
            let m:Metadata=serde_json::from_slice(&std::fs::read(bundle.join("metadata.json")).unwrap()).unwrap();
            assert_eq!(m,f.metadata); assert_eq!(m.unix_millis,0);
            assert_eq!(image::open(bundle.join(format!("{}.png",mode.profile().name))).unwrap().to_rgb8().as_raw(),f.display_rgb().as_ref());
            for change in 0..6 { let mut bad=f.clone(); match change {
                0=>bad.metadata.width/=2,1=>bad.metadata.sequence+=1,
                2=>bad.metadata.acquisition.as_mut().unwrap().geometry.cfa="RGGB".into(),
                3=>bad.metadata.acquisition.as_mut().unwrap().version=99,
                4=>bad.raw=Arc::new(vec![0;f.raw.len()-1]),
                _=>bad.metadata.acquisition.as_mut().unwrap().session=[0;32],
            };assert!(bad.validate().is_err()); }
        }
        let legacy:Metadata=serde_json::from_str(r#"{"source":"legacy xga crop","simulated":false,"sequence":1,"unix_millis":0,"width":1024,"height":768,"format":"BayerGRBG8"}"#).unwrap();
        assert!(legacy.acquisition.is_none()); assert_eq!(legacy.source,"legacy xga crop");
        std::fs::remove_dir_all(root).unwrap();
    }
}
