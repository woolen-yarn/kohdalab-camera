# Maintained GUI stack

Camera Lab uses `eframe` 0.34.3 (and its matching `egui` 0.34.3 stack).
The declared Camera Lab MSRV is Rust 1.92, matching the GUI stack's MSRV.

Validation performed on the maintained stack:

- `cargo test --workspace --locked -q` on Rust 1.99.1: 149 library tests
  passed (one explicitly ignored), 38 GUI tests passed, and all remaining
  workspace tests passed. Loopback control-API tests were run with approved
  local loopback access.
- `RUSTFLAGS='-Ddeprecated' cargo check --workspace --locked` passed.
- Windows and macOS target dependency checks reported no `ttf-parser` package.

No physical-camera or hardware tests were run; that scope was operator-waived.

## 日本語

Camera Lab は `eframe` 0.34.3 と対応する `egui` 0.34.3 stack を使用します。
Camera Lab の宣言 MSRV は GUI stack の MSRV と同じ Rust 1.92 です。

- Rust 1.99.1 で `cargo test --workspace --locked -q` を実行し、library 149
  件（明示的に ignore された 1 件を除く）、GUI 38 件、および残りの workspace
  tests が通過しました。loopback control-API tests は承認済みの local loopback
  access で実行しました。
- `RUSTFLAGS='-Ddeprecated' cargo check --workspace --locked` は通過しました。
- Windows/macOS target dependency check は `ttf-parser` package がないことを確認しました。

physical camera/hardware test は実行していません。この範囲は operator により免除されています。

## Common SDK lease race / 共通SDKの制御権競合

The Python SDK now serializes lease transitions and rejects retired heartbeat
workers by generation. A heartbeat queued before voluntary release cannot poison
the next acquisition. Real active-lease revocation remains an error. Deterministic
regression tests cover both cases; the common SDK suite passes 16 tests.

Python SDKでは制御権の遷移を排他化し、終了したheartbeatを世代で判別します。
正常な解放の前に待機していたheartbeatが次の制御権取得を誤って失敗させる競合を修正しました。
実際の制御権喪失は引き続きエラーとし、両ケースの再現試験を含む共通SDK 16件が成功しています。

## Hidden-window processing / ウィンドウ非表示時の処理

Background acquisition, API requests, stop handling and worker results run in
`App::logic`, separately from visible rendering in `App::ui`. This is required by
eframe 0.34 when a window is hidden or covered. A regression drives the logic
without drawing and verifies that worker/acquisition progress is still consumed.

測定・API・停止・ワーカー結果の処理を`App::logic`へ分離し、表示は`App::ui`で行います。
eframe 0.34で画面が非表示・被覆された場合も処理を継続するための対応です。
描画を実行せずlogicだけを動かし、処理の進行を受け取れることを回帰試験で確認します。
