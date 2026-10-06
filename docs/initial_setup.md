# Initial setup

For a portable installation, download the platform ZIP from the release linked in README, extract everything, and open KohdaLab Camera.exe or KohdaLab Camera.app. Test with `simulated` first. Select `legacy-tca` for the verified GR300/TCA3.1MP camera; choose `opencv` for an OS video camera and its camera index/API.

For development, install uv and clone https://github.com/woolen-yarn/kohdalab-camera, then run:

```sh
uv sync --frozen --extra gui --extra usb --extra dev
uv run kohdalab-camera backends
uv run kohdalab-camera-gui
```

`Capture` contains connection and image capture, `Image` contains display correction, `Export` contains destination/format and full-sensor saving, and `Advanced` contains camera-specific hardware controls. Saved RGB is taken before preview scaling. Original sensor TIFF remains unchanged by display correction.

Diagnostics: `kohdalab-camera doctor` inventories devices. `--usb-descriptors` reads USB descriptors; `--probe-video` actively opens OS cameras. Empty OS video enumeration does not prove a USB camera is absent or compatible with UVC.

Run `uv run pytest -q` for hardware-free checks. Real camera tests require explicit `--execute` or `--validate-hardware` flags. Windows setup and manufacturer-driver restore are described in windows-portable.md.
