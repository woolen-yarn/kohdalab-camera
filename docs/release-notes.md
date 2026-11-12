# KohdaLab Camera 0.1.24

## GUI control API / GUI 制御 API

For common-protocol requests, Camera returns transport-level `timeout`, `busy`
and `unavailable` errors with the original `request_id` and structured error
fields. The two-second request deadline still applies. A Windows save dialog
can delay status processing. Retry a read-only status request after the
dialog closes if needed.

共通 GUI API リクエストの通信エラー `timeout`、`busy`、`unavailable` は、
元の `request_id` と構造化エラーを含む形式で返します。リクエスト期限は引き続き
2秒です。Windows の保存ダイアログを開いている間は状態照会の処理が遅れる場合が
あります。必要に応じてダイアログを閉じてから状態を再取得してください。

## Distribution and validation / 配布と検証

Windows x64 Setup and portable ZIP are unsigned. The release includes a source
ZIP and SHA-256 checksums. The USB library remains nusb 0.2.7. The
[GR300 physical record](validation/physical-gr300-0.1.21.md) covers 0.1.21.
The [0.1.23 release](https://github.com/woolen-yarn/kohdalab-camera/releases/tag/v0.1.23)
records a separate GR300 trial. Validation for 0.1.24 uses CI with Simulation
and synthetic frames.

Windows x64 の Setup と portable ZIP は未署名です。ソース ZIP と SHA-256
チェックサムも配布します。USB ライブラリは nusb 0.2.7 です。
[GR300 実機検証記録](validation/physical-gr300-0.1.21.md)は 0.1.21 を対象とし、
[0.1.23 のリリース](https://github.com/woolen-yarn/kohdalab-camera/releases/tag/v0.1.23)
には別の実機試験を記録しています。0.1.24 は CI の Simulation と合成フレームで検証します。
