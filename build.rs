#![forbid(unsafe_code)]
fn main() {
    for path in [
        "assets/windows.rc",
        "assets/windows.manifest",
        "assets/camera.ico",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    if std::env::var("TARGET").unwrap() == "x86_64-pc-windows-gnu" {
        let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
            .join("camera-resource.o");
        let status = std::process::Command::new("x86_64-w64-mingw32-windres")
            .args(["-i", "assets/windows.rc", "-o"])
            .arg(&output)
            .status()
            .expect("MinGW windres is required");
        assert!(status.success(), "Windows resources must compile");
        println!("cargo:rustc-link-arg-bin=camera-lab={}", output.display());
    }
}
