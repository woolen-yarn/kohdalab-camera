//! Bounded subprocess adapter: USB lifetime and cleanup stay outside the UI.
use crate::{Frame, Metadata};
use std::{
    io::Read,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Arc,
};
pub struct PhysicalPreview {
    child: Child,
    pub directory: PathBuf,
    width: u32,
    height: u32,
    sequence: u64,
    source: String,
    finished: bool,
}
impl PhysicalPreview {
    pub fn start(info: &nusb::DeviceInfo, full: bool) -> Result<Self, String> {
        Self::start_settings(
            info,
            crate::controls::Settings {
                mode: if full {
                    crate::controls::Mode::Full
                } else {
                    crate::controls::Mode::Preview
                },
                ..Default::default()
            },
        )
    }
    pub fn start_settings(
        info: &nusb::DeviceInfo,
        settings: crate::controls::Settings,
    ) -> Result<Self, String> {
        settings.validate()?;
        if settings.mode == crate::controls::Mode::FullFieldSkip {
            return Err("Legacy file helper cannot certify the versioned full-field skip contract".into());
        }
        let profile = settings.mode.profile();
        if info.vendor_id() != 0x0547
            || info.product_id() != 0x4d33
            || info.product_string() != Some("3.1M USB2.0 Camera")
            || info.device_version() != 0
            || info.serial_number().is_some()
        {
            return Err("Selected device has no experimental adapter".into());
        }
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        let directory = std::env::temp_dir().join(format!(
            "camera-lab-physical-{}-{millis}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let executable = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name(if cfg!(windows) {
                "physical-probe.exe"
            } else {
                "physical-probe"
            });
        let log = std::fs::File::create(directory.join("trial.log")).map_err(|e| e.to_string())?;
        let error = log.try_clone().map_err(|e| e.to_string())?;
        let mut command = Command::new(executable);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW; logs remain redirected.
        }
        let child = command
            .arg(
                info.port_chain()
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join("."),
            )
            .args(["--capture", profile.id])
            .arg(directory.join("capture"))
            .arg(info.bus_id())
            .arg(settings.exposure_rows.to_string())
            .arg(settings.gain_eighths.to_string())
            .env("CAMERA_LAB_PROGRESS", directory.join("progress.bin"))
            .env("TEMP", &directory)
            .env("TMP", &directory)
            .env("TMPDIR", &directory)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(error))
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            child,
            directory,
            width: u32::from(profile.width),
            height: u32::from(profile.height),
            sequence: 0,
            source: format!(
                "Physical 0547:4d33 MT9T001 bus={} port={:?}; upper 8 of 10 bits; exposure={} rows; gain={}/8; clock={:04x}; profile={}; window=row{},col{}; crop without binning/skipping",
                info.bus_id(),
                info.port_chain(),
                settings.exposure_rows,
                settings.gain_eighths,
                profile.clock,
                profile.id,
                profile.row_start,
                profile.column_start
            ),
            finished: false,
        })
    }
    pub fn poll(&mut self) -> Result<(Option<Frame>, Option<String>), String> {
        let mut status = None;
        if !self.finished
            && let Some(code) = self.child.try_wait().map_err(|e| e.to_string())?
        {
            self.finished = true;
            status = Some(format!(
                "Physical trial ended ({code}). Evidence: {}",
                self.directory.display()
            ));
            if !accepted_exit(code.code(), code.success(), &self.directory) {
                return Err(format!(
                    "Physical trial stopped ({code}); inspect {}. No simulation fallback. Do not retry if cleanup failed; reconnect physically.",
                    self.directory.join("trial.log").display()
                ));
            }
        }
        let path = self.directory.join("capture/live/preview.frame");
        let Some((sequence, timestamp, raw)) = read_record(
            &path,
            self.width as usize * self.height as usize,
            self.sequence,
        )?
        else {
            return Ok((None, status));
        };
        self.sequence = sequence;
        let frame = Frame {
            raw: Arc::new(raw),
            metadata: Metadata {
                acquisition: None,
source: self.source.clone(),
                simulated: false,
                sequence,
                measurement: None, unix_millis: timestamp as u128,
                width: self.width,
                height: self.height,
                format: "BayerGRBG8".into(),
            },
        };
        frame.validate()?;
        Ok((Some(frame), status))
    }
    pub fn progress(&self) -> Option<crate::progress::Stage> {
        crate::progress::read(&self.directory.join("progress.bin"))
    }
    pub fn stop(&self) -> Result<(), String> {
        std::fs::write(
            self.directory.join("STOP"),
            b"User requested documented cleanup\n",
        )
        .map_err(|e| e.to_string())
    }
    /// Reap independently of preview parsing errors. Never release ownership early.
    pub fn completion(&mut self) -> Result<Option<bool>, String> {
        if self.child.try_wait().map_err(|e| e.to_string())?.is_some() {
            self.finished = true;
            Ok(Some(self.directory.join("capture/CLEANUP_OK").is_file()))
        } else {
            Ok(None)
        }
    }
}
// Open once: header and body refer to the same published file even if replaced.
// Unchanged frames cost only a 16-byte read; malformed files cannot grow allocation.
type Record = (u64, u64, Vec<u8>);
fn read_record(
    path: &std::path::Path,
    size: usize,
    previous: u64,
) -> Result<Option<Record>, String> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if file.metadata().map_err(|e| e.to_string())?.len() != 16 + size as u64 {
        return Err("Invalid physical preview record length".into());
    }
    let mut header = [0; 16];
    file.read_exact(&mut header).map_err(|e| e.to_string())?;
    let sequence = u64::from_le_bytes(header[..8].try_into().map_err(|_| "Sequence header")?);
    if sequence <= previous {
        return Ok(None);
    }
    let timestamp = u64::from_le_bytes(header[8..].try_into().map_err(|_| "Time header")?);
    let mut raw = vec![0; size];
    file.read_exact(&mut raw).map_err(|e| e.to_string())?;
    Ok(Some((sequence, timestamp, raw)))
}

