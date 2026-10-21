# Extending the camera platform

The next priority is support for cameras from other manufacturers. Only the
SHODENSHA GR300 configuration described in the compatibility documentation is
currently supported. This document distinguishes existing boundaries from work
that must precede a second physical backend; it does not announce new support.

## Current foundation and remaining coupling

| Area | Existing foundation | Next boundary to establish |
| --- | --- | --- |
| Discovery | `device_catalog.rs` shares USB candidate identification between the picker and startup report | Per-backend discovery, including devices that do not enumerate through USB |
| Connection | Installed-driver checks and strict physical identity checks | Select a documented backend compatible with the installed driver |
| Acquisition | `session::Factory` and `Transport`, bounded workers and lifecycle ownership | Backend-specific settings and supported operations |
| Controls | GR300 modes, exposure in sensor rows and gain in eighths | Explicit units, ranges, steps and availability for each backend |
| Frames | Validated RAW frames, acquisition identity and display recipes | Explicit format, dimensions, stride, bit depth and color interpretation for each backend |
| Timing | Host-side frame observations and acquisition-rate diagnostics | Explicit clock domains and documented device timestamp semantics |
| Presentation | Shared picker, image view, image adjustments and export flow | Show controls supported by the selected backend without exposing transport details |

The catalog is deliberately a discovery list. A matching USB ID does not grant
permission to open a device or prove compatibility. Existing sensor, firmware,
driver and physical-location checks remain in the backend. Adding a catalog
entry alone is insufficient to add a camera.

## First additional camera: a vertical implementation

Choose one actual camera with a documented acquisition interface and acceptable
SDK and redistribution terms. Record the manufacturer, model, operating system,
driver/SDK version and documentation provenance before implementing transport.
Do not infer protocols from manufacturer binaries or private USB captures.
Keep SDK integration deferred when the necessary API and license are unavailable.

Implement discovery, connection, one validated frame format, live view, saving
and normal disconnect for that device before designing a general plugin system.
Use the experience of two working backends to define the shared contract. Avoid
loading arbitrary vendor DLLs or allocating hardware resources merely to build
the picker. Preserve existing drivers; a driver incompatible with a backend
must produce a clear explanation rather than a silent replacement or fallback.

### Settings and capabilities

Keep device commands and encodings inside the backend. GR300 `exposure_rows`
and `gain_eighths` are not universal camera settings. Do not relabel rows as
milliseconds or reuse a numeric range for another sensor.

A future capability description should report supported acquisition modes,
frame formats, control units/ranges/steps, read-only values, and whether a change
requires stopping acquisition. Validate settings inside the backend as well as
in the GUI. Store preferences under a stable backend/model identity with a
schema version; never apply another model's settings just because a USB product
string looks similar. Migration must retain current GR300 defaults and behavior.

### Ownership, data flow and responsiveness

Retain a single session owner and the current shutdown guardian. A new backend
must honor stop/disconnect deadlines, reject stale frame generations and keep
driver calls off the UI thread. Vendor callbacks must not update GUI state
directly. Treat borrowed SDK buffers as valid only for their documented lifetime;
copy or transfer ownership before the next SDK operation can reuse them.

Keep preview work bounded and allow obsolete preview work to be superseded.
Image export and future analysis must use an immutable frame snapshot with its
matching settings and recipe. Do not extend preview dropping rules to a future
lossless recording feature without an explicit recording contract.

### Acceptance for each backend

- Test settings validation, unsupported controls, frame layout and conversion
  using deterministic fixtures without opening hardware.
- Test connection failures, stale callbacks, disconnect during acquisition and
  shutdown through a backend substitute.
- Qualify the actual model/driver combination with finite hardware runs covering
  live view, applying supported settings, export, disconnect and reconnect.
  Record image dimensions, pixel integrity and normal cleanup separately from
  optical quality and performance observations.
- Publish a compatibility matrix with tested versions, known limitations and
  driver requirements. Compilation or a matching USB ID is not qualification.

## Later measurement and instrument extensions

The current `measurement.rs` records capture observations such as host timing
and acquisition rate. Keep that purpose separate from distance, area, intensity
or external-instrument measurements.

For image measurements, introduce a versioned result record associated with an
immutable frame identity (session, settings generation and frame sequence).
Record algorithm version, ROI coordinates, units and calibration identity.
Define mappings between image coordinates and zoom/pan/display coordinates;
measurements must not change when the user zooms. Physical units require a valid
calibration for the optical configuration and resolution. A display adjustment
recipe is not a calibration. Make the chosen analysis input explicit: sensor RAW
or a documented derived image, with its processing provenance.

External devices should have their own lifecycle and typed samples. Associate
samples with frames through explicit time/trigger information, including clock
domain, uncertainty and missing-data status. Host arrival time alone does not
prove simultaneous exposure or hardware synchronization. Consider injectable
clock sources when implementing that work, preserving existing timestamp
semantics until a separately tested migration is available.

Export these results as separate versioned metadata linked to the original
frame and calibration. Preserve existing RAW, image and recipe export semantics.
Keep the normal GUI concise: supported controls and essential state first,
diagnostic details on demand.

## Delivery order

1. **Implemented:** share the current discovery candidate definition without
   broadening connection eligibility or changing the GUI.
2. **Next:** select one documented additional camera and establish its driver,
   API, licensing and frame-format requirements.
3. **Then:** add backend-specific settings and capabilities while retaining GR300
   regression coverage; implement and qualify that second backend end to end.
4. **Later:** add calibrated image measurements and external-instrument timing
   as separate, reproducible data products.

No additional physical backend, generic capability-driven controls or scientific
measurement engine is implemented by the discovery refactor.
