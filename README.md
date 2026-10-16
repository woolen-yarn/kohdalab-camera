# KohdaLab Camera

A Rust desktop platform for laboratory camera acquisition, live imaging, image
adjustment, and reproducible exports.

The long-term goal is a common application for cameras from multiple vendors,
with device-specific backends and a consistent capture workflow. **The current
release supports one physical camera configuration only**; broader camera
support is a roadmap, not an existing capability.

## Current camera support

| Manufacturer / camera model | Sensor / USB identity | Current support |
| --- | --- | --- |
| **SHODENSHA GR300** | Micron MT9T001; USB `0547:4d33`; ScopeTek-family backend. | Tested on one laboratory unit. Windows x64 with an existing WinUSB binding; 1024 × 768 Live and 2048 × 1536 full-sensor capture. |
| Simulation / demo | Synthetic source; no camera sensor | Available without hardware for exploring the interface. |
| Other camera models | UVC webcams, other USB cameras, and vendor SDK devices | Not supported yet; multi-camera support is planned. |

The tested camera is identified by its owner as **SHODENSHA GR300**. Its USB
product string is `3.1M USB2.0 Camera`; the sensor and USB ID above describe this
tested unit. Other GR300 revisions and similarly named models have not been
validated.

A matching USB ID alone does not establish compatibility with another camera,
firmware revision, or sensor. See [camera support and validation](docs/camera-support.md).

## Downloads and quick start

The current published baseline is **0.1.8**, for **Windows x64**.

- [Latest release and portable downloads](https://github.com/woolen-yarn/kohdalab-camera/releases/latest)
- [Windows portable ZIP](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.8/kohdalab-camera-0.1.8-windows-x64.zip)
- [Source ZIP](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.8/kohdalab-camera-0.1.8-source.zip)
- [SHA-256 checksums](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.8/SHA256SUMS.txt)

Extract the entire Windows ZIP and launch `KohdaLab Camera.exe`. Keep
`continuous-helper.exe` and the license files alongside it. Rust and Python are
not required to run the portable application. The physical backend requires an
existing WinUSB setup; the application does not install or change drivers.
No macOS or Linux executable is distributed in this release.

1. Select the supported USB camera, then choose **Load camera**.
   Choose **Simulation / demo** explicitly to explore without hardware.
2. Use the **Camera / Acquisition / Image / Export / Advanced** tabs.
   **F1–F5** also select the tabs.
3. Adjust exposure and gain, then apply acquisition settings. Image adjustments
   affect the processed image while the original Bayer data remains available.
4. Use **Capture image…** to export the displayed image, or
   **Capture full sensor image…** to acquire a new full-resolution frame and
   return to the previously applied Live settings.
5. Use **Disconnect** or close the window to end the session normally.

## Imaging and data

See [interface design and references](docs/design.md) for the UI principles,
design influences and interaction decisions.

- Per-pixel GRBG demosaicing, linear display sampling, white balance, gamma,
  rotation, flips, and monochrome display.
- Brightness is a positive multiplier with a default of **1.20×**.
- Processed PNG/TIFF exports, original Bayer TIFF, RAW data, and metadata.
  Existing output names are not overwritten.
- Capture FPS in the status bar, with preview timing available on hover.
- Timestamps represent **host UTC time when a complete frame was accepted**,
  rather than sensor exposure time.
- A separate acquisition helper owns the physical session. When cleanup cannot
  be established, the application retains its protective interlock rather than
  automatically clearing locks or resetting USB.

When one supported camera is found, it is selected automatically. Press **Load
camera** to open Live. With multiple cameras, choose one first.

Image navigation: wheel to zoom, drag to pan, double-click or **Image → Fit**
to fit the window, and **Image → 100%** for one source pixel per UI point.
Zooming out stops at the initial **Fit** size. Click the selected tab again to
close the inspector. **100%** is available when it falls between Fit and the
maximum zoom. Preview remains bounded to 1024 pixels; exports retain full source dimensions.

Camera/image settings, window size, save folder and capture numbering are
remembered. **Advanced → Reset all settings** restores defaults; press Apply
to update active camera acquisition. Startup still requires **Load camera**.
Save dialogs suggest numbered names such as `capture_0001.png`; existing files
and sidecars are never overwritten.

Default settings: exposure **120 sensor rows**, brightness **1.20×**, gamma
**2.2** and analog gain **1×**. Version 0.1.5 changes only the exposure default;
the physical run below used version 0.1.4.

## Validation and known limitations

Version 0.1.4 completed 336.2 seconds of Live acquisition on SHODENSHA GR300,
with 13,215 accepted frames. The camera was selected automatically, and **Load
camera** opened Live. The saved PNG matched an independent RAW reconstruction;
the original Bayer TIFF matched RAW. GUI and helper exited normally, with no
remaining process or protective lock. See the
[scoped validation record](docs/validation-0.1.4.json).

The owner confirmed normal manual mouse operation, arbitrary window resizing and
retention of the changed size. Automated click/drag delivery through Windows App
has tooling limitations. Picker round-trip size retention was not repeated in
this run. The owner subsequently reported that color and field of view looked
acceptable. This is a visual assessment; no optical or color calibration was performed.

The software renderer is pixel-equivalent to the previous version and passed
independent pixel tests. See [performance verification](docs/rendering-performance.md).

Observed acquisition/preview rates around 39.5 frames/s are specific to the test
conditions and are not a guaranteed performance specification or an optical
frame-rate calibration.

## Platform roadmap

1. Complete interaction, image-quality, and device qualification for the current backend.
2. Define a shared capability model for discovery, connection, acquisition,
   controls, frame formats, timestamps, and safe shutdown.
3. Add individually validated camera backends using documented, redistributable
   interfaces; keep vendor-specific behavior behind those backends.
4. Extend the common GUI and export workflow across supported cameras, publishing
   a compatibility matrix and limitations for every backend and operating system.

See [CONTRIBUTING.md](CONTRIBUTING.md) before proposing a camera backend.

## Build and validation

Use Rust stable with edition 2024 support. Dependency versions are pinned in
`Cargo.lock`.

```sh
cargo build --locked --release --package camera-lab --bin camera-lab
cargo build --locked --release --package continuous-helper-entry --bin continuous-helper
cargo test --locked --lib --bin camera-lab
```

Cross-compiling the Windows GNU target additionally requires the
`x86_64-pc-windows-gnu` Rust standard library and MinGW-w64 gcc/windres:

```sh
cargo build --locked --release --target x86_64-pc-windows-gnu --package camera-lab --bin camera-lab
cargo build --locked --release --target x86_64-pc-windows-gnu --package continuous-helper-entry --bin continuous-helper
```

Unit tests do not operate a physical camera. Building for another operating
system does not establish physical-backend support or distribution readiness.

## License and provenance

The application icon is project artwork generated for KohdaLab Camera; the PNG
source and Windows ICO are included under the project license.

Project code is MIT licensed. Retain [LICENSE](LICENSE),
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), and the texts under
`third-party-licenses/` when redistributing. The bundled HTML is a Rust runtime
copyright notice, not an HTML implementation of the application.

See [provenance and redistribution scope](docs/provenance.md) for the current
backend's research inputs and limitations. Manufacturer software, legacy
application source/binaries, and manufacturer artwork are not bundled.
