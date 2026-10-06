# Windows x64 portable

Extract the complete ZIP and open KohdaLab Camera.exe. Connect the 0547:4D33 camera and click Connect. The initial setup assigns Microsoft's inbox WinUSB to this camera using a dedicated libwdi helper adapted from KohdaLab IV. Administrator and publisher approval may be requested. The fixed allowlist excludes other instruments and requires only one matching camera during setup.

Before replacing an OEM driver, setup automatically exports it under %LOCALAPPDATA%\KohdaLab Camera\driver-backup. A failed backup prevents replacement. The legacy GR300W driver is replaced for this camera. The manufacturer's Measure/DirectShow/TWAIN tools will require restoring that driver. Windows memory integrity and driver-signature enforcement remain enabled. The application uses the USB protocol implemented and validated on macOS, through libusb/WinUSB on Windows. Exposure, gain, live capture, RGB and sensor TIFF saving, and the separate camera worker use the same code on both platforms.

The portable includes the Qt runtime, Python, libusb, libwdi, and a separate camera worker. Keep _internal and support next to the EXE. Python and uv are not needed to run it. The native helper's logs are in %LOCALAPPDATA%\KohdaLab Camera\usb-setup.log. Setup generates a signed device package and installs its publisher certificate using libwdi.

Before switching an existing device, preserve its INF package with `pnputil /export-driver oem73.inf driver-backup` (replace the INF name with the device's actual installed INF). To restore the manufacturer's driver, use Device Manager → Update driver → Browse → Let me pick, then select the manufacturer driver, or Have Disk and the saved INF. Keep the backup until the manufacturer's application works again. Installing a lower-ranked package with pnputil /add-driver alone does not guarantee that Windows selects it.

## Rebuild

On Windows x64:

```powershell
uv sync --frozen --python 3.13 --extra gui --extra usb --extra dev
uv run --frozen python scripts/build_camera_renderer.py
uv run --frozen --extra gui --extra usb --with pyinstaller python scripts/build_windows_portable.py
```

The C image renderer is compiled with an x64 MinGW compiler on Windows; a Mac with MinGW can cross-compile it using scripts/build_camera_renderer.py --windows. It implements the existing pixel-exact conversion in one pass and releases the Python GIL during computation. Both portable builders require the matching native library and the frozen smoke test checks an image hash generated before optimization.

The setup helper is built separately with scripts/build_usb_setup.sh (MinGW x64 compiler and resource compiler; a host C compiler generates embedded resources). The helper build also stages Camera-USB-Setup.exe and libwdi.dll into support. The portable builder validates both files and runs the helper self-test before packaging. Corresponding libwdi sources and LGPL terms are in native/vendor/libwdi; the setup wrapper preserves the KohdaLab IV MIT notice in native/KOHDALAB-IV-LICENSE. The replaceable LGPL DLL stays alongside the helper. Complete corresponding application and setup source is in support/SOURCE.zip.

## Windows live timing

The 1024×768 live profile uses MT9T001 register 06 = 3 on both platforms. Windows also uses register 05 = 21 and 64 queued 8KiB RAW_IO reads; 20-second test-pattern comparisons confirmed approximately 35.3 complete fps with every pixel correct. Mac keeps register 05 = 336 and 32 queued 512KiB reads pending a hardware comparison. Both platforms use the optimized, pixel-equivalent image renderer and blocking response-reader thread. The private worker sends only owned raw sensor pixels and their capture-time settings; the receiving capture thread renders RGB, reducing image IPC payload to approximately one quarter without changing pixels. Preview refresh uses a 16ms precise Qt timer and the UI distinguishes capture and preview fps.

Resolution, binning, exposure register value, gain, and the exact frame-size check are preserved. Horizontal timing changes the time represented by an exposure row; exposure in milliseconds is not calibrated. Full-sensor timing is vertical 3 / horizontal 1024 on Windows, and vertical 1023 / horizontal 336 on Mac. The Windows setting limits the full-resolution output burst: horizontal 336 produced missing pixels in the sensor test pattern even in some exact-sized images, while horizontal 1024 produced 176 fully correct 2048×1536 images in 20 seconds (8.8 fps). Both blanking registers are checked after initialization. `KOHDA_CAMERA_VBLANK=1023` and `KOHDA_CAMERA_HBLANK=336` restore earlier sensor timing; explicit overrides remain selected across control/profile reinitializations.

Opt-in hardware validation supports `KohdaLab Camera.exe --validate-hardware --validate-seconds 90`. It measures the steady capture interval after exposure/gain changes have returned to 128 rows / 1× gain, checks lossless RGB and sensor saves, and writes validation/validation.json. The steady capture rate counts complete USB frames; the validation poll's observed sequence count is not the GUI display rate. steady_preview_fps counts actual preview updates in the Qt event loop; it does not measure monitor or RustDesk delivery. Remaining incomplete USB frames are still rejected and counted.

Final native-renderer portable validated on tetra (2026-10-06): 3,189 complete 1024×768 images captured and 3,189 preview updates during a 90.016-second steady interval (both 35.427 fps). USB recoveries and superseded preview frames were zero; 20 incomplete USB images were discarded. Exposure/gain readback and PNG/TIFF/sensor-TIFF pixel equality passed. Full-resolution test-pattern trials produced 263 and 270 fully correct 2048×1536 images in separate 30-second runs (approximately 8.8–9.0 fps), and normal full-resolution saving/resumption passed. Both platforms passed all 87 automated tests. Mac arm64 bundled-worker, native renderer image hash, simulated acquisition, saving, shutdown, and signature checks passed; Mac's new real-camera timing has not been measured.
