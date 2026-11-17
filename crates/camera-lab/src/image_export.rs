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
    // Unknown time is JSON null and an empty CSV cell; never epoch zero.
    let timestamp_ms = crate::measurement::saved_timestamp_ms(&f.metadata);
    let timestamp_cell = timestamp_ms.map(|t| t.to_string()).unwrap_or_default();
    let timestamp_status = crate::measurement::timestamp_status(&f.metadata);
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
            &timestamp_cell,
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
            &timestamp_cell,
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
        crate::measurement::saved_metadata(&f.metadata)?,
    );
    conditions_object.insert("observed_acquisition".into(), acquisition);
    conditions_object.insert(
        "host_timestamp".into(),
        serde_json::json!({"timestamp_ms":timestamp_ms,"semantics":timestamp_semantics,
            "status":timestamp_status,
            "status_meaning":crate::measurement::timestamp_status_meaning(timestamp_status),
            "unknown_values":"null timestamp_ms and empty CSV frame_timestamp_unix_ms mean not observed, never epoch zero"}),
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
        serde_json::json!({"clock":"host_unix_ms","image":primary_name,"simulation":f.metadata.simulated,"frame_timestamp_unix_ms":timestamp_ms,"frame_timestamp_status":timestamp_status,"frame_timestamp_semantics":timestamp_semantics})
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
/// Publishes a completed, already closed same-directory temporary file at a
/// destination that must not exist. `Ok(true)` means the temporary name still
/// exists and must be removed by the caller; `Ok(false)` means it was consumed.
/// Every error leaves an existing destination untouched.
pub(crate) type Publish = fn(&Path, &Path) -> std::io::Result<bool>;

/// Platform no-overwrite publication.
///
/// Windows: `MoveFileExW` with flags 0 (no replace, no cross-volume copy); it
/// needs no hard-link support. macOS/Linux: same-directory hard link, which
/// fails with `AlreadyExists` instead of replacing.
pub(crate) fn publish_no_replace(temp: &Path, destination: &Path) -> std::io::Result<bool> {
    #[cfg(windows)]
    {
        capability_platform::publish_new_file(temp, destination).map(|()| false)
    }
    #[cfg(not(windows))]
    {
        fs::hard_link(temp, destination).map(|()| true)
    }
}

/// Unix errors that mean the destination file system cannot create hard links.
#[cfg(not(windows))]
fn hard_link_unsupported(error: &std::io::Error) -> bool {
    // EPERM (FAT/exFAT on Linux), EOPNOTSUPP/ENOTSUP (Linux 95; macOS 45, 102).
    #[cfg(target_os = "linux")]
    const CODES: &[i32] = &[1, 95];
    #[cfg(not(target_os = "linux"))]
    const CODES: &[i32] = &[1, 45, 102];
    error.kind() == std::io::ErrorKind::Unsupported
        || error.raw_os_error().is_some_and(|c| CODES.contains(&c))
}

fn publication_error(label: &str, destination: &Path, error: &std::io::Error) -> String {
    let base = format!("{label} publication failed (no overwrite): {error}");
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        return format!(
            "{base}. Another file appeared at {} during saving; it was kept unchanged and nothing was published under that name. Choose a new name.",
            destination.display()
        );
    }
    #[cfg(not(windows))]
    {
        if hard_link_unsupported(error) {
            return format!(
                "{base}. This folder's file system does not support the hard links used on this OS to publish without overwriting (for example FAT/exFAT, some network or cloud-synced folders). Nothing was published and no file was replaced. Save to a folder on the system disk (APFS/HFS+ on macOS; ext4, XFS or Btrfs on Linux), then copy the image together with all its sidecars."
            );
        }
    }
    format!("{base}. Nothing was published and existing files are unchanged.")
}

