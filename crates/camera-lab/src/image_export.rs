//! Frozen full-resolution RGB plus exact original sensor sidecars.
use crate::Frame;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
/// Historical v1 descriptor; new metadata uses Recipe::processing by source/version.
pub const PROCESSING: &str =
    "GRBG-cell-RGB8 + software display recipe/v1; no calibrated color/gamma/white-balance";
fn suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

pub(crate) fn standard_sidecars(path: &Path) -> [PathBuf; 4] {
    [
        suffix(path, ".csv"),
        suffix(path, ".raw.csv"),
        suffix(path, ".log"),
        suffix(path, ".run.json"),
    ]
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn filename(path: &Path) -> Result<String, String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| "Missing filename".into())
}

pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    out.write_all(bytes)
        .and_then(|_| out.sync_all())
        .map_err(|e| e.to_string())
}

pub(crate) fn write_standard_sidecars(
    f: &Frame,
    primary: &Path,
    raw: &Path,
    conditions: Option<&Path>,
    sidecars: &[PathBuf; 4],
    mut conditions_data: serde_json::Value,
) -> Result<(), String> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let raw_name = filename(raw)?;
    let primary_name = filename(primary)?;
    let names = sidecars
        .each_ref()
        .map(|path| filename(path))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let raw_hash = sha256(&f.raw);
    let (image_width, image_height) =
        image::image_dimensions(primary).map_err(|e| e.to_string())?;
    let timestamp_semantics = crate::measurement::timestamp_semantics(&f.metadata);
    let acquisition = serde_json::to_value(&f.metadata.acquisition).map_err(|e| e.to_string())?;
    let mut summary = csv::Writer::from_writer(Vec::new());
    summary
        .write_record([
            "run_id",
            "frame_sequence",
            "source",
            "simulated",
            "frame_timestamp_unix_ms",
            "raw_format",
            "sensor_width_px",
            "sensor_height_px",
            "image_width_px",
            "image_height_px",
            "timestamp_semantics",
            "image_filename",
            "raw_payload_filename",
            "raw_payload_sha256",
        ])
        .map_err(|e| e.to_string())?;
    summary
        .write_record([
            &run_id,
            &f.metadata.sequence.to_string(),
            &f.metadata.source,
            &f.metadata.simulated.to_string(),
            &f.metadata.unix_millis.to_string(),
            &f.metadata.format,
            &f.metadata.width.to_string(),
            &f.metadata.height.to_string(),
            &image_width.to_string(),
            &image_height.to_string(),
            timestamp_semantics,
            &primary_name,
            &raw_name,
            &raw_hash,
        ])
        .map_err(|e| e.to_string())?;
    let summary = summary.into_inner().map_err(|e| e.to_string())?;
    let mut raw_summary = csv::Writer::from_writer(Vec::new());
    raw_summary
        .write_record([
            "run_id",
            "payload_filename",
            "payload_sha256",
            "payload_bytes",
            "raw_format",
            "sensor_width_px",
            "sensor_height_px",
            "frame_timestamp_unix_ms",
            "simulated",
            "acquisition_settings_json",
        ])
        .map_err(|e| e.to_string())?;
    raw_summary
        .write_record([
            &run_id,
            &raw_name,
            &raw_hash,
            &f.raw.len().to_string(),
            &f.metadata.format,
            &f.metadata.width.to_string(),
            &f.metadata.height.to_string(),
            &f.metadata.unix_millis.to_string(),
            &f.metadata.simulated.to_string(),
            &acquisition.to_string(),
        ])
        .map_err(|e| e.to_string())?;
    let raw_summary = raw_summary.into_inner().map_err(|e| e.to_string())?;
    let bundle = serde_json::json!({"schema":"kohdalab_run_bundle_v1","run_id":run_id,"filenames":{"analysis_csv":names[0],"raw_csv":names[1],"log":names[2],"conditions_json":names[3]},"image":{"native_payload":primary_name,"raw_payload":raw_name,"raw_payload_sha256":raw_hash,"raw_payload_bytes":f.raw.len()}});
    let conditions_object = conditions_data
        .as_object_mut()
        .ok_or("Camera conditions must be a JSON object")?;
    conditions_object.insert(
        "frame".into(),
        serde_json::to_value(&f.metadata).map_err(|e| e.to_string())?,
    );
    conditions_object.insert("observed_acquisition".into(), acquisition);
    conditions_object.insert(
        "host_timestamp".into(),
        serde_json::json!({"timestamp_ms":f.metadata.unix_millis,"semantics":timestamp_semantics}),
    );
    conditions_object.insert("simulation".into(), serde_json::json!(f.metadata.simulated));
    conditions_object.insert("run_bundle".into(), bundle.clone());
    conditions_object.insert("run_id".into(), serde_json::json!(run_id));
    conditions_object.insert(
        "csv_schema".into(),
        serde_json::json!("camera_frame_summary_v1"),
    );
    conditions_object.insert(
        "raw_schema".into(),
        serde_json::json!("camera_raw_payload_v1"),
    );
    conditions_object.insert("log".into(), serde_json::json!({"schema":"kohdalab_run_log_v1","timestamp_unit":"unix_ms","filename":names[2]}));
    conditions_object.insert("application".into(), serde_json::json!("kohdalab-camera"));
    conditions_object.insert(
        "application_version".into(),
        serde_json::json!(env!("CARGO_PKG_VERSION")),
    );
    conditions_object.insert(
        "image_dimensions".into(),
        serde_json::json!([image_width, image_height]),
    );
    write_new(&sidecars[0], &summary)?;
    write_new(&sidecars[1], &raw_summary)?;
    let saved_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let log = format!(
        "{saved_unix_ms}\tINFO\t{run_id}\tframe_saved\t{}\n",
        serde_json::json!({"clock":"host_unix_ms","image":primary_name,"simulation":f.metadata.simulated,"frame_timestamp_semantics":timestamp_semantics})
    );
    write_new(&sidecars[2], log.as_bytes())?;
    if let Some(conditions) = conditions {
        write_new(
            conditions,
            &serde_json::to_vec_pretty(&conditions_data).map_err(|e| e.to_string())?,
        )?;
    }
    conditions_data["status"] = serde_json::json!("complete");
    conditions_data["file_hashes"] = serde_json::json!({
        "csv_sha256":sha256(&summary), "raw_csv_sha256":sha256(&raw_summary),
        "log_sha256":sha256(log.as_bytes()), "image_sha256":file_sha256(primary)?,
        "raw_payload_sha256":raw_hash,
    });
    if let Some(conditions) = conditions {
        conditions_data["legacy_conditions"] =
            serde_json::json!({"filename":filename(conditions)?,"sha256":file_sha256(conditions)?});
    }
    write_new(
        &sidecars[3],
        &serde_json::to_vec_pretty(&conditions_data).map_err(|e| e.to_string())?,
    )
}

