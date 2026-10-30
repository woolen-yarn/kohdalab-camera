# KohdaLab Camera 0.1.14

## Changes

On first normal GUI launch, a missing `~/.kohdalab/camera/config.json` (`%USERPROFILE%\.kohdalab\camera\config.json` on Windows) is automatically created from safe built-in defaults. No manual JSON preparation is needed.

## Compatibility and safety

Initialization publishes a complete file atomically without replacing an existing profile. Existing invalid, foreign or unsupported profiles remain intact and errors are reported. The continuous fake GUI and input-diagnostics launches do not initialize production defaults. Creating/loading settings does not connect hardware, enable outputs, acquire images or move shutters. Existing acquisition, data formats, native/common APIs and cleanup policies are unchanged.

## Downloads and validation

This release provides Windows x64 Setup, portable ZIP and corresponding source/build kit. No macOS Camera distribution is provided. Platform tests and artifact provenance qualify the exact packaged source; first-launch regression checks include concurrent creation, preservation and unusable directories. No new physical-device qualification is claimed. Windows executables remain unsigned.

## 日本語

初回の通常GUI起動時に、アプリ専用の標準設定を自動作成します。既存・不正・未対応の設定は保持し、エラーを表示します。設定作成・読込による装置接続・出力ON・撮影・シャッター移動は行いません。従来の測定・保存形式・API・終了処理は変更しません。
