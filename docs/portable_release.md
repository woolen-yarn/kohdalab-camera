# Portable releases

Download Windows x64 or macOS Apple Silicon ZIPs from the [GitHub Releases](https://github.com/woolen-yarn/kohdalab-camera/releases). Extract the entire archive before opening KohdaLab Camera. Python installation is unnecessary. Keep the worker, CLI, shared runtime and support files together.

Windows: open `KohdaLab Camera.exe`. Select `legacy-tca` for the verified GR300 camera. Initial WinUSB installation may require administrator approval; read [Windows setup](windows-portable.md). macOS: open `KohdaLab Camera.app`; its current signature is ad hoc, rather than an Apple Developer notarized signature. Only open a trusted download after checking its checksum.

Each GitHub release includes a separate `SHA256SUMS.txt` asset; download it alongside the ZIP. Windows can calculate a checksum with `Get-FileHash -Algorithm SHA256`; macOS can use `shasum -a 256`. Compare the entire digest. Source and licensing notices are included under `support` (inside the app's Resources on macOS).

## Rebuilding

Use Python 3.13 with dependencies from `uv.lock`, GUI/USB/dev extras and PyInstaller. Build the native renderer first with `python scripts/build_camera_renderer.py`. Windows also needs the narrowly scoped USB setup helper and libwdi, built according to [driver documentation](windows-portable.md). Run `python scripts/build_windows_portable.py` on Windows x64 or `python scripts/build_macos_portable.py` on Apple Silicon macOS. The scripts compile the shared-runtime GUI, worker and CLI, bundle source/notices, run a simulated capture/save/worker-shutdown smoke test and create ZIPs in `dist`.

Release names include the package version and architecture. Run `python scripts/stage_portable_release.py` after placing both platform ZIPs in `dist` to create the versioned assets and combined checksums in `releases/portable-v0.1.0`. Binaries live in Releases, rather than Git history; `portable/` contains distribution instructions and licensing notes. A new adapter must be included in a rebuilt package.

## Validation boundaries

The portable smoke test checks native renderer correctness, worker startup, simulated capture, saving and shutdown. Windows `--validate-hardware --validate-seconds 90` additionally measures USB capture and actual GUI preview updates, verifies control readback and lossless saves, and records results in `validation/`. Run only one camera process at a time.

Windows GR300 acquisition has been verified with the real device. Generic OpenCV devices depend on their OS drivers and need per-device testing. The current Mac package can be tested without a camera using simulation; that does not establish real-device FPS or validate the new Mac timing profile. See [supported cameras](supported_cameras.md).
