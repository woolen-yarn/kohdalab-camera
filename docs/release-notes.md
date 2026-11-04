# KohdaLab Camera 0.1.16

## Changes

The bundled common Python control client now ignores an empty native home-directory environment variable and uses a valid platform fallback. Its JSON Lines response reader scans only newly received bytes, while retaining the established response-size limit and error behavior.

Camera's Windows build is now exercised when the bundled control client changes. Rust sources were standardized by a mechanical formatting-only change, and `RECORDER-SOURCE-ID` was regenerated from this release source.

## Compatibility and safety

The common SDK remains 1.0.1 and the control wire protocol remains version 1. Camera acquisition, rendering, device policy, export formats, selected output folders, control ownership, and cleanup behavior are unchanged. Existing settings and operator-selected paths remain preserved; this release does not migrate or delete profiles.

## Downloads and validation

This release provides Windows x64 Setup, portable ZIP, and source artifacts. Windows artifacts are unsigned; no macOS Camera distribution is provided. CI checks the packaged software, installer evidence, and artifact/source hashes. It does not constitute new physical-camera qualification.

## 日本語

同梱の共通Python制御クライアントは、OS固有のホームディレクトリ環境変数が空の場合に有効なプラットフォーム側フォールバックを使用します。JSON Lines応答は新しく受信したバイト列だけを走査し、既存の応答サイズ上限とエラー動作を維持します。

同梱制御クライアントを変更した場合にもCameraのWindowsビルドを実行します。Rustソースは機械的な整形のみを行い、`RECORDER-SOURCE-ID`はこのリリースのソースから再生成しました。

## 互換性と安全性

共通SDKは1.0.1、制御通信プロトコルはv1のままです。撮像、描画、デバイス方針、画像形式、選択済み保存先、制御権、終了処理は変更しません。既存設定と操作者が指定したパスは保持し、プロファイルの移行・削除は行いません。

## 配布と検証

Windows x64 Setup、portable ZIP、sourceを配布します。Windows成果物は未署名で、macOS版Cameraは提供しません。CIではパッケージソフトウェア、installer証跡、成果物とソースのハッシュを検証しますが、新たな実機カメラ認定を意味しません。
