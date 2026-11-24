# Quality improvement tasks / 品質改善タスク

This document specifies independent, implementation-ready tasks that came out of a
read-through of the repository (sources, CI workflows, documentation, and git
history). Each task is scoped for a single pull request and a single implementer
(for example Codex). Nothing here changes acquisition, image, or export behavior.

本書は、ソース・CI 設定・ドキュメント・git 履歴を実際に読んで洗い出した、独立
して着手できる実装タスクをまとめたものです。各タスクは 1 PR・1 担当者で完結す
る粒度です。撮像・画像・保存の挙動は変更しません。

Reviewed at commit `f6cc57d` on branch `main`. Findings cite file and line
numbers from that revision. / `main` の `f6cc57d` 時点のレビューです。根拠は同
リビジョンのファイル名・行番号です。

## Shared guardrails / 全タスク共通のガードレール

These apply to every task below and must not be violated:

- Do not change recorded validation results, thresholds, or historical checksums
  to make a check pass (`CONTRIBUTING.md`, "Changes and validation").
- Do not alter `RECORDER-SOURCE-ID` or release/validation hashes except through
  the documented source-identity flow (`docs/development/architecture.md`,
  "Preserving behavior during cleanup").
- Keep `nusb` pinned at 0.2.7; dependency bumps are a separate reviewed flow
  (`CONTRIBUTING.md`, "Dependency updates").
- Leave camera ownership, shutdown deadlines, interlocks, sensor protocols,
  image formulas, export formats, and existing error text untouched unless a
  task explicitly scopes such a change with its own validation
  (`docs/development/architecture.md`).
- Tests must not connect hardware, move stages, enable outputs, or alter drivers
  (`README.md`, "Development").
- Keep documentation bilingual (English + Japanese) per `CONTRIBUTING.md`.
- Every doc added under `docs/` must be reachable from `docs/README.md`, or
  `scripts/check_docs.py` (run by `.github/workflows/docs.yml`) fails.

## One task, one PR / 1 タスク 1 PR

Each task below maps to one branch and one pull request. Do not combine tasks.
Link the matching GitHub issue from the PR body. Recommended order is the task
numbering: start with the low-risk documentation tasks (1, 2), then CI additions
(6), then the staged lint and refactor work (3, 4, 5), and finally the optional
check (7).

着手順の推奨は番号順です。低リスクな文書タスク（1, 2）→ CI 追加（6）→ 段階的な
lint・分割作業（3, 4, 5）→ 任意の整合チェック（7）。

---

## Task 1 — Document the MSRV (1.92) in the CI reference / CI 文書に MSRV 1.92 を明記

**Priority: High · Risk: Low · Labels: documentation**

### Background / 背景

The workspace declares `rust-version = "1.92"` in `Cargo.toml:12`, and
`.github/workflows/test.yml:94-104` verifies that MSRV in a dedicated
`Rust 1.92 workspace check` job. Release builds, however, pin Rust `1.99.1`
(`.github/workflows/windows-unsigned.yml:38,56`,
`.github/workflows/unix-distribution.yml:22-25`,
`.github/workflows/distribute.yml:36`).

The CI reference only mentions the build toolchain. `docs/development/continuous-integration.md:10`
and its Japanese counterpart at `:72` state "Rust 1.99.1 ... selected explicitly"
with no mention that 1.92 is the declared MSRV verified in a separate job. A
reader of that page cannot tell the project maintains a two-track setup.
`docs/validation/maintained-gui-stack.md:4` already states the MSRV is 1.92, so
the gap is in the CI reference only.

### Files and scope / 変更対象と範囲

- `docs/development/continuous-integration.md` — add one sentence (English and
  Japanese) stating that MSRV is 1.92, verified by the `Rust 1.92 workspace
  check` job in `test.yml`, while release builds use 1.99.1.

Documentation only. Do not change any workflow or version number.

### Acceptance criteria / 受け入れ基準

- The CI reference names both Rust 1.92 (MSRV, verified by the dedicated job)
  and 1.99.1 (release build toolchain), in English and Japanese.
- The stated version numbers match `Cargo.toml:12` and the workflow files
  exactly; no version values are changed anywhere.
- `python3 scripts/check_docs.py` passes.

### Task-specific guardrails / 個別ガードレール

- Do not change the actual MSRV or any CI toolchain pin; this is a wording
  change that records the existing state.

---