fn file_sha256(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
/// Existing primary/sidecars are refused. Dialog cancellation never reaches this function.
/// A same-directory hard link publishes a fully encoded primary atomically without overwrite.
/// Sidecar failure retains the valid primary and reports partial completion explicitly.
pub fn save(f: &Frame, path: &Path) -> Result<(), String> {
    save_recipe(f, path, crate::display_recipe::Recipe::default())
}
pub fn save_recipe(
    f: &Frame,
    path: &Path,
    recipe: crate::display_recipe::Recipe,
) -> Result<(), String> {
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
    let sidecars = standard_sidecars(path);
    if std::iter::once(path)
        .chain([raw.as_path(), metadata.as_path(), original.as_path()])
        .chain(sidecars.iter().map(PathBuf::as_path))
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
        let (width, height, pixels) = recipe.render(f, None)?;
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
                .open(&raw)
                .map_err(|e| e.to_string())?;
            out.write_all(f.raw.as_slice())
                .and_then(|_| out.sync_all())
                .map_err(|e| e.to_string())?;
            let original_info = if f.metadata.format == "BayerGRBG8" {
                let file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&original)
                    .map_err(|e| e.to_string())?;
                let mut writer = std::io::BufWriter::new(file);
                image::write_buffer_with_format(
                    &mut writer,
                    &f.raw,
                    f.metadata.width,
                    f.metadata.height,
                    image::ColorType::L8,
                    image::ImageFormat::Tiff,
                )
                .map_err(|e| e.to_string())?;
                writer
                    .flush()
                    .and_then(|_| writer.get_ref().sync_all())
                    .map_err(|e| e.to_string())?;
                serde_json::json!({"available":true,"path":original,"format":"TIFF L8 BayerGRBG8","dimensions":[f.metadata.width,f.metadata.height],"orientation":"unchanged sensor RAW","data_unchanged":true})
            } else {
                serde_json::json!({"available":false,"reason":"source RGB8; no original Bayer sensor data"})
            };
            write_standard_sidecars(
                f,
                path,
                &raw,
                Some(&metadata),
                &sidecars,
                serde_json::json!({"display_recipe":recipe,"processing":recipe.processing(&f.metadata.format),"original_sensor_tiff":original_info,"image_dimensions":[width,height],"orientation":"software reflections then clockwise quarter turns; RAW orientation unchanged","color_assumptions":"Declared GRBG software reconstruction or source RGB8; software arithmetic on encoded channel values; no calibrated color/gamma","raw_preserved":true,"exposure_time_seconds":null,"timestamp_semantics":crate::measurement::timestamp_semantics(&f.metadata),"recording_contract":recording_contract(f)}),
            )
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

/// Describe preserved bytes separately from sensor precision and host timing.
/// In particular a host sequence is not evidence of a sensor frame counter.
fn recording_contract(f: &Frame) -> serde_json::Value {
    let geometry = f.metadata.acquisition.as_ref().map(|a| &a.geometry);
    serde_json::json!({
        "version": 1,
        "software": {"application":"kohdalab-camera", "version":env!("CARGO_PKG_VERSION"),
            "source_identity":include_str!("../../../RECORDER-SOURCE-ID").trim()},
        "raw": {"meaning":"exact_received_payload_before_software_display_processing",
            "format":f.metadata.format,"bytes":f.raw.len(),"stored_bits_per_component":8,
            "declared_sensor_bits":geometry.map(|g|g.sensor_bits),
            "declared_transport_bits":geometry.map(|g|g.transport_bits),
            "full_sensor_precision_preserved": geometry.map(|g| g.sensor_bits == g.transport_bits),
            "simulated":f.metadata.simulated},
        "timing": {"timestamp_semantics":crate::measurement::timestamp_semantics(&f.metadata),
            "exposure_start_utc":null,"exposure_duration_seconds":null,
            "measured_sensor_clock_hz":null,"sensor_frame_counter":null,"sensor_dropped_frames":null,
            "sequence_semantics":"host_accepted_frame_sequence_not_sensor_counter",
            "unknown_values":"null means not observed, never zero"}
    })
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
                measurement: None,
                unix_millis: 0,
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
            let metadata: serde_json::Value =
                serde_json::from_slice(&fs::read(suffix(&path, ".json")).unwrap()).unwrap();
            let manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(suffix(&path, ".run.json")).unwrap()).unwrap();
            let contract = &metadata["recording_contract"];
            assert_eq!(contract["raw"]["stored_bits_per_component"], 8);
            assert!(contract["raw"]["declared_sensor_bits"].is_null());
            assert!(contract["raw"]["full_sensor_precision_preserved"].is_null());
            assert!(contract["timing"]["sensor_dropped_frames"].is_null());
            assert!(contract["timing"]["exposure_start_utc"].is_null());
            assert_eq!(contract["software"]["version"], env!("CARGO_PKG_VERSION"));
            assert_eq!(manifest["run_bundle"]["schema"], "kohdalab_run_bundle_v1");
            assert_eq!(manifest["status"], "complete");
            assert_eq!(
                manifest["run_bundle"]["filenames"]["conditions_json"],
                filename(&suffix(&path, ".run.json")).unwrap()
            );
            assert_eq!(manifest["frame"]["format"], "BayerGRBG8");
            assert!(
                manifest["host_timestamp"]["semantics"]
                    .as_str()
                    .unwrap()
                    .contains("unspecified")
            );
            for (extension, key) in [
                (".csv", "csv_sha256"),
                (".raw.csv", "raw_csv_sha256"),
                (".log", "log_sha256"),
            ] {
                assert_eq!(
                    manifest["file_hashes"][key],
                    file_sha256(&suffix(&path, extension)).unwrap()
                );
            }
            assert_eq!(
                manifest["file_hashes"]["image_sha256"],
                file_sha256(&path).unwrap()
            );
            assert_eq!(
                manifest["run_bundle"]["image"]["raw_payload"],
                "chosen.png.raw".replace("png", ext)
            );
            assert_eq!(
                manifest["run_bundle"]["image"]["raw_payload_sha256"],
                sha256(&f.raw)
            );
            assert_eq!(metadata["host_timestamp"]["timestamp_ms"], 0);
            assert_eq!(
                fs::read_to_string(suffix(&path, ".csv"))
                    .unwrap()
                    .lines()
                    .next(),
                Some(
                    "run_id,frame_sequence,source,simulated,frame_timestamp_unix_ms,raw_format,sensor_width_px,sensor_height_px,image_width_px,image_height_px,timestamp_semantics,image_filename,raw_payload_filename,raw_payload_sha256"
                )
            );
            assert!(
                save_recipe(&f, &path, crate::display_recipe::Recipe::legacy())
                    .unwrap_err()
                    .contains("Existing")
            );
        }
        assert!(
            save_recipe(
                &f,
                &root.join("missing/image.png"),
                crate::display_recipe::Recipe::legacy()
            )
            .is_err()
        );
        assert!(!root.join("missing/image.png").exists());
        #[cfg(unix)]
        {
            // Primary basename fits NAME_MAX; its RAW sidecar does not.
            let path = root.join(format!("{}.png", "x".repeat(248)));
            assert!(
                save_recipe(&f, &path, crate::display_recipe::Recipe::legacy())
                    .unwrap_err()
                    .contains("sidecars INCOMPLETE")
            );
            assert!(path.is_file());
            assert_eq!(image::open(path).unwrap().to_rgb8().as_raw(), &expected);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn finite_acquisition_contract_does_not_claim_missing_sensor_bits_or_clock() {
        let settings = crate::controls::Settings::default();
        let acquisition =
            crate::sensor_contract::Acquisition::new(settings, [1; 32], 1, 1).unwrap();
        let frame = Frame {
            raw: std::sync::Arc::new(vec![]),
            metadata: crate::Metadata {
                acquisition: Some(acquisition),
                source: "contract test".into(),
                simulated: true,
                sequence: 1,
                measurement: None,
                unix_millis: 0,
                width: 2,
                height: 2,
                format: "BayerGRBG8".into(),
            },
        };
        let contract = recording_contract(&frame);
        assert_eq!(contract["raw"]["declared_sensor_bits"], 10);
        assert_eq!(contract["raw"]["declared_transport_bits"], 8);
        assert_eq!(contract["raw"]["full_sensor_precision_preserved"], false);
        assert!(contract["timing"]["measured_sensor_clock_hz"].is_null());
        assert!(contract["timing"]["exposure_duration_seconds"].is_null());
    }
}

