# KohdaLab Camera 0.1.13

## Changes / 変更内容

Add an opt-in, authenticated read-only GUI control adapter for describe/status/config and bundle the shared Python SDK. Correct stale test expectations against the existing public mode/export contract; document historical screenshot provenance.

## Compatibility and migration / 互換性と移行

Start with `--gui-api` to opt in. Capture/start/stop and ownership are intentionally unsupported by this adapter; the acquisition helper IPC remains private. Existing acquisition/export and app-specific config/install paths remain.

## Downloads / ダウンロード

Windows x64 Setup and portable ZIP, corresponding source and checksums. No Mac executable is published by this release. Windows artifacts remain unsigned.

[Release packages](https://github.com/woolen-yarn/kohdalab-camera/releases). Existing older releases remain available.

## Validation and provenance / 検証と出所

Local library: 147 passed, one hardware-dependent test ignored; GUI binary: 34 passed; common Rust contract: seven passed. Actual Mac fake GUI validated reads, authentication and unsupported mutations. Existing strict-clippy warnings are not presented as a passing check.

The common Python SDK has seven device-free tests. Source/build identifiers, package manifests and SHA-256 evidence accompany the release. These checks do not newly qualify physical instruments, optical closure, camera calibration or scientific accuracy.

## 日本語

共通GUI APIの読み取り専用窓口を追加しました。画像取得・停止等は公開APIの対象外で、既存の取得・保存動作は維持します。配布は未署名のWindows x64版です。Macの検証画面はMac版配布を意味しません。