## Task 2 — Add provenance headers to validation records / 検証記録に版・環境ヘッダを付与

**Priority: High · Risk: Low · Labels: documentation**

### Background / 背景

Device-free Rust test counts differ across validation records because each was
measured at a different revision, command, and platform, but the context is not
uniformly stated:

- `docs/validation/maintained-gui-stack.md:8-9` — 149 library, 38 GUI (Rust 1.99.1).
- `docs/validation/maintained-gui-stack.md:36` — common SDK 16 tests.
- `docs/validation/common-control-api.md:5-6` — 188 Rust, 9 Python SDK.
- `docs/validation/suite-quality-audit.md:42` — 195 Rust, 14 SDK.
- `docs/validation/repository-layout.md:11` — 144 passed, 2 failed, 1 ignored
  (the "2 failed" is an intentional pre-migration reproduction, only clarified
  at `:35`).

These are versioned measurement snapshots. `CONTRIBUTING.md` forbids rewriting
recorded results, so **the numbers must not be changed or reconciled.** The fix
is to make each record's scope legible.

### Files and scope / 変更対象と範囲

- `docs/validation/maintained-gui-stack.md`
- `docs/validation/common-control-api.md`
- `docs/validation/suite-quality-audit.md`
- `docs/validation/repository-layout.md`

Add a short provenance header (revision/commit, command run, platform,
measurement date) to the top of each Markdown record where one is missing or
incomplete. Optionally add one line in `docs/README.md` noting these are
independent point-in-time snapshots.

### Acceptance criteria / 受け入れ基準

- Each listed record states, near its top, the commit/version, the command, the
  platform, and the date of measurement, in English and Japanese.
- No numeric measurement value, threshold, pass/fail count, or JSON evidence is
  altered. (Diff touches prose headers only.)
- `python3 scripts/check_docs.py` passes.

### Task-specific guardrails / 個別ガードレール

- Treat every count and result as immutable history. Only add contextual prose.
- Do not touch any `*.json` evidence file.

---

## Task 3 — Make Clippy a staged CI gate / Clippy を段階的に CI ゲート化

**Priority: High · Risk: Medium · Labels: rust, enhancement**

### Background / 背景

The Clippy step in `.github/workflows/test.yml:85-91` only reports a warning
count to the job summary. It pipes output through `tee` and counts warnings with
`awk`; there is no `-D warnings` and the step cannot fail the job on lint
findings. `docs/validation/suite-quality-audit.md:42` confirms existing advisory
warnings (argument count, enum size, non-Windows unused code), so flipping to
`-D warnings` immediately would turn CI red.

The crate roots declare `#![forbid(unsafe_code)]` (`crates/camera-lab/src/lib.rs:1`,
`crates/camera-lab/src/main.rs:1`), but there are zero `#[allow(...)]` or
`#[expect(clippy::...)]` annotations in the sources, so current lint exceptions
are undocumented.

### Files and scope / 変更対象と範囲

Stage this; do not jump straight to a hard global gate.

1. Inventory the current Clippy warnings and annotate each known, accepted one
   with `#[expect(clippy::<lint>, reason = "...")]` at its site (sources under
   `crates/*/src`). Prefer small, per-file commits inside the single PR.
2. Then change `.github/workflows/test.yml` so the Clippy step fails on new
   warnings — either `-D warnings` once the tree is clean, or a scoped
   `-D clippy::<category>` set agreed with maintainers.

### Acceptance criteria / 受け入れ基準

- `cargo clippy --locked --workspace --all-targets` produces no unannotated
  warnings; every suppression uses `#[expect(...)]` with a `reason`.
- The CI Clippy step returns a non-zero exit code when a new, unannotated
  warning is introduced (demonstrated by a scratch commit in the PR description,
  then reverted).
- `cargo fmt --all --check` still passes.
- No behavior change: no edits to image formulas, export formats, ownership,
  shutdown, or error text while silencing lints.

### Task-specific guardrails / 個別ガードレール

- Keep edits mechanical. Per `docs/development/architecture.md`, GUI changes
  must stay small and be checked against existing regression tests; if silencing
  a lint would require logic changes in `session.rs` or `continuous_session/`,
  stop and leave a `#[expect]` with a note rather than refactoring here.
- Do not weaken `#![forbid(unsafe_code)]`.

---

## Task 4 — Split tests out of the largest source files / 最大ファイルの本体とテストを分離

**Priority: Medium · Risk: Medium · Labels: rust**

