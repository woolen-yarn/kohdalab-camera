#![forbid(unsafe_code)]

mod windows_resources {
    include!("../assets/windows_resources.rs");
}

fn main() {
    let helper_root = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must set CARGO_MANIFEST_DIR"),
    );
    let workspace_root = helper_root
        .parent()
        .expect("helper-entry must remain directly under the workspace root");
    windows_resources::compile(
        workspace_root,
        windows_resources::Metadata {
            binary: "continuous-helper",
            file_description: "KohdaLab Camera capture helper",
            internal_name: "continuous-helper",
            original_filename: "continuous-helper.exe",
            include_gui_resources: false,
        },
    );
}
