# KohdaLab Camera

A Rust desktop platform for laboratory camera acquisition, live imaging, image
adjustment, and reproducible exports.

The long-term goal is a common application for cameras from multiple vendors,
with device-specific backends and a consistent capture workflow. **The current
release supports one physical camera configuration only**; broader camera
support is a roadmap, not an existing capability.

## Current camera support

| Camera / backend | Current status | Requirements and modes |
| --- | --- | --- |
| ScopeTek-family USB camera, USB ID `0547:4d33`, Micron MT9T001 sensor | Initial physical backend; tested on one laboratory unit. Commercial model name is unconfirmed. | Windows x64 with an existing WinUSB binding; 1024 × 768 Live and 2048 × 1536 full-sensor capture. |
| Simulation / demo | Available without a physical camera | Synthetic frames for exploring the interface; does not validate hardware compatibility. |
| Other USB cameras, UVC webcams, and vendor SDK cameras | Not supported yet | Future backends require documented interfaces, redistribution-compatible dependencies, and separate hardware validation. |

A matching USB ID alone does not establish compatibility with another camera,
firmware revision, or sensor. See [camera support and validation](docs/camera-support.md).

## Downloads and quick start

The current published baseline is **0.1.1**, for **Windows x64**.

- [Latest release and portable downloads](https://github.com/woolen-yarn/kohdalab-camera/releases/latest)
- [Windows portable ZIP](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.1/kohdalab-camera-0.1.1-windows-x64.zip)
- [Source ZIP](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.1/kohdalab-camera-0.1.1-source.zip)
- [SHA-256 checksums](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.1/SHA256SUMS.txt)

Extract the entire Windows ZIP and launch `KohdaLab Camera.exe`. Keep
`continuous-helper.exe` and the license files alongside it. Rust and Python are
not required to run the portable application. The physical backend requires an
existing WinUSB setup; the application does not install or change drivers.
No macOS or Linux executable is distributed in this release.

1. Select the supported USB camera, then choose **Connect and open capture window**.
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

- Per-pixel GRBG demosaicing, linear display sampling, white balance, gamma,
  rotation, flips, and monochrome display.
- Brightness is a positive multiplier with a default of **1.20×**.
- Processed PNG/TIFF exports, original Bayer TIFF, RAW data, and metadata.
  Existing output names are not overwritten.
- Separate acquisition and preview FPS readouts.
- Timestamps represent **host UTC time when a complete frame was accepted**,
  rather than sensor exposure time.
- A separate acquisition helper owns the physical session. When cleanup cannot
  be established, the application retains its protective interlock rather than
  automatically clearing locks or resetting USB.

## Validation and known limitations

A pre-publication candidate with the same capture and imaging feature code was
checked on the laboratory unit for Live acquisition, PNG export, full-sensor
capture, restoration of Live settings, and normal process shutdown. Saved PNG
pixels matched an independent reconstruction from RAW; original Bayer TIFF
pixels matched RAW exactly. The publication build has different binary hashes
after naming and diagnostic-path changes and has **not been physically retested**.

Ordinary mouse operation through Windows App remains under investigation;
keyboard operation with F1–F5, Tab, and Enter was observed. Color/CFA and optical
field-of-view calibration, arbitrary window-size persistence, and long-duration
qualification of the current release remain incomplete. Long-duration testing
is deferred. This is an initial development baseline, not a fully qualified
multi-camera release.

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
