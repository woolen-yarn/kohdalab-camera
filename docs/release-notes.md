# KohdaLab Camera 0.1.18

## Maintenance and compatibility / 保守性と互換性

This release updates the desktop GUI foundation and removes the unmaintained font parser. Camera polling and the opt-in, read-only GUI observer continue independently of visible painting. Capture, image/RAW export, sidecar schemas, device policy and the observer's mutation restrictions are unchanged.

本リリースではデスクトップGUI基盤を更新し、保守終了したフォント解析を除去します。Cameraのポーリングと、任意で有効化する読み取り専用GUI observerは、描画の表示状態と独立して継続します。撮影、画像／RAW出力、付属JSON形式、機器方針、observerによる変更操作の禁止は変わりません。

Simulation and device-free recording/API checks cover the packaged Windows app; macOS CI covers Simulation only. No new physical-camera fault, power-loss, or stopping test was performed for this release. Windows x64 Setup/portable/source are provided with checksums and remain unsigned. Camera does not publish a macOS acquisition package.

Simulationと機器不要の記録／API試験でWindows配布版を確認し、macOS CIはSimulationのみを確認しています。本リリースでは新たな実機カメラの異常・電源断・停止試験を実施していません。Windows x64 Setup／portable／sourceをチェックサム付きで提供し、引き続き未署名です。CameraのmacOS実機取得版は公開しません。

## Verification and distribution / 検証と配布

Windows CI now exercises image/raw export and frame-provenance tests alongside existing installer checks. A macOS job verifies the same device-free recording tests and native Simulation observer. Windows x64 Setup/portable/source remain the supported distributions; no macOS physical-camera package is added. This change does not constitute new hardware qualification. Windows packages remain unsigned.

Windows CIで画像・raw保存とフレーム情報のテストを、既存のインストーラー検証と合わせて実行します。macOSでも同じ機器不要の保存テストとネイティブSimulationのAPIを確認します。配布は引き続きWindows x64 Setup／portable／sourceで、Mac向け実機カメラ版は追加しません。新たな実機検証ではなく、Windows版は未署名です。
