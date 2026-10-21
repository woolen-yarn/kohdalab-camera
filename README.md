<img src="crates/camera-lab/assets/kohdalab-camera.png" width="64" height="64" align="right" alt="KohdaLab Camera icon">

# KohdaLab Camera

[![CI](https://github.com/woolen-yarn/kohdalab-camera/actions/workflows/windows-unsigned.yml/badge.svg)](https://github.com/woolen-yarn/kohdalab-camera/actions/workflows/windows-unsigned.yml)
[![Release](https://img.shields.io/github/v/release/woolen-yarn/kohdalab-camera)](https://github.com/woolen-yarn/kohdalab-camera/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Rust desktop application for laboratory camera acquisition, live imaging, and reproducible image export.

[Download](https://github.com/woolen-yarn/kohdalab-camera/releases) · [Documentation](docs/README.md) · [Report an issue](https://github.com/woolen-yarn/kohdalab-camera/issues)

- Live preview with exposure/gain controls and image adjustments.
- Processed PNG/TIFF, original Bayer TIFF/RAW, and metadata exports.
- An isolated acquisition helper owns the physical session; simulation is available without hardware.

## Interface

![KohdaLab Camera interface](docs/images/image-controls.png)

Native interface in Simulation on macOS. Published executables currently target Windows x64; window decorations differ.

## Downloads

| Channel | Version | Platforms |
| --- | --- | --- |
| Stable | [0.1.11](https://github.com/woolen-yarn/kohdalab-camera/releases/tag/v0.1.11) | Windows x64 Setup / portable ZIP |

Rust and Python are not required to run the packaged application. Current Windows
artifacts are unsigned; download packages and checksums together from Releases.

## Supported hardware

| Source | Support |
| --- | --- |
| SHODENSHA GR300 | Tested laboratory unit: Micron MT9T001, USB `0547:4d33`, existing WinUSB binding |
| Simulation / demo | Synthetic source; no camera required |
| Other camera models | Not supported yet |

See [camera support and qualification](docs/reference/hardware.md). A matching model name or USB ID alone does not establish compatibility. Multi-camera/vendor support is a roadmap, not a current capability.

## Quick start

1. Run Windows Setup or extract the complete portable ZIP. Keep `continuous-helper.exe` and license files alongside the GUI.
2. Select **Simulation / demo**, then **Load camera**, to explore without hardware.
3. For the supported physical configuration, keep the existing WinUSB setup and select **Load camera** deliberately. The application does not replace drivers.
4. Adjust acquisition/image settings and export using the capture actions. Existing output names are not overwritten.

Builds on another operating system do not establish physical-backend support. See the [Windows installation guide](docs/guides/windows-installation.md) and [qualification evidence](docs/validation/).

## Development

Run commands from the repository root. The root `Cargo.toml` and `Cargo.lock`
are the workspace entry point; product packages live under `crates/`.

```sh
cargo test --locked --lib --bin camera-lab
cargo build --locked --release --package camera-lab --bin camera-lab
cargo build --locked --release --package continuous-helper-entry --bin continuous-helper
```

```text
crates/       Rust desktop, acquisition helper, and capability platform
packaging/    Windows installer assets
scripts/      Build, package, and validation entry points
third-party-licenses/  Redistributed dependency license texts
docs/         Guides, development notes, references, validation, images
```

See [repository layout](docs/development/repository-layout.md) and
[CONTRIBUTING.md](CONTRIBUTING.md) before changing device or build boundaries.
Tests must not connect hardware, move stages, enable outputs, or alter drivers.

## Documentation

- [Documentation index](docs/README.md): the entry point for all guides and references.
- `docs/guides/`: installation and everyday operation.
- `docs/development/`: architecture, extension points, and build/release maintenance.
- `docs/reference/`: hardware, protocols, formats, and compatibility contracts.
- `docs/validation/`: versioned evidence and measured limitations.
- `docs/images/`: repository screenshots and figures.

## KohdaLab applications

[IV](https://github.com/woolen-yarn/kohdalab-iv) · [TRKR](https://github.com/woolen-yarn/kohdalab-trkr) · [Camera](https://github.com/woolen-yarn/kohdalab-camera)

The applications share a root Cargo workspace, `crates/` source packages, focused
build scripts, and the same documentation categories. Hardware backends and
qualification remain product-specific.

## License

Project code is [MIT licensed](LICENSE). Native helpers and redistributed
dependencies retain their own licenses; keep the accompanying notices and source.
See [third-party notices](THIRD_PARTY_NOTICES.md) and [provenance](docs/reference/provenance.md).
