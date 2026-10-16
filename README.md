# KohdaLab Camera

独立して実装したRust製のカメラアプリです。このリポジトリは、今後の改修に使う最新版の公式開発ベースです。Windows x64版を提供します。

## ダウンロード

[最新版のWindows版](https://github.com/woolen-yarn/kohdalab-camera/releases/latest)を展開し、`camera-lab.exe`を起動してください。`continuous-helper.exe`とライセンス類を同じフォルダに保ってください。既存のWinUSB接続環境が必要です。ドライバを自動変更しません。

## 使い方

1. USBカメラを選択し、**Connect and open capture window**で接続します。実カメラなしで確認する場合はSimulation / demoを明示的に選びます。
2. Camera / Acquisition / Image / Export / Advancedの各タブで操作します。F1〜F5でも切り替えられます。
3. Imageの明るさは正の倍率で、初期値は**1.20倍**です。画素ごとのGRBG補間と線形表示、ホワイトバランス、ガンマ、回転、反転、モノクロを使えます。これらは表示用の処理です。
4. Exportの**Capture image…**は表示中の画像をPNG/TIFFで保存します。元のBayer TIFF、RAW、メタデータも別に保存します。既存名の上書きは拒否します。
5. **Capture full sensor image…**は2048×1536の新しい画像を取得し、元の適用済みLive設定へ戻ります。通常のLiveは1024×768です。
6. Disconnectまたはウィンドウの×で通常終了します。解放が証明できなかった場合は保護ロックを保持します。ロックの自動削除やUSBリセットで再試行しません。

Windows App経由の今回の操作では、一部のマウス入力が反映されず、F1〜F5、Tab、Enterで操作できました。**通常のマウス操作の確認・改善は継続中です。**

## 確認範囲

同じ機能コードを持つ公開直前の実機確認で、1024×768 Live、PNG保存、2048×1536 Full撮影、元のLiveへの復帰、親子プロセスの正常終了と保護ロック不在を確認しました。両サイズの保存PNGは元RAWからの独立計算と全画素一致し、元のBayer TIFFもRAWと完全一致しました。

取得FPSとプレビューFPSは別表示です。観測値は約39.5でしたが、条件依存であり性能保証や光学的FPSの校正値ではありません。時刻は**ホストが完全フレームを受け付けたUTC取得完了時刻**で、センサー露光時刻ではありません。

色/CFA/光学的視野の校正、ウィンドウの任意サイズ保持、通常のマウス操作は未完了です。最新版の長時間運転確認は後日行います。全体完成・旧版に対する全条件の優位は主張しません。公開用の名称・記録先整理後の実行物は、公開前候補と別ハッシュです。

## ビルド

Rust stable（edition 2024対応）を使用します。依存バージョンはCargo.lockに固定されています。

```sh
cargo build --locked --release --package camera-lab --bin camera-lab
cargo build --locked --release --package continuous-helper-entry --bin continuous-helper
```

Windows GNUターゲットでのクロスビルドには`x86_64-pc-windows-gnu`標準ライブラリとMinGW-w64のgcc/windresが必要です。

```sh
cargo build --locked --release --target x86_64-pc-windows-gnu --package camera-lab --bin camera-lab
cargo build --locked --release --target x86_64-pc-windows-gnu --package continuous-helper-entry --bin continuous-helper
```

テストは`cargo test --locked --lib --bin camera-lab`で実行できます。実カメラの操作は単体テストには含めません。macOSの実行物はこのReleaseでは配布しません。

## ライセンスと出所

プロジェクトコードはMITです。[LICENSE](LICENSE)、[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)、[出所と範囲](docs/provenance.md)を参照してください。依存ライセンス本文は`third-party-licenses/`にあります。メーカー製ソフト、旧アプリのソース・バイナリ・素材は含みません。
