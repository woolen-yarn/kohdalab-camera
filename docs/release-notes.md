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
