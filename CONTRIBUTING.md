# Contributing

Use English for project documentation, user-facing interface text, issue and
pull-request descriptions, and new code comments. Preserve third-party license
texts in their original form. Non-English test fixtures may remain when needed
to exercise Unicode handling.

## Adding camera support

KohdaLab Camera aims to provide one capture workflow across multiple camera
families. Before implementing a backend, document:

- Manufacturer, commercial model, USB/device identifiers, sensor, and firmware scope.
- The public API, protocol documentation, or redistributable SDK to be used,
  including provenance and dependency licenses.
- Supported operating systems, setup requirements, acquisition modes, controls,
  and pixel formats.
- Timestamp and frame-rate semantics, original-data preservation, ownership,
  shutdown, and failure behavior.
- A bounded hardware-validation plan and the exact limits of any resulting claims.

Do not incorporate manufacturer binary reverse-engineering results, private USB
traffic captures, proprietary initialization data, or legacy application assets.
Keep vendor-specific details behind backend boundaries. Simulation and unit
tests help development but do not qualify physical devices. Update
[the support matrix](docs/camera-support.md) only to the extent supported by
actual validation.

## Changes and validation

Read the relevant implementation before editing. Use targeted tests appropriate
to the change; documentation-only edits do not require physical camera runs.
Do not change recorded results, thresholds, or historical checksums to make a
check pass. Preserve license notices in source and portable distributions.
