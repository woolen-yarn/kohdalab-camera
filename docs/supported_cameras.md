# Supported cameras and evidence

| Backend | Camera / connection | Current status |
| --- | --- | --- |
| simulated | Synthetic RGB | Automated tests; no hardware |
| opencv | OS video device, AVFoundation/MSMF/DirectShow | Implemented generic adapter; models require independent checks |
| legacy-tca | SHODENSHA GR300 / TCA3.1MP, USB 0547:4D33, MT9T001 1621 | Exact identity checked; real-device capture, controls and export verified |

The GR300 path uses direct USB rather than inventing a vendor SDK ABI. Windows requires its existing WinUSB assignment or the bundled device-specific setup helper; Mac uses libusb. Device matching is constrained by USB identity, descriptors and sensor chip ID.

Measured Windows live timing: 1024×768, 2× binning, vertical 3 rows, horizontal 21 clocks, 64 queued 8KiB RAW_IO reads. Full-resolution timing: 2048×1536, vertical 3 / horizontal 1024, approximately 9 fps. Both preserve exact image lengths and reject incomplete data. The full-resolution timing was selected after test-pattern checks revealed corrupted data at the prior setting.

Mac retains 32 queued 512KiB reads and horizontal 336 clocks; its new live vertical gap is 3 rows. Mac full-sensor timing remains vertical 1023 / horizontal 336. The new Mac timing is implemented but awaits real-camera FPS validation. Historical Mac capture/control/export checks and current portable simulated checks are recorded separately.

Exposure uses sensor rows and gain uses the documented register code. Horizontal timing affects the duration of a row; calibrated milliseconds or physical gain accuracy are not promised. See validation.md for exact measurements, limitations and historical failures.

Future targets include additional OS cameras, industrial vendor SDK cameras and direct USB devices. Each model must obtain its own transport tests and real-device evidence before being labeled verified.
