# Production panic-path review: session and continuous session

This is an analysis record. It changes no code, lifecycle ownership, cleanup
authority or error mapping. It lists every explicit panic site in the
production code of two files and assigns each one a follow-up class. Each class
maps to required validation and to an independent follow-up pull request (PR).

## Scope and baseline

- Baseline commit: `f6cc57d`. All reasoning below was done against the source
  at that commit.
- Files: `crates/camera-lab/src/session.rs` and
  `crates/camera-lab/src/continuous_session/windows.rs`.
- Counted forms: `.unwrap()`, `.expect(`, `panic!`, `unreachable!`, `todo!`,
  `unimplemented!`, `assert!`, `assert_eq!`, `assert_ne!`. Not counted:
  `unwrap_or*`, `.expect_eof(`, `?`, and implicit panics (indexing, arithmetic
  overflow, allocation failure).
- Production code at `f6cc57d`:
  - `session.rs` lines 1–983. Lines 985–2599 are seven `cfg(test)` modules and
    are excluded. Lines 573–611 (`Physical`, a `cfg(test)` test double) lie
    inside the production range but contain no counted form.
  - `windows.rs` lines 1–1984. The test module (1986–2640) and the `cfg(test)`
    helpers after it (2642–2720) are excluded. Lines 31–32 (`cfg(test)` import)
    contain no counted form.
- Issue #21 moves the `session.rs` test modules into
  `crates/camera-lab/src/session_tests/` and leaves lines 1–983 byte-identical.
  Production line numbers are therefore the same before and after that move.
  `windows.rs` is unchanged.

## Counts

| File | Production | Of which fixture-only paths | Excluded `cfg(test)` |
| --- | ---: | ---: | ---: |
| `session.rs` | 15 (all `.unwrap()`) | 0 | 310 |
| `continuous_session/windows.rs` | 19 (all `.unwrap()`; 18 lines, line 763 has two) | 7 | 220 |

"Fixture-only" means production code that runs only for a `Fixture` request
(lines 997, 1199, and `fake_backend` 1693–1866). It is compiled into release
builds but is never reached by the physical path.

## Reproducible counts

Run from the repository root. `grep -noE` prints one `line:match` row per match
(not per line), so `awk -F: '$1 …'` can select by line number.

```sh
P='\.unwrap\(\)|\.expect\(|panic!|unreachable!|todo!|unimplemented!|assert(_eq|_ne)?!'
S=crates/camera-lab/src/session.rs
W=crates/camera-lab/src/continuous_session/windows.rs

# Baseline f6cc57d: session.rs
git show "f6cc57d:$S" | grep -noE "$P" | awk -F: '$1 <= 983' | wc -l               # production: 15
git show "f6cc57d:$S" | grep -noE "$P" | awk -F: '$1 >= 985' | wc -l               # excluded tests: 310
git show "f6cc57d:$S" | grep -noE "$P" | awk -F: '$1 >= 573 && $1 <= 611' | wc -l  # cfg(test) double: 0

# Baseline f6cc57d: windows.rs
git show "f6cc57d:$W" | grep -noE "$P" | awk -F: '$1 <= 1984' | wc -l              # production: 19
git show "f6cc57d:$W" | grep -noE "$P" | awk -F: '$1 >= 1986' | wc -l              # excluded tests/helpers: 220
git show "f6cc57d:$W" | grep -noE "$P" \
  | awk -F: '$1 == 997 || $1 == 1199 || ($1 >= 1693 && $1 <= 1866)' | wc -l        # fixture-only: 7

# Working tree after #21 (production lines unchanged; tests moved)
grep -noE "$P" "$S" | wc -l                                                         # production: 15
grep -rnoE "$P" crates/camera-lab/src/session_tests | wc -l                         # moved tests: 310
grep -noE "$P" "$W" | awk -F: '$1 <= 1984' | wc -l                                  # production: 19
grep -noE "$P" "$W" | awk -F: '$1 >= 1986' | wc -l                                  # excluded: 220
```

