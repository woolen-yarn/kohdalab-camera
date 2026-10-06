# KohdaLab Camera

[![Test](https://github.com/woolen-yarn/kohdalab-camera/actions/workflows/test.yml/badge.svg)](https://github.com/woolen-yarn/kohdalab-camera/actions/workflows/test.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A laboratory camera toolkit for live viewing, camera controls, diagnostics and reproducible image export through GUI, CLI and Python API workflows.

カメラ全般を対象とする撮影ソフトです。接続方式ごとのアダプターを追加して拡張できます。最初の実機対応はSHODENSHA GR300 / TCA3.1MPです。

## Portable desktop downloads

No Python installation is needed. Extract the entire ZIP and open the application.

| Platform | Version 0.1.0 |
| --- | --- |
| Windows x64 | [Download portable](https://github.com/woolen-yarn/kohdalab-camera/releases/download/portable-v0.1.0/kohdalab-camera-0.1.0-windows-x64-portable.zip) |
| Mac Apple Silicon | [Download portable](https://github.com/woolen-yarn/kohdalab-camera/releases/download/portable-v0.1.0/kohdalab-camera-0.1.0-macos-arm64-portable.zip) |

[Release assets and checksums](https://github.com/woolen-yarn/kohdalab-camera/releases/tag/portable-v0.1.0) · [Portable guide](docs/portable_release.md)

Keep the Windows `_internal` and `support` folders beside the EXE. The Mac app includes its worker and support files inside the bundle. Select `simulated` for a hardware-free test, `legacy-tca` for the verified legacy camera, or `opencv` for an OS video camera.

## Supported connections

| Adapter | Scope | Verification |
| --- | --- | --- |
| `simulated` | Generated RGB images and simulated controls | Automated acquisition, export and shutdown tests |
| `opencv` | OS video devices through AVFoundation, MSMF or DirectShow | Generic integration available; individual models are not certified |
| `legacy-tca` | SHODENSHA GR300 / TCA3.1MP, USB 0547:4D33, MT9T001 chip 1621 | Real acquisition, controls and lossless export on Mac and Windows |

The verified legacy device is not UVC. Its Windows portable uses WinUSB with a setup helper restricted to that USB identity. Generic camera support does not imply that every USB camera is compatible. See [the support matrix](docs/supported_cameras.md).

## What it does

- Live viewing and controls through a shared camera interface.
- Camera-independent RGB PNG/TIFF export with capture-time metadata; original sensor TIFF where available.
- OS/USB discovery and diagnostics, separate from opening hardware.
- A backend registry shared by the API, CLI and GUI, allowing additional OS, vendor SDK and direct-USB adapters.
- An independent USB worker and a pixel-equivalent native renderer for the verified legacy camera.

Windows legacy-camera validation reached approximately **35.4 fps for both acquisition and application preview updates at 1024×768**, and approximately **8.8–9.0 fps at 2048×1536**. Incomplete frames remain rejected. The Mac app includes the common rendering/IPC improvements; its new real-camera timing has not yet been measured. These figures do not measure RustDesk or monitor delivery. [Validation evidence](docs/validation.md).

## Development quick start

```sh
uv sync --frozen --extra gui --extra usb --extra dev
uv run kohdalab-camera backends
uv run kohdalab-camera doctor
uv run kohdalab-camera capture --backend simulated --output captures
uv run kohdalab-camera-gui
uv run pytest -q
```

The base package provides the API, CLI and simulated adapter. Install `gui`, `uvc`, `usb` or `mac` extras for the corresponding optional connections. Hardware inspection opens no camera unless an explicit probe or capture is requested.

```python
from kohdalab_camera.api import make_camera, save_frame

camera = make_camera("simulated")
try:
    camera.open()
    frame = camera.read()
    path = save_frame(frame, "captures", "tiff")
finally:
    camera.close()
```

## Project organization

```text
src/kohdalab_camera/   Public API, adapters, acquisition, GUI, diagnostics and export
native/               Exact image renderer, Windows setup helper and vendor source notices
scripts/              Native/portable builds, artifact checks and opt-in hardware validation
tests/                Hardware-free adapter, GUI, protocol, processing and cleanup tests
docs/                 Usage, architecture, supported devices and validation evidence
portable/             Distribution entry points, licenses and platform guides
.github/              Cross-platform tests and issue templates
```

This organization follows KohdaLab IV. Portable binaries are GitHub Release assets rather than large files in Git history; corresponding application/native source accompanies each portable in `support/SOURCE.zip`.

## Documentation and extension

- [Initial setup and usage](docs/initial_setup.md)
- [Public API and adding an adapter](docs/api_usage.md)
- [Architecture](docs/architecture.md)
- [Supported cameras](docs/supported_cameras.md)
- [Portable release and rebuilds](docs/portable_release.md)
- [Windows driver setup and restore](docs/windows-portable.md)
- [Hardware validation record](docs/validation.md)
- [Roadmap](ROADMAP.md) · [Contributing](CONTRIBUTING.md) · [Changelog](CHANGELOG.md)

## License

The application and renderer are MIT licensed. Bundled components retain their own licenses, including LGPL libwdi and the original KohdaLab IV notice. See [LICENSE](LICENSE) and [third-party notices](portable/licenses/THIRD_PARTY.md). Manufacturer software and drivers used during analysis are not redistributed.
