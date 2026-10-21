# Code structure and maintenance boundaries

The code separates camera transport, session ownership, image processing,
export and presentation. The GUI application still coordinates several concerns;
changes there should be small and checked against the existing regression tests.

| Concern | Entry points |
| --- | --- |
| Application state, worker integration and event handling | `src/main.rs` |
| Camera picker, presentation state and viewport transitions | `src/natural_ui.rs` |
| Inspector controls and appearance | `src/parity_ui.rs` |
| Fit, zoom and pan transforms | `src/image_view.rs` |
| Saved user settings, schema version and initial Live size | `src/preferences.rs` |
| Image corrections and display recipe | `src/display_recipe.rs` |
| Bounded background preview work | `src/preview_pipeline.rs` |
| Image/RAW/metadata export | `src/image_export.rs` |
| Session state machine, worker and lifecycle guardian | `src/session.rs` |
| Platform-specific continuous helper transport | `src/continuous_session/` |
| Driver eligibility | `src/driver_policy.rs` |
| Windows PE resource generation for both executables | `assets/windows_resources.rs` |
| Packaging and disposable-runner installation checks | `scripts/`, `.github/workflows/windows-unsigned.yml` |

## Preserving behavior during cleanup

- Keep default window size and saved-settings schema values in `preferences.rs`.
  Reuse them for initialization, persistence and reset. Changing a constant's
  value is a behavior/schema change, not a refactor.
- Resolve the selected physical device through the shared picker lookup. Keep
  connection-state, scan and installed-driver gates in their existing order;
  presentation must not grant connection or restart authority.
- Test-only constructors are compiled under `cfg(test)`. Production startup
  continues to use the normal initialization and preference-loading path.
- Keep camera ownership, shutdown deadlines, interlocks, sensor protocols,
  image formulas, export formats and existing error text outside incidental UI
  cleanup. These need separate, scoped changes and validation.
- Refresh `RECORDER-SOURCE-ID` after changing tracked build inputs. This records
  current source; do not change historical release or validation hashes.

## Regression tests

`src/app_tests.rs` contains the application regression tests as the existing
`tests` module, keeping names and assertions intact while leaving production
event handling easier to navigate. Focused UI, image-view and preference tests
remain with their respective modules. Cross-module scenarios remain in
`src/parity_tests.rs` and `src/integrated_ux_tests.rs`.

Run `cargo test --locked --bin camera-lab` for application-side changes. Windows
GUI tests also require the synthetic-frame helper and a new private fixture root;
the unsigned CI workflow supplies both. Unit/synthetic-frame checks do not
establish physical capture or optical qualification.