Numbered variants: replace `| wc -l` with `| cut -d: -f1 | uniq -c`. This prints
`count line` for every line with at least one match. For example, line 763 of
`windows.rs` shows a count of 2. For the moved test files use
`| cut -d: -f1,2 | uniq -c`, which keeps the file name.

## Follow-up classes

- **K — keep as invariant.** The value is `Some`/`Ok` by construction,
  including constant-range slices of fixed arrays converted with `try_into`.
  No behavior change is needed.
- **R — propagate `Result`.** Route the failure into an error path that
  already exists, such as `?`/`map_err`, `fault(..)` + `break`, the helper's
  `sender_error`, or a returned `Outcome`. The existing collection and cleanup
  proof then still runs.
- **F — use `failure_record` when cleanup is unproved.** After the failure, the
  normal path can no longer prove cleanup. The follow-up must avoid panicking
  on the caller and must mark the state unproved with restart disabled, so the
  session `failure_record::Recorder` persists the first failure.

## Classification

### `session.rs`

| Line | Site | Class | Follow-up PR | Reasoning at `f6cc57d` |
| ---: | --- | :---: | --- | --- |
| 301 | `self.active.as_mut().unwrap()` | K | PR-1 (optional) | Inside `if let Some(a) = self.active.as_mut()`. Neither `stop()` nor `fail()` clears `active`. |
| 302 | `self.active.as_ref().unwrap()` | K | PR-1 (optional) | Same block. |
| 304 | `self.active.as_ref().unwrap()` | K | PR-1 (optional) | Same block. |
| 305 | `self.active.as_ref().unwrap()` | K | PR-1 (optional) | Same block. |
| 306 | `self.active.as_ref().unwrap()` | K | PR-1 (optional) | Same block. |
| 310 | `self.active.as_ref().unwrap()` | K | PR-1 (optional) | `Frame` arm; `active` is unchanged since 301. |
| 368 | `self.active.take().unwrap()` | K | PR-1 (optional) | `Done` arm. The only `take()`; nothing later in the tick unwraps `active`. |
| 399 | `self.active.as_ref().unwrap()` | K | PR-1 (optional) | `Progress` arm. |
| 800 | `state.lock().unwrap()` | F | PR-2 (verify only) | Worker thread inside `catch_unwind`. A poison panic reaches the existing `WorkerPanic` path, which records through `recorder.observe` and `finish` and retains the owner. PR-2 adds a test; no change is expected. |
| 833 | `state.lock().unwrap()` | F | PR-2 (verify only) | Same as 800. |
| 919 | `self.shared.lock().unwrap()` | R | PR-2 | `Worker::request_at` already returns `Result`. On poison, return the existing "restart disabled" error instead of panicking the caller. |
| 946 | `self.shared.lock().unwrap()` | F | PR-2 | `Worker::snapshot`. A poisoned lock means a panic happened while it was held, so cleanup is unproved. Recover with `into_inner` (as `LifecycleGuardian` does) and present Error with restart disabled; the recorder keeps the first failure. |
| 953 | `self.shared.lock().unwrap()` | F | PR-2 | `Worker::set_wake`. Same as 946. |
| 959 | `self.shared.lock().unwrap()` | F | PR-2 | `Worker::close_at`. Same as 946; closing must still request stop. |
| 976 | `self.shared.lock().unwrap()` | F | PR-2 | `Worker::closed`. Same as 946; must not report `closed` unless the worker set it. |

### `continuous_session/windows.rs`