impl Drop for PhysicalPreview {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_record_and_sequence() {
        let path = std::env::temp_dir().join(format!("camera-record-{}", std::process::id()));
        assert!(read_record(&path, 4, 0).unwrap().is_none());
        let mut bytes = Vec::from(3u64.to_le_bytes());
        bytes.extend_from_slice(&7u64.to_le_bytes());
        bytes.extend_from_slice(&[1, 2, 3, 4]);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            read_record(&path, 4, 0).unwrap(),
            Some((3, 7, vec![1, 2, 3, 4]))
        );
        assert!(read_record(&path, 4, 3).unwrap().is_none());
        assert!(read_record(&path, 3, 0).is_err());
        std::fs::write(&path, &bytes[..17]).unwrap();
        assert!(read_record(&path, 4, 0).is_err());
        std::fs::remove_file(path).unwrap();
    }
}

/// Cancellation is accepted only with the requested stop and both terminal markers.
pub fn accepted_exit(code: Option<i32>, success: bool, directory: &std::path::Path) -> bool {
    success
        || (code == Some(2)
            && ["STOP", "capture/CANCELLED", "capture/CLEANUP_OK"]
                .iter()
                .all(|p| directory.join(p).is_file()))
}

pub fn helper_error_exit(error: &str) -> i32 {
    if error == crate::physical::CANCELLED {
        2
    } else {
        1
    }
}
#[cfg(test)]
impl PhysicalPreview {
    pub(crate) fn test_child(child: Child, directory: PathBuf) -> Self {
        Self {
            child,
            directory,
            width: 2,
            height: 2,
            sequence: 0,
            source: "Non-hardware protocol fixture".into(),
            finished: false,
        }
    }
}
#[cfg(test)]
mod protocol_tests {
    #[test]
    fn child() {
        if let Some(root) = std::env::var_os("CAMERA_LAB_PROTOCOL_TEST") {
            let root = std::path::PathBuf::from(root);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while !root.join("STOP").is_file() && std::time::Instant::now() < deadline {
                std::thread::yield_now();
            }
            std::fs::create_dir(root.join("capture")).unwrap();
            if std::env::var_os("CAMERA_LAB_NO_CLEANUP").is_none() {
                std::fs::write(root.join("capture/CLEANUP_OK"), b"mock cleanup").unwrap();
            }
            std::fs::write(root.join("capture/CANCELLED"), b"mock cancelled").unwrap();
            std::process::exit(super::helper_error_exit(crate::physical::CANCELLED));
        }
    }
}
