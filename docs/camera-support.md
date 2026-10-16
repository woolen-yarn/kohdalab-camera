# Camera support and validation

## Current physical backend

### SHODENSHA GR300

The current supported commercial camera is **SHODENSHA GR300**, identified by the
owner of the tested laboratory unit. Manufacturer and camera model are the
primary identifiers; sensor and USB information are supplementary details.

The tested unit uses a **Micron MT9T001** sensor and USB ID **`0547:4d33`**. It
reports `3.1M USB2.0 Camera` as its USB product name and is handled by the current
ScopeTek-family backend. The backend family is a technical classification, not
the product's manufacturer name.

Windows driver metadata currently uses "KohdaLab" as a registration name; it
does not replace the SHODENSHA product identity. Support is scoped to this tested
unit. Other GR300 revisions, GR300-series variants, or devices sharing the same
USB identifiers or sensor are not automatically qualified.

| Property | Current scope |
| --- | --- |
| Manufacturer / model | SHODENSHA GR300 (tested unit, owner-confirmed identity) |
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

The exact published version 0.1.9 binaries completed 399.4 seconds of physical
Live acquisition, image adjustment and Fit controls, a pixel-checked endpoint
export, and normal GUI/helper shutdown. A single supported camera was
automatically selected. See [the scoped record](validation-0.1.9.json).

The owner confirmed manual mouse use, arbitrary resizing and retention of the
changed size during earlier checks. See [the earlier record](validation-0.1.4.json).

The owner subsequently reported acceptable color and field of view by visual
inspection. Optical/color calibration has not been performed. Full-sensor capture/Live restoration were checked in pre-publication
feature code but not repeated in this finite run. Picker round-trip size
retention was not repeated. Earlier records retain their original scope and
runtime hashes.

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
