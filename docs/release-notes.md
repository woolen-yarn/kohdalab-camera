# KohdaLab Camera 0.1.10

- Preserve existing Windows driver bindings. Reuse WinUSB only when it is the
  active binding; a different or missing driver receives an explanation instead
  of a silent replacement or unsupported acquisition attempt.
- Recheck the binding in the physical helper. Detect a missing helper before
  creating a physical-session interlock.
- Add `--startup-report new-file.json`, an enumeration-only support report that
  does not open USB or launch the acquisition helper.
- Add per-user Windows Setup tooling and a signature audit, with documented
  organization-signing prerequisites and honest unsigned-release status.

Official GR300 SDK integration is deferred at the owner's request. The existing
WinUSB backend remains the only implemented physical acquisition backend.
Version 0.1.9 retains the last five-minute physical validation; new 0.1.10 checks
cover driver policy, UI regression and Windows startup/installation behavior.

# KohdaLab Camera 0.1.9

- Distinguish the whole-image **Fit** view from magnification such as
  **2.0× Fit**, with an explanation of how it differs from 100%.
- Add a quick start with current Simulation screenshots and a responsiveness
  report separating CPU timings from UI-tool observation overhead.

The targeted GUI check, native Simulation image adjustment/export/normal close,
and exact saved-image replay passed. Windows x64 built successfully. Subsequent
physical validation of the exact published binaries completed 399.4 seconds of
GR300 Live with 15,701 accepted frames, image adjustment and Fit controls,
PNG/RAW/Bayer TIFF export, and normal Disconnect/Close. All saved pixels matched
independent RAW reconstruction; both processes exited normally without a
remaining protective lock. See [validation](validation-0.1.9.json) and
[observed responsiveness](responsiveness.md).

The initial zero-connection attempt was blocked by Windows App input delivery.
The owner opened the picker, and held mouse clicks then allowed the finite run
to proceed. Validation is supplemental to the original release archives; their
bytes and original manifests remain unchanged.

# KohdaLab Camera 0.1.8

- Show the installed application version in the camera-selection window title,
  Live window title, and Live toolbar.

This release did not repeat physical-camera acquisition. It was built and
published from the current Windows x64 source baseline.

# KohdaLab Camera 0.1.7

- Refine the dark interface: make the image the visual focus, use a restrained
  accent for available primary actions, and allow a selected toolbar tab to
  close its inspector.
- Make Fit the minimum zoom, add visible Fit / minus / magnification / plus /
  100% controls, and keep 100% unavailable when it would be below Fit or beyond
  the supported zoom range.
- Remove the reserved blank area before Cursor in the status bar and simplify
  Advanced details and duplicated display controls.
- Avoid idle repaint polling and reuse compiled display recipes until image
  controls change. Image and export semantics remain unchanged.

Validation included GUI, renderer, and preview-pipeline tests; native Simulation
interaction checks; and replay against a saved GR300 endpoint image. This
release did not repeat physical-camera acquisition, and its local CPU
measurements do not establish a physical capture-FPS improvement.

# KohdaLab Camera 0.1.6

- Remember camera/image settings, Live window size, save folder and capture
  numbering. Restore defaults from Advanced without automatically reconnecting
  or applying camera controls.
- Add pointer-centered wheel zoom, drag panning, double-click to fit, and Fit /
  100% buttons in Image. View transforms do not change exported pixels.
- Suggest numbered PNG names and skip occupied image/sidecar names.
- Render preview pixels directly into ColorImage and remove the unused retained
  image clone. This eliminates an intermediate RGB allocation and pixel copy;
  measured CPU timings vary, so no additional capture-FPS increase is claimed.

Validation includes exact direct/indirect pixel equivalence, bounded pipeline
checks, saved physical-image replay, preferences overwrite/retry, view geometry,
input isolation, no-auto-connect/reset tests, independent review and native Mac
Simulation interaction/restart/reset checks. The last physical Live validation
remains the scoped 0.1.4 run.

# KohdaLab Camera 0.1.5

- Set the default exposure to 120 sensor rows. Gamma remains 2.2 and brightness
  remains 1.20×.
- Record the owner's visual confirmation of acceptable color and field of view;
  this does not imply optical/color calibration.

The five-minute physical validation applies to 0.1.4. This default-setting update
was checked with the UI/integration suite and a Windows release build.

# KohdaLab Camera 0.1.4

- Automatically select a single supported camera after enumeration; open it with
  **Load camera**. Multiple cameras still require a choice, and a missing selected
  camera is never silently replaced by a different device.
- Shorten the opening screen instructions.

Validation: 26 UI tests and independent review passed. The exact Windows build
completed 336.2 seconds of real GR300 Live acquisition, a PNG/RAW/Bayer TIFF export
with independent pixel agreement, and normal GUI/helper exit. No process or
protective lock remained. The owner confirmed manual resizing and size retention.
Color/optical qualification and picker round-trip resizing remain outside this
run. See [the scoped validation record](validation-0.1.4.json).

# KohdaLab Camera 0.1.3

- Simplify the Live status bar and export panel. Move technical counters and
  startup diagnostics into a collapsed Diagnostics section.
- Keep capture FPS, simulation/retained-image labels and actionable errors visible.

- Reconstruct only the color neighbors required at each Bayer site, with layout
  validation once per render. RGB pixels, RAW bytes and recipe semantics remain
  unchanged. The synthetic Mac CPU benchmark reduced rendering time by about
  31–35%; this is not a Windows capture-FPS claim.
- Verify all pixels against the preceding real-camera export and retain exact
  orientation, scaling, color and malformed-input regression coverage.

Validation: 26 UI tests passed; rendering tests passed; simulation Image/Export
panels were visually checked with mouse clicks. The 0.1.3 Windows build has not
had a new physical-camera run. See [performance verification](rendering-performance.md).

## Additional 0.1.2 validation

The published build subsequently completed 333.2 seconds of Live acquisition on
SHODENSHA GR300, with a pixel-checked endpoint export and normal native shutdown.
Manual mouse operation was confirmed by the owner. The planned start export was
canceled, so the full two-endpoint export plan remains incomplete. See
[the later validation record](validation-0.1.2-five-minute.json).

The earlier observations below retain their original candidate scope.

# KohdaLab Camera 0.1.2

A focused stability update for the SHODENSHA GR300 backend.

- Remember valid Live window sizes as they change, and preserve them when native
  transition reports are missing or still describe the smaller camera picker.
- Keep the SIMULATION label visible on retained images after disconnect.
- Add a regression test for normal mouse press/release on all five Live tabs.
- Add optional, simulation-only input diagnostics with bounded pointer/navigation
  records, no typed/pasted text, and a normal-close deadline. This feature is
  disabled in the distributed application.

A short physical run observed Live acquisition, image and sidecar export,
independent pixel agreement, and native GUI/helper exit code 0. Postflight found
no remaining camera process or protective interlock and normal WinUSB status.
The 60-second preflight-to-Connect window was not proven, so this is not an
acceptance PASS. The final GUI binary additionally includes the retained-
simulation label fix and has not been physically retested. See
`docs/validation-0.1.2.json` for the exact scope.

Ordinary mouse input through Windows App, physical arbitrary-resize persistence,
and color/field-of-view calibration remain open. Long-duration testing is
deferred. No acquisition or image-processing algorithm was changed.