#[cfg(test)]
mod correction_v032 {
    use super::*;
    #[test]
    fn frozen_asymmetric_bayer_recipe_original_and_metadata() {
        let root = std::env::temp_dir().join(format!("save-v032-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let mut live = Frame {
            raw: std::sync::Arc::new(vec![3, 19, 47, 83, 101, 29, 157, 61]),
            metadata: crate::Metadata {
                acquisition: None,
                source: "own asymmetric held Bayer".into(),
                simulated: true,
                sequence: 32017,
                measurement: None,
                unix_millis: 123456789,
                width: 4,
                height: 2,
                format: "BayerGRBG8".into(),
            },
        };
        let mut draft = crate::display_recipe::Recipe {
            ev: 1.,
            gamma: 2.,
            wb: [0.5, 1.25, 0.75],
            mirror: true,
            quarter_turns: 1,
            ..crate::display_recipe::Recipe::neutral()
        };
        let held = live.clone();
        let frozen = draft;
        // Independent offline scalar oracle: explicit GRBG neighborhoods and
        // reflected edges; source order [7,3,6,2,5,1,4,0]; gain 2, sqrt gamma.
        let expected: Vec<u8> = vec![
            145, 197, 245, 145, 186, 245, 114, 171, 245, 114, 173, 245, 70, 136, 222, 70, 131, 222,
            70, 101, 197, 70, 44, 197,
        ];
        let check = |p: &Path| {
            let processed = image::open(p).unwrap();
            assert_eq!(processed.color(), image::ColorType::Rgb8);
            assert_eq!((processed.width(), processed.height()), (2, 4));
            assert_eq!(processed.to_rgb8().into_raw(), expected);
            let original_path = suffix(p, ".original-RAW.tiff");
            let original = image::open(&original_path).unwrap();
            assert_eq!(original.color(), image::ColorType::L8);
            assert_eq!((original.width(), original.height()), (4, 2));
            assert_eq!(original.to_luma8().as_raw(), held.raw.as_ref());
            assert_eq!(fs::read(suffix(p, ".raw")).unwrap(), *held.raw);
            let m: serde_json::Value =
                serde_json::from_slice(&fs::read(suffix(p, ".json")).unwrap()).unwrap();
            assert_eq!(m["frame"], serde_json::to_value(&held.metadata).unwrap());
            assert_eq!(m["frame"]["source"], "own asymmetric held Bayer");
            assert_eq!(m["frame"]["sequence"], 32017);
            assert_eq!(m["frame"]["format"], "BayerGRBG8");
            assert_eq!(m["display_recipe"], serde_json::to_value(frozen).unwrap());
            assert_eq!(m["image_dimensions"], serde_json::json!([2, 4]));
            assert_eq!(
                m["original_sensor_tiff"],
                serde_json::json!({"available":true,"path":original_path,"format":"TIFF L8 BayerGRBG8","dimensions":[4,2],"orientation":"unchanged sensor RAW","data_unchanged":true})
            );
            assert_eq!(
                m["processing"],
                "GRBG-bilinear-reflect101-u16-half-up; software recipe/v2; channel-gain-clamp-gamma-LUT256; mono-post-gamma-half-up; processed-RGB-box-u64-half-up; terminal-box-clipped"
            );
            assert_eq!(m["raw_preserved"], true);
            assert_eq!(
                m["orientation"],
                "software reflections then clockwise quarter turns; RAW orientation unchanged"
            );
        };
        for ext in ["png", "tiff"] {
            let before = root.join(format!("before.{ext}"));
            save_recipe(&held, &before, frozen).unwrap();
            check(&before);
            draft = crate::display_recipe::Recipe {
                ev: -2.,
                gamma: 0.5,
                flip: true,
                quarter_turns: 3,
                ..draft
            };
            live.raw = std::sync::Arc::new(vec![211; 8]);
            live.metadata.sequence += 1;
            live.metadata.source = "new live source".into();
            assert_ne!(draft, frozen);
            assert_ne!(live.raw, held.raw);
            assert_ne!(live.metadata, held.metadata);
            let after = root.join(format!("after.{ext}"));
            save_recipe(&held, &after, frozen).unwrap();
            check(&after);
            for ending in ["", ".raw", ".original-RAW.tiff"] {
                assert_eq!(
                    fs::read(suffix(&before, ending)).unwrap(),
                    fs::read(suffix(&after, ending)).unwrap()
                );
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod brightness_export_v033 {
    use super::*;
    #[test]
    fn default_multiplier_frozen_in_export() {
        let root = std::env::temp_dir().join(format!("default-v033-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let f = Frame {
            raw: std::sync::Arc::new(vec![10, 20, 30, 40, 50, 60]),
            metadata: crate::Metadata {
                acquisition: None,
                source: "own RGB fixture".into(),
                simulated: true,
                sequence: 1,
                measurement: None,
                unix_millis: 0,
                width: 2,
                height: 1,
                format: "RGB8".into(),
            },
        };
        let mut draft = crate::display_recipe::Recipe::default();
        let frozen = draft;
        draft.set_brightness(2.);
        assert_ne!(draft, frozen);
        let path = root.join("default.png");
        save_recipe(&f, &path, frozen).unwrap();
        let m: serde_json::Value =
            serde_json::from_slice(&fs::read(suffix(&path, ".json")).unwrap()).unwrap();
        let stored: crate::display_recipe::Recipe =
            serde_json::from_value(m["display_recipe"].clone()).unwrap();
        assert_eq!(stored, frozen);
        assert!((stored.brightness() - 1.2).abs() < 1e-6);
        assert_eq!(m["image_dimensions"], serde_json::json!([2, 1]));
        // Independently computed round(255 * (channel / 255 * 1.2)^(1/2.2)), rotated 180°.
        assert_eq!(
            image::open(path).unwrap().to_rgb8().into_raw(),
            vec![119, 132, 144, 64, 87, 105]
        );
        fs::remove_dir_all(root).unwrap();
    }
}
