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
