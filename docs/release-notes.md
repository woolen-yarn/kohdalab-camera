# KohdaLab Camera 0.1.17

## Recording provenance / 保存データの由来

Image sidecars now include a versioned recording contract: application version/source identity, exact received payload size/format, stored component bits, declared sensor/transport bits when available, simulation status and explicit timing limitations. Existing image pixels, RAW bytes and display recipes are unchanged.

画像の付属JSONに、版付きの保存仕様を追加します。アプリ版・ソース識別子、受信データのサイズ・形式・保存ビット数、確認できたセンサー／通信ビット数、Simulationの区別、時刻の制約を記録します。画像の画素値・RAWバイト列・表示補正は変更しません。

The supported GR300 transport provides 8 bits from a 10-bit sensor. RAW preserves the received bytes; it cannot recover missing bits. Exposure duration/clock, sensor exposure-start time and dropped-frame count remain unknown (`null`). Host frame acceptance time/sequence is not relabeled as sensor timing.

対応GR300の通信経路は10-bitセンサーの8-bitを運びます。RAWは受信値を保持しますが、欠けたビットは復元しません。露光時間・実測クロック・センサー露光開始時刻・欠落フレーム数は不明（`null`）です。ホストの受信時刻・連番をセンサー時刻として扱いません。

## Verification and distribution / 検証と配布

Windows CI now exercises image/raw export and frame-provenance tests alongside existing installer checks. A macOS job verifies the same device-free recording tests and native Simulation observer. Windows x64 Setup/portable/source remain the supported distributions; no macOS physical-camera package is added. This change does not constitute new hardware qualification. Windows packages remain unsigned.

Windows CIで画像・raw保存とフレーム情報のテストを、既存のインストーラー検証と合わせて実行します。macOSでも同じ機器不要の保存テストとネイティブSimulationのAPIを確認します。配布は引き続きWindows x64 Setup／portable／sourceで、Mac向け実機カメラ版は追加しません。新たな実機検証ではなく、Windows版は未署名です。
