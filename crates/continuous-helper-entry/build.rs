#![forbid(unsafe_code)]

mod windows_resources {
    include!("../camera-lab/assets/windows_resources.rs");
}

fn main() {
    let helper_root = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must set CARGO_MANIFEST_DIR"),
    );
    let workspace_root = helper_root
        .parent()
        .and_then(std::path::Path::parent)
        .expect("helper-entry must remain under crates/ in the workspace root");
    let application_root = workspace_root.join("crates/camera-lab");
    windows_resources::compile(
        &application_root,
        windows_resources::Metadata {
            binary: "continuous-helper",
            file_description: "KohdaLab Camera capture helper",
            internal_name: "continuous-helper",
            original_filename: "continuous-helper.exe",
            include_gui_resources: false,
        },
    );
}
