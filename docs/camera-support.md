# Camera support and validation

## Current physical backend

The initial backend targets a **ScopeTek-family USB camera with a Micron MT9T001
sensor**, identified by USB vendor ID `0547` and product ID `4d33`. The commercial
model name has not been confirmed. Support is scoped to the single laboratory
unit used during development, not to every device sharing these identifiers.

| Property | Current scope |
| --- | --- |
| Distributed operating system | Windows x64 |
| USB access | Existing WinUSB binding; no automatic driver installation or replacement |
| Live mode | 1024 × 768 |
| Full-sensor capture | 2048 × 1536, followed by restoration of applied Live settings |
| Sensor data | GRBG Bayer; upper 8 bits of the sensor's 10-bit data path |
| Image controls | Exposure and gain; processed-image brightness, white balance, gamma, rotation, flips, and monochrome |
| Exports | Processed PNG/TIFF, original Bayer TIFF, RAW, and metadata |
| Timestamp semantics | Host UTC at complete-frame acceptance; not sensor exposure time |
| Frame-rate semantics | Acquisition and preview update rates reported separately; no guaranteed or optically calibrated rate |

The GUI filters physical candidates by USB ID. Enumeration is not proof of a
compatible sensor, successful acquisition, or hardware qualification. Confirm
USB identity, sensor, firmware compatibility, and driver setup before treating a
second unit as supported.

## Validation status

Pre-publication feature code was exercised for Live acquisition, displayed-image
export, full-sensor acquisition, Live restoration, and normal shutdown. Processed
PNG pixels were independently reconstructed from RAW, and original Bayer TIFF
pixels were checked against RAW. The published 0.1.0 binaries were rebuilt after
publication naming/diagnostic-path changes and have not been physically retested.

Remaining work includes ordinary mouse interaction through Windows App,
color/CFA and optical field-of-view calibration, arbitrary window-size
persistence, and long-duration qualification. These checks remain open; source
or unit-test results do not substitute for hardware evidence.

## Other cameras and operating systems

Simulation is an explicit synthetic source and does not imply support for a
physical device. UVC webcams, other ScopeTek variants, and cameras using vendor
SDKs currently have no supported backend. No macOS or Linux physical-camera
release is offered.

The project aims to support multiple camera families through a common GUI and
acquisition model. Each future backend must document its supported device and
firmware identifiers, operating systems, dependency redistribution terms,
controls, pixel formats, timestamp semantics, cleanup behavior, and validation
limits. Hardware support will be added to this matrix only after scoped testing.
