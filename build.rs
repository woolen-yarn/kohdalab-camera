#![forbid(unsafe_code)]

mod windows_resources {
    include!("assets/windows_resources.rs");
}

fn main() {
    let workspace_root = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must set CARGO_MANIFEST_DIR"),
    );
    windows_resources::compile(
        &workspace_root,
        windows_resources::Metadata {
            binary: "camera-lab",
            file_description: "KohdaLab Camera",
            internal_name: "kohdalab-camera",
            original_filename: "KohdaLab Camera.exe",
            include_gui_resources: true,
        },
    );
}
