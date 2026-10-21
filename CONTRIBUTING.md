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

See [code structure and maintenance boundaries](docs/code-structure.md) before
moving responsibilities or shared defaults between modules.

Read the relevant implementation before editing. Use targeted tests appropriate
to the change; documentation-only edits do not require physical camera runs.
Do not change recorded results, thresholds, or historical checksums to make a
check pass. Preserve license notices in source and portable distributions.

## Native input diagnostics

For a bounded, simulation-only investigation of input delivery, build the
application with `--features input-diagnostics`. Set `KOHDALAB_INPUT_TRACE` to a
new writable file before launch. The diagnostic build disables physical camera
selection and the physical-adapter CLI path, and requests normal closure after
180 seconds. Do not use this feature in release distributions.

Only pointer, focus, and navigation-key metadata are recorded. Text and paste
contents are omitted; each frame inspects at most 16 events and the file is
capped at 256 records of at most 4096 characters. A diagnostic trace can establish
which events reached the application; it cannot establish that an external
remote-input tool actually sent a click.

After changing build inputs, run `python3 scripts/update_source_identity.py`
before the release build. The diagnostic marker is supplementary; release
manifest commit and binary hashes identify the distributed artifacts.
