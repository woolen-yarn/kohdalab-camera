# KohdaLab Camera 0.1.16

## Changes

The common Python control client now ignores empty native home variables in favor of a valid platform fallback. Its response reader checks only newly received bytes for the JSON-lines delimiter, preserving the existing response limit and error contract.

The canonical configuration is `~/.kohdalab/camera/config/config.json` (Windows: `%USERPROFILE%\.kohdalab\camera\config\config.json`). Runtime API descriptors use `camera/run`; default measurements and application logs use sibling `results` and `logs` where applicable. Camera exports continue to honor the user-selected folder. The shared path definition is vendored identically in all five applications.

## Compatibility and safety

A missing default is created atomically on the first normal GUI launch. Existing settings, including invalid/foreign/unsupported files, are preserved with visible errors. Old direct-child, shared, preview and AppData profiles are not imported or removed automatically; use Browse/Load after reviewing an existing profile. Explicit profile and output paths remain respected. No device connection, source output, capture or shutter movement is added. Scientific file formats and control ownership/cleanup policies are unchanged.

Camera's Windows build now runs when this bundled common client changes. Rust source formatting is standardized in a separate mechanical commit; acquisition, rendering, device policy and image export algorithms are unchanged. The derived source identity was regenerated from the exact release inputs.

The bundled common SDK remains 1.0.1 and wire protocol stays at version 1. Settings, output folders, capture formats, control ownership and cleanup policies are unchanged. Existing cross-process recovery interlocks and OS driver caches retain their safety/OS-managed namespaces rather than being mistaken for GUI configuration.

## Downloads and validation

Windows x64 Setup/portable ZIP and corresponding source kit; no Mac Camera distribution. Windows artifacts remain unsigned; supported Mac applications are ad hoc signed and not notarized. Normal GUI startup is tested in isolated user homes, including restart/invalid-file preservation and canonical config/run paths. Platform tests, installer evidence and artifact/source hashes qualify the attached builds; no new physical-device qualification is claimed.

## 日本語

共通Pythonクライアントでは、空のネイティブホーム環境変数を有効なOS側フォールバックで補います。API応答では新しく受信した断片だけからJSON Linesの区切りを検出し、既存のサイズ制限とエラー契約を維持します。

共通クライアントを変更したときもCameraのWindowsビルドを実行します。Rustソースは機械的に標準整形しましたが、撮像・描画・デバイス方針・画像保存アルゴリズムは変更しません。ソース識別子は実際の公開入力から再生成しました。共通SDKは1.0.1、通信プロトコルはv1のままです。設定・保存先・画像形式・制御権・終了処理は変更しません。
