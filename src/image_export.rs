//! Frozen full-resolution RGB plus exact original sensor sidecars.
use crate::Frame;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
/// Historical v1 descriptor; new metadata uses Recipe::processing by source/version.
pub const PROCESSING: &str = "GRBG-cell-RGB8 + software display recipe/v1; no calibrated color/gamma/white-balance";
fn suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
/// Existing primary/sidecars are refused. Dialog cancellation never reaches this function.
/// A same-directory hard link publishes a fully encoded primary atomically without overwrite.
/// Sidecar failure retains the valid primary and reports partial completion explicitly.
pub fn save(f: &Frame, path: &Path) -> Result<(), String> {
    save_recipe(f, path, crate::display_recipe::Recipe::default())
}
pub fn save_recipe(f: &Frame, path: &Path, recipe: crate::display_recipe::Recipe) -> Result<(), String> {
    f.validate()?;
    recipe.validate()?;
    let format = match path
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => image::ImageFormat::Png,
        Some("tif" | "tiff") => image::ImageFormat::Tiff,
        _ => return Err("Choose PNG or TIFF".into()),
    };
    let raw = suffix(path, ".raw");
    let metadata = suffix(path, ".json");
    let original = suffix(path, ".original-RAW.tiff");
    if [path, raw.as_path(), metadata.as_path(), original.as_path()]
        .iter()
        .any(|p| p.exists())
    {
        return Err("Existing image or sidecars: choose a new name; overwrite is disabled".into());
    }
    let temp = path
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!(".camera-lab-save-{}.pending", std::process::id()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        let mut writer = std::io::BufWriter::new(file);
        let (width,height,pixels) = recipe.render(f,None)?;
        image::write_buffer_with_format(
            &mut writer,
            &pixels,
            width,
            height,
            image::ColorType::Rgb8,
            format,
        )
        .map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        writer.get_ref().sync_all().map_err(|e| e.to_string())?;
        fs::hard_link(&temp, path)
            .map_err(|e| format!("Primary publication failed (no overwrite): {e}"))?;
        let sidecars = (|| -> Result<(), String> {
            let mut out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(raw)
                .map_err(|e| e.to_string())?;
            out.write_all(f.raw.as_slice())
                .and_then(|_| out.sync_all())
                .map_err(|e| e.to_string())?;
            let original_info=if f.metadata.format=="BayerGRBG8" {
                let file=OpenOptions::new().write(true).create_new(true).open(&original).map_err(|e|e.to_string())?;
                let mut writer=std::io::BufWriter::new(file);
                image::write_buffer_with_format(&mut writer,&f.raw,f.metadata.width,f.metadata.height,image::ColorType::L8,image::ImageFormat::Tiff).map_err(|e|e.to_string())?;
                writer.flush().and_then(|_|writer.get_ref().sync_all()).map_err(|e|e.to_string())?;
                serde_json::json!({"available":true,"path":original,"format":"TIFF L8 BayerGRBG8","dimensions":[f.metadata.width,f.metadata.height],"orientation":"unchanged sensor RAW","data_unchanged":true})
            }else{serde_json::json!({"available":false,"reason":"source RGB8; no original Bayer sensor data"})};
            let data = serde_json::json!({"frame":f.metadata,"processing":recipe.processing(&f.metadata.format),"original_sensor_tiff":original_info,"display_recipe":recipe,"image_dimensions":[width,height],"orientation":"software reflections then clockwise quarter turns; RAW orientation unchanged","color_assumptions":"Declared GRBG software reconstruction or source RGB8; software arithmetic on encoded channel values; no calibrated color/gamma","raw_preserved":true,"exposure_time_seconds":null,"timestamp_semantics":crate::measurement::timestamp_semantics(&f.metadata)});
            let mut out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(metadata)
                .map_err(|e| e.to_string())?;
            out.write_all(&serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?)
                .and_then(|_| out.sync_all())
                .map_err(|e| e.to_string())
        })();
        sidecars.map_err(|e| {
            format!(
                "PRIMARY IMAGE SAVED at {}; sidecars INCOMPLETE: {e}",
                path.display()
            )
        })
    })();
    let _ = fs::remove_file(temp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_cell_pixels_primary_overwrite_disk_and_sidecar_outcomes() {
        let root = std::env::temp_dir().join(format!("direct-export-v022-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let f = Frame {
            raw: std::sync::Arc::new(vec![0, 10, 20, 30]),
            metadata: crate::Metadata {
                acquisition: None,
                source: "independent literal GR/BG cell".into(),
                simulated: true,
                sequence: 1,
                measurement: None, unix_millis: 0,
                width: 2,
                height: 2,
                format: "BayerGRBG8".into(),
            },
        };
        let expected = [10, 15, 20, 10, 15, 20, 10, 15, 20, 10, 15, 20];
        assert_eq!(f.legacy_preview_rgb(1024).unwrap().2, expected);
        for ext in ["png", "tiff"] {
            let path = root.join(format!("chosen.{ext}"));
            save_recipe(&f, &path, crate::display_recipe::Recipe::legacy()).unwrap();
            assert_eq!(image::open(&path).unwrap().to_rgb8().as_raw(), &expected);
            assert_eq!(fs::read(suffix(&path, ".raw")).unwrap(), *f.raw);
            assert!(save_recipe(&f, &path, crate::display_recipe::Recipe::legacy()).unwrap_err().contains("Existing"));
        }
        assert!(save_recipe(&f, &root.join("missing/image.png"), crate::display_recipe::Recipe::legacy()).is_err());
        assert!(!root.join("missing/image.png").exists());
        #[cfg(unix)]
        {
            // Primary basename fits NAME_MAX; its RAW sidecar does not.
            let path = root.join(format!("{}.png", "x".repeat(248)));
            assert!(save_recipe(&f, &path, crate::display_recipe::Recipe::legacy()).unwrap_err().contains("sidecars INCOMPLETE"));
            assert!(path.is_file());
            assert_eq!(image::open(path).unwrap().to_rgb8().as_raw(), &expected);
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)] mod correction_v032 {
 use super::*;
 #[test] fn frozen_asymmetric_bayer_recipe_original_and_metadata() {
  let root=std::env::temp_dir().join(format!("save-v032-{}",std::process::id()));fs::create_dir(&root).unwrap();
  let mut live=Frame{raw:std::sync::Arc::new(vec![3,19,47,83,101,29,157,61]),metadata:crate::Metadata{acquisition:None,source:"own asymmetric held Bayer".into(),simulated:true,sequence:32017,measurement:None,unix_millis:123456789,width:4,height:2,format:"BayerGRBG8".into()}};
  let mut draft=crate::display_recipe::Recipe{ev:1.,gamma:2.,wb:[0.5,1.25,0.75],mirror:true,quarter_turns:1,..crate::display_recipe::Recipe::neutral()};
  let held=live.clone();let frozen=draft;
  // Independent offline scalar oracle: explicit GRBG neighborhoods and
  // reflected edges; source order [7,3,6,2,5,1,4,0]; gain 2, sqrt gamma.
  let expected:Vec<u8>=vec![145, 197, 245, 145, 186, 245, 114, 171, 245, 114, 173, 245, 70, 136, 222, 70, 131, 222, 70, 101, 197, 70, 44, 197];
  let check=|p:&Path| {
   let processed=image::open(p).unwrap();assert_eq!(processed.color(),image::ColorType::Rgb8);assert_eq!((processed.width(),processed.height()),(2,4));assert_eq!(processed.to_rgb8().into_raw(),expected);
   let original_path=suffix(p,".original-RAW.tiff");let original=image::open(&original_path).unwrap();assert_eq!(original.color(),image::ColorType::L8);assert_eq!((original.width(),original.height()),(4,2));assert_eq!(original.to_luma8().as_raw(),held.raw.as_ref());assert_eq!(fs::read(suffix(p,".raw")).unwrap(),*held.raw);
   let m:serde_json::Value=serde_json::from_slice(&fs::read(suffix(p,".json")).unwrap()).unwrap();
   assert_eq!(m["frame"],serde_json::to_value(&held.metadata).unwrap());assert_eq!(m["frame"]["source"],"own asymmetric held Bayer");assert_eq!(m["frame"]["sequence"],32017);assert_eq!(m["frame"]["format"],"BayerGRBG8");
   assert_eq!(m["display_recipe"],serde_json::to_value(frozen).unwrap());assert_eq!(m["image_dimensions"],serde_json::json!([2,4]));
   assert_eq!(m["original_sensor_tiff"],serde_json::json!({"available":true,"path":original_path,"format":"TIFF L8 BayerGRBG8","dimensions":[4,2],"orientation":"unchanged sensor RAW","data_unchanged":true}));
   assert_eq!(m["processing"],"GRBG-bilinear-reflect101-u16-half-up; software recipe/v2; channel-gain-clamp-gamma-LUT256; mono-post-gamma-half-up; processed-RGB-box-u64-half-up; terminal-box-clipped");
   assert_eq!(m["raw_preserved"],true);assert_eq!(m["orientation"],"software reflections then clockwise quarter turns; RAW orientation unchanged");
  };
  for ext in ["png","tiff"] {
   let before=root.join(format!("before.{ext}"));save_recipe(&held,&before,frozen).unwrap();check(&before);
   draft=crate::display_recipe::Recipe{ev:-2.,gamma:0.5,flip:true,quarter_turns:3,..draft};
   live.raw=std::sync::Arc::new(vec![211;8]);live.metadata.sequence+=1;live.metadata.source="new live source".into();
   assert_ne!(draft,frozen);assert_ne!(live.raw,held.raw);assert_ne!(live.metadata,held.metadata);
   let after=root.join(format!("after.{ext}"));save_recipe(&held,&after,frozen).unwrap();check(&after);
   for ending in ["",".raw",".original-RAW.tiff"] {assert_eq!(fs::read(suffix(&before,ending)).unwrap(),fs::read(suffix(&after,ending)).unwrap());}
  }
  fs::remove_dir_all(root).unwrap();
 }
}

#[cfg(test)] mod brightness_export_v033 {
 use super::*;
 #[test] fn default_multiplier_frozen_in_export() {
  let root=std::env::temp_dir().join(format!("default-v033-{}",std::process::id()));fs::create_dir(&root).unwrap();
  let f=Frame{raw:std::sync::Arc::new(vec![10,20,30,40,50,60]),metadata:crate::Metadata{acquisition:None,source:"own RGB fixture".into(),simulated:true,sequence:1,measurement:None,unix_millis:0,width:2,height:1,format:"RGB8".into()}};
  let mut draft=crate::display_recipe::Recipe::default();let frozen=draft;draft.set_brightness(2.);assert_ne!(draft,frozen);
  let path=root.join("default.png");save_recipe(&f,&path,frozen).unwrap();
  let m:serde_json::Value=serde_json::from_slice(&fs::read(suffix(&path,".json")).unwrap()).unwrap();let stored:crate::display_recipe::Recipe=serde_json::from_value(m["display_recipe"].clone()).unwrap();assert_eq!(stored,frozen);assert!((stored.brightness()-1.2).abs()<1e-6);assert_eq!(m["image_dimensions"],serde_json::json!([2,1]));
  // Independently computed round(255 * (channel / 255 * 1.2)^(1/2.2)), rotated 180°.
  assert_eq!(image::open(path).unwrap().to_rgb8().into_raw(),vec![119,132,144,64,87,105]);
  fs::remove_dir_all(root).unwrap();
 }
}
