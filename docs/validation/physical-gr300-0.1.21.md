# GR300 physical validation for 0.1.21 / 0.1.21 の GR300 実機検証

On 2026-11-12 JST, the published Windows x64 portable build of KohdaLab Camera
0.1.21 was exercised on Windows build 26300 with a SHODENSHA GR300 laboratory
unit (Micron MT9T001, USB `0547:4d33`, existing WinUSB binding). The GUI ran in
the workstation's interactive Remote Desktop session. The portable ZIP's SHA-256
matched the published digest
`0d0bd1b229a2a314a3a4758968740c2be5b0b54c11fe1a167c2cc0d6896715ea`.
See the [published release](https://github.com/woolen-yarn/kohdalab-camera/releases/tag/v0.1.21).

| Check / 確認項目 | Observed result / 確認結果 |
| --- | --- |
| Live | A 1024 × 768 physical preview updated. / 1024 × 768 の実機映像が更新されました。 |
| Normal capture / 通常保存 | The saved PNG decoded at 1024 × 768. / 保存した PNG を開き、1024 × 768 を確認しました。 |
| Full-sensor capture / フルセンサー保存 | The saved PNG decoded at 2048 × 1536. The GUI reported **Full image saved · Live restored** and the 1024 × 768 preview resumed. / 保存した PNG を開き、2048 × 1536 を確認しました。GUI に **Full image saved · Live restored** が表示され、1024 × 768 の映像更新が再開しました。 |
| Export evidence / 保存セット | Both exports included Bayer TIFF, RAW, JSON metadata, frame CSV, RAW index CSV, a lifecycle log and conditions JSON. Both CSVs recorded `simulated=false`, each RAW SHA-256 matched its CSV, and both conditions manifests reported `complete`. / 両方の保存セットに Bayer TIFF、RAW、JSON メタデータ、フレーム CSV、RAW 索引 CSV、ライフサイクルログ、条件 JSON が揃いました。両 CSV の `simulated=false`、実ファイルと CSV の RAW SHA-256 の一致、両条件 JSON の `complete` を確認しました。 |
| Disconnect, reconnect and close / 切断・再接続・終了 | The GUI showed **Disconnected**, a subsequent reconnect returned to Live, and no GUI or acquisition helper process remained after normal close. / GUI に **Disconnected** が表示され、再接続で Live に戻りました。通常終了後に GUI と取得ヘルパーのプロセスが残っていないことを確認しました。 |

The saved frames have host acceptance timestamps of 11:05:41.716 and
11:06:25.283 JST. Both captures used the existing exposure of 360 sensor rows
and gain of 2.25×. This check did not cover exposure or gain changes, sustained
operation, fault recovery, or optical calibration.

2026-11-12 JST に、Windows build 26300 の対話型リモートデスクトップで、
SHA-256 を照合した公開版 KohdaLab Camera 0.1.21 の Windows x64 portable 版を
操作しました。使用機器は既存の WinUSB バインディングを使用する SHODENSHA GR300
（Micron MT9T001、USB `0547:4d33`）です。

通常・フルセンサー保存のホスト受理時刻はそれぞれ 11:05:41.716 と
11:06:25.283 JST です。露光は既存値の360センサー行、ゲインは2.25×で確認しました。
露光・ゲインの変更、長時間運転、異常時の復旧、光学校正は今回の検証範囲外です。
