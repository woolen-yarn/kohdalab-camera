# KohdaLab Camera 0.1.19

## Maintenance and compatibility / 保守性と互換性

This corrective release aligns every Camera workspace member with the application's existing Rust 1.92 minimum. Ordinary Cargo and Dependabot updates can now select supported current dependencies rather than being constrained by the former lower member declarations. Capture, image/RAW export, sidecar schemas, device policy and the opt-in observer's mutation restrictions are unchanged.

この修正版では、全Cameraワークスペースクレートをアプリ既存のRust 1.92最小要件に揃えます。通常のCargo／Dependabot更新が旧来の低いクレート宣言に制限されず、対応する新しい依存を選べるようにします。撮影、画像／RAW出力、付属JSON形式、機器方針、任意observerによる変更操作の禁止は変わりません。

Simulation and device-free recording/API checks cover the packaged Windows app; macOS CI covers Simulation only. No new physical-camera fault, power-loss, or stopping test was performed for this release. Windows x64 Setup/portable/source are provided with checksums and remain unsigned. Camera does not publish a macOS acquisition package.

Simulationと機器不要の記録／API試験でWindows配布版を確認し、macOS CIはSimulationのみを確認しています。本リリースでは新たな実機カメラの異常・電源断・停止試験を実施していません。Windows x64 Setup／portable／sourceをチェックサム付きで提供し、引き続き未署名です。CameraのmacOS実機取得版は公開しません。

## Verification and distribution / 検証と配布

Windows CI now exercises image/raw export and frame-provenance tests alongside existing installer checks. A macOS job verifies the same device-free recording tests and native Simulation observer. Windows x64 Setup/portable/source remain the supported distributions; no macOS physical-camera package is added. This change does not constitute new hardware qualification. Windows packages remain unsigned.

Windows CIで画像・raw保存とフレーム情報のテストを、既存のインストーラー検証と合わせて実行します。macOSでも同じ機器不要の保存テストとネイティブSimulationのAPIを確認します。配布は引き続きWindows x64 Setup／portable／sourceで、Mac向け実機カメラ版は追加しません。新たな実機検証ではなく、Windows版は未署名です。