| Line | Site | Class | Follow-up PR | Reasoning at `f6cc57d` |
| ---: | --- | :---: | --- | --- |
| 191 | `raw[80..112].try_into().unwrap()` | K | — | Fixed array: `[u8; 128]` to `[u8; 32]`. |
| 192 | `raw[112..120].try_into().unwrap()` | K | — | Fixed array: to `[u8; 8]`. |
| 528 | `state.snapshot.lock().unwrap()` | K | — | `State` was created on the previous line and is not yet shared, so the lock cannot be poisoned. |
| 763 | `b.bytes().unwrap()` | K | — | The same `Bootstrap` value was already serialized with `?` at line 665. |
| 763 | `os::digest(..).unwrap()` | R | PR-3 | Runs after `os::launch_helper` (line 666), before the STOP writer and supervisor exist, with no `catch_unwind`. The follow-up computes this digest in `startup()` before the launch, where failures are already `io::Result` and no child exists. If it cannot be moved, reclassify as F: fault as `Terminal::Unproved` and keep the interlock. |
| 963 | `raw[68..].try_into().unwrap()` | K | — | Fixed array: to `[u8; 60]`, as `measurement::decode` requires. |
| 992 | `raw[68..76].try_into().unwrap()` | K | — | Fixed array: 8 bytes. |
| 993 | `raw[76..78].try_into().unwrap()` | K | — | Fixed array: 2 bytes. |
| 994 | `raw[78..80].try_into().unwrap()` | K | — | Fixed array: 2 bytes. |
| 997 | `b.settings().unwrap()` | R | PR-4 | Fixture `TypedFrames`. Replace with `fault(&state, ..)` + `break`, the same as the neighbouring "Invalid frame header" path. |
| 1009 | `b.settings().unwrap()` | R | PR-4 | Physical frame path. Same as 997. |
| 1041 | `frame_contract(&b, seq).unwrap()` | R | PR-4 | `frame_binding_valid` at line 1014 already evaluated `frame_contract_hash` for the same `(b, seq)`. Route any failure to fault + break anyway. |
| 1199 | `b.output.parent().unwrap()` | K | — | Fixture `UnlockFail`. `output` is `root.join("session-…")` and was validated as absolute. |
| 1545 | `frame_contract(&b, ..).unwrap()` | R | PR-5 | Helper main thread. On failure, use the existing `sender_error = true; monitor.stop(); continue` path from line 1547. |
| 1696 | `b.fixture.unwrap()` | K | — | `fake_backend` runs only in `session-fake` mode, and `validate(true)` there requires `fixture.is_some()`. |
| 1716 | `b.settings().unwrap()` | R | PR-5 | Fixture backend worker. A panic is already mapped to "Backend panicked before release" by `worker.join().unwrap_or(..)` at line 1582. Return an `Outcome` failure explicitly instead. |
| 1722 | `b.settings().unwrap()` | R | PR-5 | Same as 1716. |
| 1747 | `b.settings().unwrap()` | R | PR-5 | Same as 1716. |
| 1759 | `frame_contract(b, sequence).unwrap()` | R | PR-5 | Same as 1716. |

Class totals: K = 8 (`session.rs`) + 10 (`windows.rs`), R = 1 + 9, F = 6 + 0.
That is 15 + 19 = 34 production sites.

### Limits of this analysis

- For the R sites that re-derive values from the validated bootstrap (`b.settings()`,
  `frame_contract`, `os::digest`), reading the code found one plausible failure
  source: the operating system's SHA-256 provider (`capability_platform::digest`
  through `settings_hash`). `Acquisition::new` depends only on
  `Settings::validate` and `Geometry::for_mode`, which `b.settings()` already
  exercised. This is a code-reading result, not proof. No test injects a
  digest failure, and other causes were not exhaustively ruled out.
- What happens to the helper after a parent panic at line 763 was inferred from
  code, not observed. Unwinding leaves `end` unset, so the session never reports
  collection. The interlock marker is retained because `RetainedInterlock::drop`
  does not release it, and the helper is not reaped by this owner. Whether the
  helper always exits, and how quickly, is not demonstrated by any test.
- That a helper panic at line 1545 leads to "Missing terminal receipt" and a
  retained interlock was also inferred, not tested.

## Follow-up PRs and required validation

The proposed changes can be reviewed independently. The startup digest and
receipt-loop changes occupy separate sections of `parent()`. Re-run the counts
for each PR and verify that unrelated sites retain their disposition.

