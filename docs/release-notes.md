# KohdaLab Camera 0.1.22

## Fixes and maintenance / 修正と保守

- On macOS, the read-only GUI API now waits within the existing two-second deadline for a local request to finish arriving. A loopback regression test covers requests that arrive in parts. / macOS の読み取り専用 GUI API は、ローカルのリクエストが分割して届いても既存の2秒の期限内で受信を待ちます。分割受信をループバック回帰テストで確認します。
- The failure-panel test uses a deterministic simulated failure. The dependency update command refreshes the notice table and source identity after checking the license inventory. / 障害パネルのテストには再現可能な模擬障害を使います。依存更新コマンドはライセンス一覧の照合後に通知一覧とソース ID を更新します。
- The changelog, documentation index, and vulnerability reporting policy have been updated. / 変更履歴、文書索引、脆弱性の報告手順を整備しました。

## Distribution and validation / 配布と検証

Windows x64 Setup and portable ZIP are unsigned. The source ZIP and SHA-256 checksums accompany the release. The USB library stays at nusb 0.2.7.

Windows x64 の Setup と portable ZIP は未署名です。ソース ZIP と SHA-256 チェックサムも配布します。USB ライブラリは nusb 0.2.7 を使用します。

The [physical GR300 validation record](validation/physical-gr300-0.1.21.md) applies to the 0.1.21 portable build. Validation for 0.1.22 uses CI with Simulation and synthetic frames.

[GR300 の実機検証記録](validation/physical-gr300-0.1.21.md)の対象は 0.1.21 の portable 版です。0.1.22 は CI の Simulation と合成フレームで検証します。
