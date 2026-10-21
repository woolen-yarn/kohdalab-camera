# Repository-layout validation

The source layout migration keeps package and executable names, dependency
versions, hardware access rules, and image/acquisition behavior unchanged.
The root lockfile is unchanged; the stale nested platform lock is removed because
all three packages share the root workspace. Source identity is regenerated for
the new build-input paths.

## Existing test failures

On macOS, the full library suite reported 144 passed, 2 failed, and 1 ignored.
The following failures reproduce identically from a clean archive of the
pre-migration commit, using the same locked dependencies and toolchain:

- `continuous_runtime::tests::shared_recipe_then_six_hours_live_single_init_exact_pattern_gates`
- `sensor_contract::tests::typed_raw_roundtrip_malformed_bytes_and_legacy_reading`

The corresponding Rust source files are byte-identical before and after the
move. No numerical/image expectation, test assertion, acquisition behavior, or
ignored-test policy was changed to obtain a passing result. These are existing
issues, not new hardware qualification. Windows distribution CI retains its
existing driver-policy, synthetic-helper, GUI, installer and metadata checks.

Installer Python tests and source-identity verification pass. Build and test
commands now run from the root `Cargo.toml`; source packages live in `crates/`.