| PR | Scope (only) | Sites | Required validation |
| --- | --- | --- | --- |
| PR-1 (optional) | `session.rs` `Engine::tick_at`: bind `active` once instead of repeated `unwrap` (pure refactor) | 301–399 (K) | `cargo test --locked -p camera-lab --lib session::` on macOS/Linux and in the Windows GNU workspace job. `cargo clippy --workspace --all-targets`. No change to snapshot or state text. |
| PR-2 | `session.rs` `Worker` / `LifecycleGuardian` mutex-poison handling; uses the existing `failure_record::Recorder` | 800, 833 (verify), 919 (R), 946–976 (F) | New unit test that poisons `shared` from a panicking thread. Assert that `request` returns `Err`, that `snapshot` shows Error with `restart_disabled`, and that the caller thread does not panic. Existing `panicked_worker_is_terminal_and_rejects_new_requests` and `worker_open_panic_publishes_full_failure_and_failed_persistence` still pass. `first.json` / `collection.json` keep the first failure. |
| PR-3 | `windows.rs` `startup()` / `parent()` signature: compute the bootstrap digest before `launch_helper` | 763 digest (R) | Windows GNU workspace job with the `Normal`, `BadBootstrap`, `SettingsDigest` and `ParentThreadFail` fixtures. A digest failure must yield a startup error with no child launched and `collected` set. Injecting a digest failure needs a new test seam, which this PR must add or explicitly defer. No physical-camera claim. |
| PR-4 | `windows.rs` `parent()` receipt loop, frame branch (`raw[3] == 5`) | 997, 1009, 1041 (R) | Windows GNU job with the `TypedFrames`, `FullFrames`, `XgaFrames` and `Normal` fixtures. Fault + break must still reach the mandatory wait, release and collection steps. The physical path (1009) cannot run in CI; state it as unvalidated on hardware. |
| PR-5 | `windows.rs` helper side: `helper_dispatch` frame forwarding and `fake_backend` | 1545, 1716, 1722, 1747, 1759 (R) | Windows GNU job fixtures (`TypedFrames`, `Normal`, `NoFrame`, `ProducerStall`). The terminal receipt, proof flags and helper exit code must match current expectations. The `Backend panicked before release` mapping is unchanged for real panics. |

Sites in class K outside PR-1 (191, 192, 528, 763 bytes, 963, 992–994, 1199,
1696) need no PR. The only validation for them is that the counts stay as
listed.

---

# 本番コードの panic 経路点検: session と continuous session（日本語）

この文書は分析記録です。コード、lifecycle ownership、cleanup authority、error mapping は変更しません。2 ファイルの本番コードにある明示的な panic 箇所をすべて挙げ、それぞれに後続対応の区分を割り当てます。各区分には、必要な検証と独立した後続 PR の範囲を対応させます。

## 範囲と基準 / Scope and baseline

- 基準コミットは `f6cc57d` で、以下の判断はすべてこのコミットのソースに基づきます。
- 数える形: `.unwrap()`、`.expect(`、`panic!`、`unreachable!`、`todo!`、`unimplemented!`、`assert!`、`assert_eq!`、`assert_ne!`。
- 数えないもの: `unwrap_or*`、`.expect_eof(`、`?`、暗黙の panic。
- 本番コード:
  - `session.rs` は 1–983 行です。985–2599 行は `cfg(test)` の 7 module で除外します。573–611 行（`cfg(test)` の `Physical`）に対象箇所はありません。
  - `windows.rs` は 1–1984 行です。test module（1986–2640）と後続の `cfg(test)` helper（2642–2720）は除外します。
- #21 の後も 1–983 行はバイト単位で同一なので、行番号は変わりません。

## 件数 / Counts

| ファイル | 本番 | うち fixture 専用 | 除外 `cfg(test)` |
| --- | ---: | ---: | ---: |
| `session.rs` | 15（すべて `.unwrap()`） | 0 | 310 |
| `continuous_session/windows.rs` | 19（18 行。763 行に 2 箇所） | 7 | 220 |