### Background / 背景

`camera-lab` is ~27,776 lines across 54 Rust files. Several files mix production
code and a large in-file `#[cfg(test)]` module:

- `crates/camera-lab/src/session.rs` — 2,599 lines; `#[cfg(test)]` begins at
  line 2 (the file is almost entirely one test module plus production logic).
- `crates/camera-lab/src/physical.rs` — 2,508 lines; `#[cfg(test)]` at line 474.
- `crates/camera-lab/src/display_recipe.rs` — 1,178 lines; `#[cfg(test)]` at 435.
- `crates/camera-lab/src/image_export.rs` — 1,036 lines; `#[cfg(test)]` at 452.

`docs/development/architecture.md` ("Regression tests") already records a
deliberate choice to keep focused tests in their modules and cross-module
scenarios in `app_tests.rs`/`parity_tests.rs`/`integrated_ux_tests.rs`. This task
stays inside that intent: move only the in-file unit-test modules of the very
largest files into sibling test files, preserving names and assertions.

### Files and scope / 変更対象と範囲

Start with `session.rs` alone (smallest-blast-radius candidate for this PR):

- Move its `#[cfg(test)]` module to a sibling file (e.g. `session_tests.rs`
  included via `#[path]`/`mod`), keeping every test name, module path, and
  assertion identical.

Leave the other large files for follow-up PRs under this same task family.

### Acceptance criteria / 受け入れ基準

- `cargo test --locked --bin camera-lab` runs the exact same set of tests, with
  identical names, before and after (compare `--list` output).
- No production logic, constants, or error text in `session.rs` changes.
- `cargo fmt --all --check` and the workspace build pass.

### Task-specific guardrails / 個別ガードレール

- Preserve test names and assertions verbatim (`architecture.md`).
- Do not change `preferences.rs` constants or any schema value (these are
  behavior/schema changes, not refactors).
- One file per PR; do not batch all four large files together.

---

## Task 5 — Catalog production panic paths in ownership/shutdown layers / 所有・shutdown 層のパニック経路を要求化

**Priority: Medium · Risk: Medium · Labels: rust**

### Background / 背景

A grep excluding test files still shows a high concentration of
`.unwrap()`/`.expect()`/`panic!`/`unreachable!` in the layers whose failure
behavior `CONTRIBUTING.md` asks implementers to document (shutdown, failure):

- `crates/camera-lab/src/session.rs` — 133 hits
- `crates/camera-lab/src/continuous_session/windows.rs` — 106 hits
- `crates/camera-lab/src/image_export.rs` — 69 hits
- `crates/camera-lab/src/preview_pipeline.rs` — 62 hits
- `crates/camera-lab/src/physical.rs` — 57 hits
- `crates/camera-lab/src/continuous_runtime.rs` — 57 hits

Note: the raw grep still counts some lines inside `#[cfg(test)]` blocks (for
example `session.rs` is mostly a test module), so the true production counts are
lower and must be measured precisely first.

`docs/development/architecture.md` warns that ownership, shutdown deadlines, and
interlocks need separate, scoped changes with their own validation. Therefore
**this task produces a requirements list, not code edits.**

### Files and scope / 変更対象と範囲

- Add a new analysis note under `docs/development/` (e.g.
  `panic-paths-review.md`) that, for `session.rs` and
  `continuous_session/windows.rs`:
  - reports the precise production (non-`cfg(test)`) count per file,
  - classifies each production panic site as "provably unreachable / invariant",
    "should become `Result`", or "should route through `failure_record.rs`",
  - proposes follow-up PR boundaries with the validation each would need.
- Link the new note from `docs/README.md`.

No source code changes in this task.

### Acceptance criteria / 受け入れ基準

- The note gives an exact, reproducible count command and its output for the two
  files, separating test from production lines.
- Every production panic site in those two files is listed with a proposed
  disposition and the validation a future change would require.
- Bilingual, and reachable from `docs/README.md`; `scripts/check_docs.py` passes.

### Task-specific guardrails / 個別ガードレール

- Do not change ownership, shutdown deadlines, interlocks, or error text in this
  PR (`architecture.md`). This is analysis only.
- Do not relax any existing test or threshold.

---

## Task 6 — Add Python lint/type checks for the control binding / Python バインディングに lint・型チェックを追加

**Priority: Medium · Risk: Low · Labels: enhancement**

### Background / 背景

