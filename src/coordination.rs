//! Persistent create-new recovery interlock, independent of portable application paths.
use std::{
    io::Write,
    path::{Path, PathBuf},
};
pub fn user_root() -> Result<PathBuf, String> {
    let base = std::env::var_os(if cfg!(windows) {
        "LOCALAPPDATA"
    } else {
        "HOME"
    })
    .ok_or("Missing stable per-user data directory; refusing device open")?;
    let base = PathBuf::from(base);
    if !base.is_absolute() {
        return Err("Per-user data directory must be absolute".into());
    }
    Ok(base.join("CameraLab").join("device-interlocks-v1"))
}
pub fn acquire(root: &Path, identity: &str) -> Result<PathBuf, String> {
    // Hex encoding is injective and excludes separators; topology is the supported identity.
    let key: String = identity
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if key.is_empty() || key.len() > 220 {
        return Err("Invalid device identity".into());
    }
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let path = root.join(format!("{key}.lock"));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| {
            format!(
                "Device interlock retained: {e}; inspect evidence, no automatic stale-lock clearing"
            )
        })?;
    writeln!(file, "pid={} identity={identity}; remove only after proved cleanup or separately authorized recovery", std::process::id()).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child() {
        if let Some(root) = std::env::var_os("CAMERA_LAB_LOCK_TEST") {
            let path = acquire(
                Path::new(&root),
                &std::env::var("CAMERA_LAB_LOCK_ID").unwrap(),
            );
            if path.is_ok() && std::env::var_os("CAMERA_LAB_LOCK_HOLD").is_some() {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while !Path::new(&root).join("release").exists()
                    && std::time::Instant::now() < deadline
                {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
            std::process::exit(if path.is_ok() { 0 } else { 3 });
        }
    }
    #[test]
    fn two_process_copies_same_device_and_stale_error_preservation() {
        let root = std::env::temp_dir().join(format!("lock-test-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let exe = std::env::current_exe().unwrap();
        let shared = root.join("shared");
        for name in ["copy-a", "copy-b"] {
            std::fs::create_dir(root.join(name)).unwrap();
            std::fs::copy(&exe, root.join(name).join(exe.file_name().unwrap())).unwrap();
        }
        let command = |copy: &str, id: &str| {
            let mut c = std::process::Command::new(root.join(copy).join(exe.file_name().unwrap()));
            c.args(["--exact", "coordination::tests::child"])
                .current_dir(root.join(copy))
                .env("CAMERA_LAB_LOCK_TEST", &shared)
                .env("CAMERA_LAB_LOCK_ID", id);
            c
        };
        let launch = |copy: &str, id: &str| command(copy, id).status().unwrap().code().unwrap();
        let mut owner = command("copy-a", "device-one")
            .env("CAMERA_LAB_LOCK_HOLD", "1")
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !shared.exists() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        while std::fs::read_dir(&shared).unwrap().next().is_none()
            && std::time::Instant::now() < deadline
        {
            std::thread::yield_now();
        }
        assert_eq!(
            launch("copy-b", "device-one"),
            3,
            "live other executable copy must exclude"
        );
        std::fs::write(shared.join("release"), b"release fixture").unwrap();
        assert!(owner.wait().unwrap().success());
        std::fs::remove_file(shared.join("release")).unwrap();
        let lock = std::fs::read_dir(&shared)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let bytes = std::fs::read(&lock).unwrap();
        assert_eq!(launch("copy-b", "device-one"), 3);
        assert_eq!(std::fs::read(&lock).unwrap(), bytes); // exited owner is still interlocked
        assert_eq!(launch("copy-b", "device-two"), 0);
        let invalid = root.join("not-directory");
        std::fs::write(&invalid, b"retained error").unwrap();
        assert!(acquire(&invalid, "device-three").is_err());
        assert_eq!(std::fs::read(invalid).unwrap(), b"retained error");
        std::fs::remove_dir_all(root).unwrap();
    }
}