英語版の [Reproducible counts](#reproducible-counts) で件数を再確認できます。行番号付きの一覧は、`| wc -l` を `| cut -d: -f1 | uniq -c` に替えると得られます（移動後の test ファイルでは `-f1,2`）。

## 区分 / Classes

- **K（不変条件として維持）**: 構造上 `Some`/`Ok` が保証される箇所です。固定長配列への `try_into` もここに含めます。
- **R（`Result` で伝播）**: 既存の error 経路（`?`、`fault` + `break`、helper の `sender_error`、`Outcome`）に流し、既存の回収・cleanup 証明を維持します。
- **F（cleanup 未証明時は `failure_record`）**: 失敗後に通常経路では cleanup を証明できない箇所です。呼出し側で panic させず、unproved かつ restart 無効の状態にして、`failure_record::Recorder` に最初の失敗を残します。

## 分類 / Classification

| ファイル | 行 | 区分 | PR |
| --- | --- | :---: | --- |
| `session.rs` | 301, 302, 304, 305, 306, 310, 368, 399 | K | PR-1（任意） |
| `session.rs` | 800, 833 | F（既存の `WorkerPanic` 経路。確認のみ） | PR-2 |
| `session.rs` | 919 | R | PR-2 |
| `session.rs` | 946, 953, 959, 976 | F | PR-2 |
| `windows.rs` | 191, 192, 963, 992, 993, 994（固定長配列）、528、763 の `b.bytes()`、1199、1696 | K | なし |
| `windows.rs` | 763 の `os::digest`（launch 前へ移す。移せなければ F） | R | PR-3 |
| `windows.rs` | 997, 1009, 1041 | R | PR-4 |
| `windows.rs` | 1545, 1716, 1722, 1747, 1759 | R | PR-5 |

区分ごとの合計: K = 18、R = 10、F = 6。合計 34 箇所です。

## 分析の限界 / Limits

- bootstrap から再計算する R 箇所の失敗源として、コードを読む限り見つかったのは OS の SHA-256 provider（`capability_platform::digest`）だけです。`Acquisition::new` は `b.settings()` が既に通した `Settings::validate` と `Geometry::for_mode` にしか依存しません。ただしこれはコード読解の結果で、証明ではありません。digest 失敗を注入する test はなく、他の原因も網羅的には排除していません。
- 763 行で parent が panic した後の helper の挙動（`end` が立たず未回収のまま残る、interlock marker が保持される、この owner は helper を reap しない）は推定です。helper が必ず終了するか、どれだけ早く終了するかは test で示されていません。
- 1545 行の helper panic が `Missing terminal receipt` と interlock 保持につながるという点も、推定であり test で確認していません。

## 後続 PR と必要な検証 / Follow-up PRs and validation

各変更は個別にレビューできます。起動時の digest と受信ループの変更は `parent()` 内の別の箇所を扱います。各 PR で件数を再集計し、対象外の箇所の扱いを維持します。

- **PR-1（任意）**: `Engine::tick_at` の純粋な refactor です。検証: macOS/Linux と Windows GNU job での `cargo test --locked -p camera-lab --lib session::`、および `cargo clippy`。
- **PR-2**: `Worker` の mutex poison 処理です。検証: poison を起こす新しい test で、`request` が `Err` を返すこと、`snapshot` が Error/restart 無効を示すこと、呼出し側が panic しないことを確認します。既存の panic test 2 件と、`first.json`/`collection.json` の最初の失敗の保持も確認します。
- **PR-3**: digest を `launch_helper` の前に移します。検証: Windows GNU job の `Normal`/`BadBootstrap`/`SettingsDigest`/`ParentThreadFail` fixture。digest 失敗時は child が起動されないことを確認します。失敗注入の seam は追加するか、明示的に先送りします。
- **PR-4**: receipt loop の frame 分岐です。検証: `TypedFrames`/`FullFrames`/`XgaFrames`/`Normal` fixture で、必須の wait/release/回収まで到達することを確認します。物理経路（1009）は CI では実行できず、実機では未検証と明記します。
- **PR-5**: helper 側（`helper_dispatch` と `fake_backend`）です。検証: `TypedFrames`/`Normal`/`NoFrame`/`ProducerStall` fixture で、terminal receipt、proof flag、exit code が現状どおりであることを確認します。