The Python control binding under `bindings/control/` has unit tests wired into
CI (`.github/workflows/windows-unsigned.yml:81` and
`.github/workflows/distribute.yml:66` run
`python -m unittest discover -s bindings/control`), but there is no Python lint,
formatter, or type-check anywhere in `.github/workflows/` (a search for
`ruff|pytest|mypy|flake8|black` returns nothing). The Rust side already gates
formatting with `cargo fmt --all --check` (`test.yml:84`), so Python lacks an
equivalent gate, even though `CONTRIBUTING.md` sets English-first style
expectations for new code.

### Files and scope / 変更対象と範囲

- Add a lightweight CI job (new workflow or a job appended to an existing
  Python-triggered workflow) that runs `ruff check` and `ruff format --check`
  over `bindings/control/`, optionally `mypy`.
- Add the matching tool config (e.g. `[tool.ruff]`) to
  `bindings/control/pyproject.toml`.
- Reuse the same path/trigger filters the existing binding tests use so the job
  only runs when `bindings/control/**` changes.

### Acceptance criteria / 受け入れ基準

- CI runs `ruff` (and any added type check) against `bindings/control/` on
  changes to that path, and fails on violations.
- The existing `python -m unittest` binding tests continue to run unchanged.
- No change to Rust build/release workflows, release logic, or the SDK's public
  behavior.
- `kohdalab_control.py` and `test_control.py` pass the new checks (fix only
  style/type issues the tools flag; no functional change).

### Task-specific guardrails / 個別ガードレール

- Do not alter the SDK's runtime behavior, protocol framing, or lease logic to
  satisfy a linter; prefer config or minimal, behavior-neutral edits.
- Keep third-party license texts untouched.

---

## Task 7 — Optional doc/version consistency check / 文書とバージョン整合の軽量チェック（任意）

**Priority: Low · Risk: Low · Labels: documentation, enhancement**

### Background / 背景

`scripts/check_docs.py` (run by `.github/workflows/docs.yml:17`) verifies local
link integrity and reachability from `docs/README.md`, but not content
consistency. The version-string gap in Task 1 and the un-headered records in
Task 2 are both invisible to current CI. A small, separate checker could catch
future drift between documented Rust versions and the actual
`Cargo.toml`/workflow pins.

### Files and scope / 変更対象と範囲

- Add a new script (e.g. `scripts/check_doc_versions.py`) that asserts any
  "Rust X.Y(.Z)" string in `docs/development/continuous-integration.md` is one
  of: the MSRV in `Cargo.toml` or a toolchain pin present in
  `.github/workflows/*.yml`.
- Wire it into `.github/workflows/docs.yml` as an additional step.

Keep it deliberately narrow; do not try to validate test counts (those are
immutable historical records per Task 2).

### Acceptance criteria / 受け入れ基準

- The script passes on the current tree after Task 1 lands, and fails if a
  documented Rust version no longer matches `Cargo.toml` or any workflow pin.
- The check is additive; it does not modify any file it inspects.
- `docs.yml` runs the new step and the existing link check still runs.

### Task-specific guardrails / 個別ガードレール

- Do not assert on historical validation numbers or JSON evidence.
- The checker must be read-only (no file rewrites).

---

## Summary table / 一覧

| # | Task | Priority | Risk | Labels |
| --- | --- | --- | --- | --- |
| 1 | Document MSRV 1.92 in CI reference | High | Low | documentation |
| 2 | Provenance headers on validation records | High | Low | documentation |
| 3 | Staged Clippy CI gate | High | Medium | rust, enhancement |
| 4 | Split tests from largest source files | Medium | Medium | rust |
| 5 | Catalog production panic paths (analysis) | Medium | Medium | rust |
| 6 | Python lint/type checks for the binding | Medium | Low | enhancement |
| 7 | Optional doc/version consistency check | Low | Low | documentation, enhancement |

---

## 日本語の補足

各タスクは 1 PR・1 担当で完結します。推奨着手順は番号順（低リスクな文書タスク
1・2 → CI 追加 6 → 段階的な lint・分割・分析 3・4・5 → 任意の整合チェック 7）。
数値・閾値・履歴 checksum の書き換え、`nusb` 0.2.7 の変更、所有権・shutdown・画像
式・保存形式・既存エラー文言の変更は、各タスクが明示的に scope しない限り行いま
せん。`docs/` 配下の新規文書は必ず `docs/README.md` から到達可能にし、
`scripts/check_docs.py` を通してください。