/// Encode into a new hidden same-directory temporary file, flush, sync and
/// close it, then publish it without overwrite. An incomplete encoding is
/// never visible under `destination`; the temporary file is always removed.
fn publish_encoded(
    destination: &Path,
    label: &str,
    publish: Publish,
    encode: impl FnOnce(&mut std::io::BufWriter<fs::File>) -> Result<(), String>,
) -> Result<(), String> {
    let temp = destination.parent().unwrap_or(Path::new(".")).join(format!(
        ".camera-lab-save-{}-{}.pending",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let mut remove_temp = true;
    let result = (|| {
        let mut writer = std::io::BufWriter::new(file);
        encode(&mut writer)?;
        let file = writer.into_inner().map_err(|e| e.error().to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        // Close before publication: no writer handle survives into the
        // published name, and only fully flushed, synced bytes are moved.
        drop(file);
        match publish(&temp, destination) {
            Ok(remains) => {
                remove_temp = remains;
                Ok(())
            }
            Err(e) => Err(publication_error(label, destination, &e)),
        }
    })();
    if remove_temp {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Existing primary/sidecars are refused. Dialog cancellation never reaches this function.
/// A fully encoded, closed primary is published without overwrite (see `publish_no_replace`).
/// Sidecar failure retains the valid primary and reports partial completion explicitly.
pub fn save(f: &Frame, path: &Path) -> Result<(), String> {
    save_recipe(f, path, crate::display_recipe::Recipe::default())
}
pub fn save_recipe(
    f: &Frame,
    path: &Path,
    recipe: crate::display_recipe::Recipe,
) -> Result<(), String> {
    save_recipe_with(f, path, recipe, publish_no_replace)
}
pub(crate) fn save_recipe_with(
    f: &Frame,
    path: &Path,
    recipe: crate::display_recipe::Recipe,
    publish: Publish,
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
    let (width, height, pixels) = recipe.render(f, None)?;
    publish_encoded(path, "Primary", publish, |writer| {
        image::write_buffer_with_format(
            writer,
            &pixels,
            width,
            height,
            image::ColorType::Rgb8,
            format,
        )
        .map_err(|e| e.to_string())
    })?;
    drop(pixels);
    let written = (|| -> Result<(), String> {
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&raw)
            .map_err(|e| e.to_string())?;
        out.write_all(f.raw.as_slice())
            .and_then(|_| out.sync_all())
            .map_err(|e| e.to_string())?;
        let original_info = if f.metadata.format == "BayerGRBG8" {
            // Same closed-temporary no-overwrite publication as the primary:
            // a partially encoded original sensor TIFF is never visible.
            publish_encoded(&original, "Original sensor TIFF", publish, |writer| {
                image::write_buffer_with_format(
                    writer,
                    &f.raw,
                    f.metadata.width,
                    f.metadata.height,
                    image::ColorType::L8,
                    image::ImageFormat::Tiff,
                )
                .map_err(|e| e.to_string())
            })?;
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
    written.map_err(|e| {
        format!(
            "PRIMARY IMAGE SAVED at {}; sidecars INCOMPLETE: {e}",
            path.display()
        )
    })
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
            "host_timestamp_unix_ms":crate::measurement::saved_timestamp_ms(&f.metadata),
            "host_timestamp_status":crate::measurement::timestamp_status(&f.metadata),
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
            // Unknown (unix_millis 0 without observation) is null, never epoch zero.
            assert!(metadata["host_timestamp"]["timestamp_ms"].is_null());
            assert_eq!(metadata["host_timestamp"]["status"], "unknown");
            assert!(metadata["frame"]["unix_millis"].is_null());
            assert!(manifest["frame"]["unix_millis"].is_null());
            assert!(manifest["host_timestamp"]["timestamp_ms"].is_null());
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

#[cfg(test)]
mod p1_timestamp_and_publication {
    use super::*;
    use std::{io, sync::Arc};

    fn fresh(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("p1-{name}-{}", std::process::id()));
        fs::create_dir(&p).unwrap();
        p
    }
    fn bayer(width: u32, height: u32, unix_millis: u128, simulated: bool) -> Frame {
        Frame {
            raw: Arc::new(
                (0..width as usize * height as usize)
                    .map(|i| (i * 37 % 251) as u8)
                    .collect(),
            ),
            metadata: crate::Metadata {
                acquisition: None,
                source: "P1 own fixture".into(),
                simulated,
                sequence: 7,
                measurement: None,
                unix_millis,
                width,
                height,
                format: "BayerGRBG8".into(),
            },
        }
    }
    fn json(path: &Path) -> serde_json::Value {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }
    fn csv_cell(path: &Path, column: &str) -> String {
        let mut reader = csv::Reader::from_path(path).unwrap();
        let index = reader
            .headers()
            .unwrap()
            .iter()
            .position(|h| h == column)
            .unwrap();
        let record = reader.records().next().unwrap().unwrap();
        record[index].to_owned()
    }
    fn outputs(path: &Path) -> Vec<PathBuf> {
        let mut all = vec![
            path.to_path_buf(),
            suffix(path, ".raw"),
            suffix(path, ".json"),
            suffix(path, ".original-RAW.tiff"),
        ];
        all.extend(standard_sidecars(path));
        all
    }
    fn pending(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".pending"))
            .collect()
    }

    #[test]
    fn unknown_time_is_null_and_empty_in_every_saved_record() {
        let root = fresh("timestamps");
        // Verified observation: real binding path from measurement::Source.
        let mut observed = Frame {
            raw: Arc::new(vec![1; 1024 * 768]),
            ..bayer(1024, 768, 0, false)
        };
        observed.metadata.acquisition = Some(
            crate::sensor_contract::Acquisition::new(
                crate::controls::Settings::default(),
                [7; 32],
                1,
                1,
            )
            .unwrap(),
        );
        observed.metadata.sequence = 1;
        let mut source = crate::measurement::Source::default();
        source.complete();
        source.bind(&mut observed);
        let observed_ms = observed.metadata.unix_millis;
        assert_ne!(observed_ms, 0);
        // Observation present but its clock is missing: unknown, not zero.
        let mut clock_missing = observed.clone();
        clock_missing
            .metadata
            .measurement
            .as_mut()
            .unwrap()
            .host_accepted_completion_unix_ms = None;
        clock_missing.metadata.unix_millis = 0;
        let simulation =
            crate::CameraAdapter::next_frame(&mut crate::Simulation { sequence: 0 }).unwrap();
        let simulation_ms = simulation.metadata.unix_millis;
        assert_ne!(simulation_ms, 0);
        let legacy_ms = 1_700_000_000_123;
        let cases = [
            (
                "observed",
                observed,
                Some(observed_ms),
                "observed_host_accepted_completion",
            ),
            ("clock-missing", clock_missing, None, "unknown"),
            ("unknown", bayer(4, 2, 0, true), None, "unknown"),
            (
                "simulation",
                simulation,
                Some(simulation_ms),
                "simulation_host_clock_not_observed",
            ),
            (
                "legacy",
                bayer(4, 2, legacy_ms, false),
                Some(legacy_ms),
                "legacy_unverified",
            ),
        ];
        for (name, f, expected, status) in cases {
            let path = root.join(format!("{name}.png"));
            save_recipe(&f, &path, crate::display_recipe::Recipe::neutral()).unwrap();
            let value = expected.map_or(serde_json::Value::Null, |t| serde_json::json!(t));
            let cell = expected.map(|t| t.to_string()).unwrap_or_default();
            for document in [
                json(&suffix(&path, ".json")),
                json(&suffix(&path, ".run.json")),
            ] {
                assert_eq!(document["frame"]["unix_millis"], value, "{name}");
                assert_eq!(document["host_timestamp"]["timestamp_ms"], value, "{name}");
                assert_eq!(document["host_timestamp"]["status"], status, "{name}");
                assert!(document["host_timestamp"]["status_meaning"].is_string());
                assert_eq!(
                    document["recording_contract"]["timing"]["host_timestamp_unix_ms"], value,
                    "{name}"
                );
                // Existing readers: saved frame still deserializes; null -> 0 sentinel.
                let back: crate::Metadata =
                    serde_json::from_value(document["frame"].clone()).unwrap();
                assert_eq!(back.unix_millis, f.metadata.unix_millis, "{name}");
            }
            for csv in [suffix(&path, ".csv"), suffix(&path, ".raw.csv")] {
                assert_eq!(csv_cell(&csv, "frame_timestamp_unix_ms"), cell, "{name}");
            }
            let log = fs::read_to_string(suffix(&path, ".log")).unwrap();
            let log: serde_json::Value =
                serde_json::from_str(log.trim_end().rsplit('\t').next().unwrap()).unwrap();
            assert_eq!(log["frame_timestamp_unix_ms"], value, "{name}");
            assert_eq!(log["frame_timestamp_status"], status, "{name}");
        }
        // Legacy bundle export: metadata.json and run.json frame use null too.
        let unknown = bayer(4, 2, 0, true);
        crate::export(&unknown, &root.join("bundle.png")).unwrap();
        let bundle = root.join("bundle.png.capture");
        let legacy = json(&bundle.join("metadata.json"));
        assert!(legacy["unix_millis"].is_null());
        let back: crate::Metadata = serde_json::from_value(legacy).unwrap();
        assert_eq!(back, unknown.metadata);
        assert!(json(&bundle.join("bundle.png.run.json"))["frame"]["unix_millis"].is_null());
        assert_eq!(
            csv_cell(&bundle.join("bundle.png.csv"), "frame_timestamp_unix_ms"),
            ""
        );
        // Existing numeric inputs remain readable; null is the new unknown form.
        for (text, expected) in [("0", 0), ("null", 0), ("123", 123)] {
            let input = format!(
                r#"{{"source":"input","simulated":false,"sequence":1,"unix_millis":{text},"width":2,"height":2,"format":"BayerGRBG8"}}"#
            );
            let m: crate::Metadata = serde_json::from_str(&input).unwrap();
            assert_eq!(m.unix_millis, expected);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn publish_no_replace_platform_collision_and_success() {
        // On Windows this exercises MoveFileExW(flags 0); elsewhere hard links.
        let root = fresh("publish");
        let temp = root.join("a.pending");
        let destination = root.join("a.png");
        fs::write(&temp, b"complete").unwrap();
        let remains = publish_no_replace(&temp, &destination).unwrap();
        assert_eq!(remains, cfg!(not(windows)));
        assert_eq!(temp.exists(), remains);
        assert_eq!(fs::read(&destination).unwrap(), b"complete");
        let _ = fs::remove_file(&temp);
        fs::write(&temp, b"second").unwrap();
        let error = publish_no_replace(&temp, &destination).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&destination).unwrap(), b"complete");
        assert_eq!(fs::read(&temp).unwrap(), b"second");
        let text = publication_error("Primary", &destination, &error);
        assert!(text.starts_with("Primary publication failed (no overwrite)"));
        assert!(text.contains("kept unchanged"));
        #[cfg(not(windows))]
        {
            for code in [1, if cfg!(target_os = "linux") { 95 } else { 45 }] {
                let text =
                    publication_error("Primary", &destination, &io::Error::from_raw_os_error(code));
                assert!(text.contains("does not support the hard links"), "{code}");
                assert!(text.contains("no file was replaced"));
            }
        }
        fs::remove_dir_all(root).unwrap();
    }

    fn racing(temp: &Path, destination: &Path) -> io::Result<bool> {
        // Another writer creates the chosen name after the existence check.
        fs::write(destination, b"RACE-WINNER")?;
        publish_no_replace(temp, destination)
    }
    fn refused(_: &Path, _: &Path) -> io::Result<bool> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn refuse_original(temp: &Path, destination: &Path) -> io::Result<bool> {
        if destination
            .to_string_lossy()
            .ends_with(".original-RAW.tiff")
        {
            Err(io::ErrorKind::Unsupported.into())
        } else {
            publish_no_replace(temp, destination)
        }
    }

    #[test]
    fn save_never_overwrites_and_cleans_temporaries_normal_and_full() {
        for (name, width, height) in [("normal", 1024, 768), ("full", 2048, 1536)] {
            let root = fresh(&format!("save-{name}"));
            let f = bayer(width, height, 0, true);
            let recipe = crate::display_recipe::Recipe::neutral();
            // Every pre-existing original/sidecar is refused and left unchanged.
            for (i, existing) in outputs(&root.join("x.png")).into_iter().enumerate() {
                let dir = root.join(format!("existing-{i}"));
                fs::create_dir(&dir).unwrap();
                let path = dir.join("x.png");
                let existing = dir.join(existing.file_name().unwrap());
                fs::write(&existing, b"KEEP").unwrap();
                assert!(
                    save_recipe(&f, &path, recipe)
                        .unwrap_err()
                        .contains("Existing")
                );
                assert_eq!(fs::read(&existing).unwrap(), b"KEEP");
                for other in outputs(&path).into_iter().filter(|p| *p != existing) {
                    assert!(!other.exists(), "{}", other.display());
                }
                assert!(pending(&dir).is_empty());
            }
            // Ordinary success: complete outputs and no temporary residue.
            let ok = root.join("ok.png");
            save_recipe(&f, &ok, recipe).unwrap();
            assert_eq!(image::image_dimensions(&ok).unwrap(), (width, height));
            assert_eq!(
                image::open(suffix(&ok, ".original-RAW.tiff"))
                    .unwrap()
                    .to_luma8()
                    .as_raw(),
                f.raw.as_ref()
            );
            assert_eq!(json(&suffix(&ok, ".run.json"))["status"], "complete");
            assert!(outputs(&ok).iter().all(|p| p.is_file()));
            assert!(pending(&root).is_empty());
            // A competing file at publication time is kept; nothing else appears.
            let raced = root.join("raced.png");
            let error = save_recipe_with(&f, &raced, recipe, racing).unwrap_err();
            assert!(error.contains("kept unchanged"), "{error}");
            assert_eq!(fs::read(&raced).unwrap(), b"RACE-WINNER");
            assert!(outputs(&raced).iter().skip(1).all(|p| !p.exists()));
            // Publication failure (e.g. hard links unsupported): nothing published.
            let failed = root.join("failed.png");
            let error = save_recipe_with(&f, &failed, recipe, refused).unwrap_err();
            assert!(error.starts_with("Primary publication failed (no overwrite)"));
            assert!(error.contains("Nothing was published"), "{error}");
            #[cfg(not(windows))]
            {
                assert!(error.contains("does not support the hard links"), "{error}");
            }
            assert!(outputs(&failed).iter().all(|p| !p.exists()));
            // Original TIFF publication failure after the primary: explicit partial.
            let partial = root.join("partial.png");
            let error = save_recipe_with(&f, &partial, recipe, refuse_original).unwrap_err();
            assert!(error.contains("PRIMARY IMAGE SAVED"), "{error}");
            assert!(error.contains("Original sensor TIFF publication failed"));
            assert_eq!(image::image_dimensions(&partial).unwrap(), (width, height));
            assert!(!suffix(&partial, ".original-RAW.tiff").exists());
            assert!(!suffix(&partial, ".run.json").exists());
            assert!(pending(&root).is_empty(), "{:?}", pending(&root));
            fs::remove_dir_all(root).unwrap();
        }
    }
}
