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

---

# リポジトリ構造の検証 / Repository-layout validation（日本語）

source layout migration は package と executable 名、dependency version、hardware access rule、image/acquisition behavior を変更しません。root lockfile は不変です。三つの package が root workspace を共有するため stale nested platform lock を削除しました。source identity は新しい build-input path に対して再生成しています。

## 既存の test failure / Existing test failures

macOS の full library suite は **144 passed, 2 failed, 1 ignored** でした。次の failure は、同じ locked dependency/toolchain を使う pre-migration commit の clean archive でも同一に再現します。

- `continuous_runtime::tests::shared_recipe_then_six_hours_live_single_init_exact_pattern_gates`
- `sensor_contract::tests::typed_raw_roundtrip_malformed_bytes_and_legacy_reading`

対応する Rust source file は移動前後で byte-identical です。pass を得るために numerical/image expectation、test assertion、acquisition behavior、ignored-test policy を変更していません。これは既存 issue であり、新たな hardware qualification ではありません。Windows distribution CI は既存の driver-policy、synthetic-helper、GUI、installer、metadata check を維持します。

installer Python test と source-identity verification は pass します。build/test command は root `Cargo.toml` から実行し、source package は `crates/` にあります。
