# KohdaLab Camera 0.1.21

## Maintenance and compatibility / 保守性と互換性

This is a packaging and verification release. The bundled third-party notices and license texts now match the locked dependencies exactly. Packages through 0.1.20 omitted the notice rows and license texts for some linked crates, including `csv`, `sha2` and `uuid`; 0.1.21 corrects this. Published 0.1.20 and earlier assets are left unchanged.

配布物と検証を整えるリリースです。同梱の第三者通知とライセンス文を、固定した依存と完全に一致させました。0.1.20 までの配布物は、`csv`・`sha2`・`uuid` など一部のcrateの通知行とライセンス文を欠いていました。0.1.21 で修正します。公開済みの 0.1.20 以前のassetは変更しません。

Application source is unchanged from 0.1.20 apart from the version number. Capture behavior, device policy, image/RAW exports, output bundles, settings and the read-only GUI API are unchanged. The USB library remains nusb 0.2.7, the version shipped since 0.1.13.

アプリ本体のソースは、版番号を除いて 0.1.20 と同じです。撮影動作、機器方針、画像・RAW出力、保存セット、設定、読み取り専用のGUI APIは変わりません。USBライブラリは 0.1.13 以降と同じ nusb 0.2.7 のままです。

## Verification and distribution / 検証と配布

CI now runs every Rust workspace test on Windows and macOS instead of a named subset, including the Windows-only helper and GUI tests that previously ran nowhere. A new check compares the bundled license directories and notice table with `cargo metadata` and fails on any missing, extra or mismatched entry. Rust 1.99.1 is pinned; formatting, Clippy output and the Rust 1.92 minimum are checked.

CIは名前で選んだ一部ではなく、WindowsとmacOSで全workspaceテストを実行します。これまでどこでも実行されていなかったWindows専用のhelper・GUIテストも含みます。同梱ライセンスのディレクトリと通知の表を `cargo metadata` と照合し、不足・余剰・不一致があれば失敗する検査を追加しました。Rust 1.99.1を固定し、整形・Clippy・最低版1.92を確認します。

All checks use Simulation and synthetic frames. No physical-camera test was performed on the 0.1.21 binaries. Windows x64 Setup/portable/source are provided with checksums and remain unsigned. Camera does not publish a macOS acquisition package.

検査はすべてSimulationと合成フレームによるものです。0.1.21 のバイナリでは実機カメラの試験を行っていません。Windows x64 Setup／portable／sourceをチェックサム付きで提供し、引き続き未署名です。CameraのmacOS実機取得版は公開しません。
