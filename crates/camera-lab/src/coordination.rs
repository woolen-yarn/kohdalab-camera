//! Persistent create-new recovery interlock, independent of portable application paths.
use std::{
    io::Write,
    path::{Path, PathBuf},
};
/// Safe recovery whenever reconnection is disabled. Reconnection can be
/// disabled before any interlock was created (for example a missing helper),
/// so this text never asserts that an interlock exists; it only governs one
/// that may remain. A confirmed existing marker is reported separately by
/// [`retained_interlock_message`] with its path.
///
/// It separates three different facts: the normal end state (needs nothing),
/// diagnostic evidence (observations to preserve), and confirmation by the
/// responsible person (the only authority for manual removal). It never
/// authorizes automatic deletion, unproved manual deletion, or treating a
/// camera power cycle alone as proof.
pub const RECOVERY_GUIDANCE: &str = "Safe recovery: reconnection stays disabled for this application session. \
This failure may or may not have left a protective device interlock; this message alone does not show that one exists. \
[Normal end] A Disconnect or Close that reports verified cleanup removes any interlock it created; then no action is needed. Only that report is a normal end. \
[Diagnostic evidence] Keep the application closed. If an interlock file remains, do not delete, rename or edit it. Preserve any remaining interlock file, the Error details text and the failure records unchanged, and note in Task Manager whether any KohdaLab Camera or continuous-helper.exe process remains. \
[Responsible-person confirmation] Give that evidence to the person responsible for the camera and this PC. Only after they have reviewed it and confirmed that no process still uses the camera may they move a remaining interlock file out of the folder and keep it with the evidence. \
Never delete a remaining interlock automatically or without that confirmation. Unplugging or power-cycling the camera, restarting the application or reinstalling it is not proof of cleanup.";

/// Startup refusal for an interlock that already exists. The file is not
/// opened for writing, renamed or deleted; the caller keeps refusing startup.
pub fn retained_interlock_message(path: &Path, cause: &dyn std::fmt::Display) -> String {
    format!(
        "Device interlock retained at {}: {cause}. {RECOVERY_GUIDANCE}",
        path.display()
    )
}

/// Errors from a create-new interlock attempt that mean "a marker is present".
pub fn is_retained_interlock(error: &std::io::Error) -> bool {
    // Windows sharing/lock violation: another live owner holds the marker.
    let held = cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33));
    error.kind() == std::io::ErrorKind::AlreadyExists || held
}

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
            if is_retained_interlock(&e) {
                retained_interlock_message(
                    &path,
                    &format_args!("{e}; inspect evidence, no automatic stale-lock clearing"),
                )
            } else {
                format!(
                    "Device interlock retained: {e}; inspect evidence, no automatic stale-lock clearing"
                )
            }
        })?;
    writeln!(file, "pid={} identity={identity}; remove only after proved cleanup or separately authorized recovery", std::process::id()).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
pub(crate) mod tests {
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
    /// Mandatory guidance content, shared by the GUI and Windows startup tests.
    pub(crate) fn assert_safe_recovery_guidance(text: &str) {
        for required in [
            "reconnection stays disabled",
            "may or may not have left a protective device interlock",
            "does not show that one exists",
            "[Normal end]",
            "verified cleanup",
            "[Diagnostic evidence]",
            "If an interlock file remains, do not delete, rename or edit it",
            "continuous-helper.exe",
            "[Responsible-person confirmation]",
            "Only after they have reviewed it",
            "Never delete a remaining interlock automatically or without that confirmation",
            "power-cycling the camera",
            "is not proof of cleanup",
        ] {
            assert!(text.contains(required), "missing {required:?}");
        }
        // Ordering: evidence precedes confirmation, which precedes any removal.
        let evidence = text.find("[Diagnostic evidence]").unwrap();
        let confirm = text.find("[Responsible-person confirmation]").unwrap();
        let removal = text.find("move a remaining interlock file").unwrap();
        assert!(evidence < confirm && confirm < removal);
        for forbidden in [
            "delete the interlock file",
            "power cycle to clear",
            "remove it to retry",
        ] {
            assert!(!text.contains(forbidden), "unsafe {forbidden:?}");
        }
    }
    #[test]
    fn interlock_recovery_existing_marker_refused_unchanged_with_guidance() {
        let root = std::env::temp_dir().join(format!("interlock-recovery-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        // Abnormal end model: the owner exits without proved cleanup; marker persists.
        let marker = acquire(&root, "device-one").unwrap();
        let before = std::fs::read(&marker).unwrap();
        for _ in 0..2 {
            let error = acquire(&root, "device-one").unwrap_err();
            assert!(error.contains("no automatic stale-lock clearing"));
            assert!(error.contains(&marker.display().to_string()));
            assert_safe_recovery_guidance(&error);
        }
        assert_eq!(std::fs::read(&marker).unwrap(), before);
        assert!(is_retained_interlock(
            &std::io::ErrorKind::AlreadyExists.into()
        ));
        assert!(!is_retained_interlock(&std::io::ErrorKind::NotFound.into()));
        assert_safe_recovery_guidance(&retained_interlock_message(&marker, &"x"));
        // Test-private fixture directory only; never a user interlock root.
        std::fs::remove_dir_all(root).unwrap();
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
