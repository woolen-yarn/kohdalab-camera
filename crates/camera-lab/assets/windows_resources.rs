use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub struct Metadata<'a> {
    pub binary: &'a str,
    pub file_description: &'a str,
    pub internal_name: &'a str,
    pub original_filename: &'a str,
    pub include_gui_resources: bool,
}

/// Compile the common Windows resource template for a GNU Windows target.
///
/// The application package version is deliberately used for both executables. The
/// helper is released only alongside the GUI, so a separate helper-entry
/// package version must not leak into its PE metadata.
pub fn compile(application_root: &Path, metadata: Metadata<'_>) {
    let target = env::var("TARGET").expect("Cargo must set TARGET for build scripts");
    if !target.ends_with("-pc-windows-gnu") {
        return;
    }

    let resource_template = application_root.join("assets/windows.rc");
    let manifest = application_root.join("assets/windows.manifest");
    let icon = application_root.join("assets/camera.ico");
    let application_manifest = application_root.join("Cargo.toml");
    for path in [&resource_template, &manifest, &icon, &application_manifest] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    for name in windres_environment_names(&target) {
        println!("cargo:rerun-if-env-changed={name}");
    }

    let version = application_package_version(&application_manifest);
    let version_commas = pe_version_components(&version);
    let template = fs::read_to_string(&resource_template)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", resource_template.display()));
    let rendered = template
        .replace("@VERSION_COMMAS@", &version_commas)
        .replace("@VERSION_STRING@", &rc_string(&version))
        .replace("@FILE_DESCRIPTION@", &rc_string(metadata.file_description))
        .replace("@INTERNAL_NAME@", &rc_string(metadata.internal_name))
        .replace("@ORIGINAL_FILENAME@", &rc_string(metadata.original_filename));
    if rendered.contains('@') {
        panic!("unresolved Windows resource template placeholder");
    }

    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"));
    let rendered_resource = output_dir.join(format!("{}-resource.rc", metadata.binary));
    fs::write(&rendered_resource, rendered).unwrap_or_else(|error| {
        panic!(
            "could not write generated resource {}: {error}",
            rendered_resource.display()
        )
    });
    let output = output_dir.join(format!("{}-resource.o", metadata.binary));
    let windres = find_windres(&target).unwrap_or_else(|| {
        panic!(
            "no GNU Windows resource compiler was found for {target}; set KOHDA_CAMERA_WINDRES, {} or WINDRES",
            target_specific_windres_environment_name(&target)
        )
    });

    let mut command = Command::new(&windres);
    command
        .current_dir(application_root)
        .arg("-i")
        .arg(&rendered_resource)
        .arg("-o")
        .arg(&output);
    if metadata.include_gui_resources {
        command.arg("-DINCLUDE_GUI_RESOURCES");
    }
    let status = command.status().unwrap_or_else(|error| {
        panic!(
            "could not run GNU Windows resource compiler {}: {error}",
            windres.to_string_lossy()
        )
    });
    assert!(
        status.success(),
        "GNU Windows resource compiler {} failed for {}",
        windres.to_string_lossy(),
        metadata.binary
    );
    println!(
        "cargo:rustc-link-arg-bin={}={}",
        metadata.binary,
        output.display()
    );
}

fn application_package_version(application_manifest: &Path) -> String {
    let contents = fs::read_to_string(application_manifest).unwrap_or_else(|error| {
        panic!("could not read {}: {error}", application_manifest.display())
    });
    let mut in_package = false;
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            if let Some(value) = line.strip_prefix("version") {
                let value = value.trim_start();
                if let Some(value) = value.strip_prefix('=') {
                    let value = value.trim();
                    if let Some(version) = value
                        .strip_prefix('"')
                        .and_then(|value| value.strip_suffix('"'))
                    {
                        return version.to_owned();
                    }
                }
            }
        }
    }
    panic!(
        "could not find a quoted package version in {}",
        application_manifest.display()
    );
}

fn pe_version_components(version: &str) -> String {
    let numeric = version
        .split(['-', '+'])
        .next()
        .expect("split always yields at least one component");
    let mut values = numeric
        .split('.')
        .map(|part| {
            part.parse::<u16>().unwrap_or_else(|_| {
                panic!("package version {version:?} cannot be represented in a PE VERSIONINFO")
            })
        })
        .collect::<Vec<_>>();
    if values.is_empty() || values.len() > 4 {
        panic!("package version {version:?} must have between one and four numeric components");
    }
    values.resize(4, 0);
    values
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn rc_string(value: &str) -> String {
    if value.contains(['"', '\\', '\r', '\n']) {
        panic!("Windows resource string contains an unsupported character");
    }
    value.to_owned()
}

fn windres_environment_names(target: &str) -> [String; 3] {
    [
        "KOHDA_CAMERA_WINDRES".to_owned(),
        target_specific_windres_environment_name(target),
        "WINDRES".to_owned(),
    ]
}

fn target_specific_windres_environment_name(target: &str) -> String {
    format!(
        "CARGO_TARGET_{}_WINDRES",
        target.replace('-', "_").to_ascii_uppercase()
    )
}

fn find_windres(target: &str) -> Option<OsString> {
    for name in windres_environment_names(target) {
        if let Some(value) = env::var_os(&name).filter(|value| !value.is_empty()) {
            return Some(value);
        }
    }

    let mut candidates = Vec::new();
    if target.starts_with("x86_64-") {
        candidates.push(OsString::from("x86_64-w64-mingw32-windres"));
    } else if target.starts_with("i686-") {
        candidates.push(OsString::from("i686-w64-mingw32-windres"));
    }
    // MSYS2's MinGW package on a Windows GitHub runner commonly exposes the
    // unprefixed command, while Linux cross toolchains conventionally use the
    // target-prefixed command above.
    candidates.push(OsString::from("windres"));
    candidates
        .into_iter()
        .find(|candidate| windres_is_available(candidate))
}

fn windres_is_available(candidate: &OsString) -> bool {
    Command::new(candidate)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}
