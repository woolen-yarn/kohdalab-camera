## 現行実装への反映

Measureのカメラ設定はDirectShowドライバーに委譲されていました。現在は高速クロック8000、32本の512KiB待機転送を採用。高い露光・ゲインではUSB停止が再現するため、128行/1倍を安定設定とし、画像側の明るさ補正と失敗時の設定復帰を使います。最新の実機結果は[validation.md](validation.md)を参照してください。以下は解析経緯です。

# MeasureS 静的解析 — 2026-10-06

メーカー配布ページのMeasureを取得し、Windowsコードを実行・登録せず展開した。

- 配布: https://www.shodensha-inc.co.jp/ja/download-software/gr300bcm2/
- ZIP: https://www.shodensha-inc.co.jp/ja/wp-content/uploads/sites/2/MeasureS_200625.zip
- SHA256: a2aa07601d9b88081f84e201e057380b8b74ac29f45e0ddf4252550e381bcfd4
- ZIPにはsetup.exeと日本語・英語の説明書Ver1.08が入っている。
- InstallShieldの圧縮されたoverlayを静的に復元し、MeasureS.msi → Data1.cabを7-Zipで展開。
- 展開形式は公開ISx/Unpackerソースと照合した。ベンダーのsetup.exeは実行していない。
  参照: https://github.com/lifenjoiner/ISx / https://github.com/pawstas80/Unpacker
- 一時解析ファイル: /private/tmp/kohda-measure-analysis/

## 設定の実体

payloadはmeasures.exe、bcgcbpro1100u.dll、setting.ini、5個の.rlrファイル。
INIのVideoはSelectDevice=1、Device=Integrated Camera、Width=640、Height=480。
これは配布時の汎用設定であり、接続中のカメラの設定ではない。露光・ゲイン設定はこのINIにはない。

measures.exeは32bitネイティブPE。CLSID_FilterGraph、IAMStreamConfig、ISpecifyPropertyPagesのGUIDを含む。
IAMCameraControl / IAMVideoProcAmpのGUIDは検索範囲では見つからず、直接USB制御のDeviceIoControl importもない。

0x41970A付近の処理はISpecifyPropertyPagesをQueryInterfaceし、GetPagesを呼んで、OLEPRO32 ordinal250経由でプロパティ画面を開く。
したがって、カメラのプロパティ設定はDirectShowフィルター側に委譲する構造と判断できる。
Measureだけに機種固有のUSB設定表が入っているという構造ではない。

## GR300W.ax側で追跡できた設定

| 内容 | 静的解析の根拠 | 結果 |
| --- | --- | --- |
| 露光初期値 | 0x100021F0: object+580=0x12C | 300行。センサー初期化の1561行の後に上書きされる経路あり |
| 露光適用 | 0x1000391C付近 | object+580をsensor register09へ書く |
| 速度 | 0x100038C9 / 0x10003E50 | bool切替でregister0A=8001または8000、register05=0150 |
| 保存キー | 0x100040C5 / 0x1000410D / 0x10004259 | AutoExposure / ExposureTime / Speedをレジストリへ保存 |
| ゲイン変換 | 0x100067A0 | UI値をsensor register35のアナログ・デジタルゲイン符号へ変換 |

ゲイン変換を復元すると、入力g（画面の数値）に対し:

- g < 25: register35 = g + 8
- 25 ≤ g < 41: register35 = g + 56
- 41 ≤ g < 86: register35 = ((g - 41) << 8) + 0x160
- g ≥ 86: register35 = 0x2D60

画面のゲイン値とレジスタの生コードは一致しない。Macアプリは現状8–32の生コード（1–4倍のアナログ領域）に限定する。
露光のms換算には機器のCLK_IN実周波数が必要で、まだ行数のまま扱う。

## Macで得られた進展と限界

Micron MT9T001のメーカー資料でchip ID1621、露光register09、ゲインregister35、クロック分周register0Aを照合。
資料: https://datasheet.octopart.com/MT9T001P12STC-Micron-datasheet-136597.pdf

低速クロック8008、露光32行、ゲイン8で白飛びを解消し、被写体の輪郭・模様を確認。
露光128行では平均画素値約63。これは低速実験設定で、Windowsの標準速度設定の再現ではない。
4本の512KiB非同期転送のプロトタイプでは12フレーム相当をUSBエラーなしで取得。
GUI統合版のフレーム境界処理には欠落・タイムアウトが出たため修正と再検証を継続。
色順序・ホワイトバランス・長時間の安定性は未確認。


## GUI統合後の実機検証

フレームの長さは固定3,145,728byte。短いUSB転送で開始同期し、その後は固定長で切り出す。毎フレームの終わりにZLPが届くとは限らず、ZLP待ち必須の実装は欠落・タイムアウトを生じたため修正。

センサー内蔵テスト（register32=0400、register07=0042）を3枚取得。すべて全画素が64と191に均等に分かれ、各行の偶数列64・奇数列191となった。データの上位8bit取得と水平先頭位置を確認。テスト終了後は通常のセンサー初期化で元に戻した。

GUIで10枚表示、露光128→256→64行、ゲイン8を試験。平均画素値約60→113→33で、露光による光学的な明るさの変化を確認。元データTIFFと表示PNG＋JSONを保存し、正常停止。転送中の直接設定変更ではUSBエラーが出たため、設定は読み取りをキャンセルして再初期化・再開する方式に変更。設定後は数秒の待ち時間がある。

短時間のGUI試験成功であり、長時間運転・Windows 11・Bayer色順序・露光ms校正はまだ未検証。ユニットテスト38件成功。


### 再現性についての追記

10枚と露光変更・保存・停止が成功した試験の後、GUIの再起動・設定再開を再試験するとUSB async status1が再発した。ソフトウェアUSBリセット後も3枚で停止する試行があった。成功した短時間試験を、安定した繰り返し起動・長時間ライブ動作の証明として扱わない。エラー時は正常にワーカーを停止する。最終GUI試験の追加保存待ちまでを安定して完了させる検証は未達。


## 静的展開・解析の再現

配布ZIPのsetup.exeを任意の作業フォルダーへ取り出し、以下を実行する。展開スクリプトは今回確認したsetup.exeのSHA256と一致する場合だけ動作し、EXEを実行しない。

```sh
uv run python scripts/extract_measure_setup.py /path/to/setup.exe /path/to/installer-output
# 7-ZipでMeasureS.msiを展開し、その中のData1.cabをさらに展開する。
uv run --with pefile --with capstone python scripts/analyze_driver.py /path/to/measures.exe /path/to/disassembly --applications
```

MeasureS.msiのSHA256: 55bd2632fc39b4599267daf16e6ce9aa2b35847480c1a7c40bf62cd2e2b1fe49。
展開ロジックのライセンスはthird-party-notices.md参照。

最終的にはSpeedの高速側（8000）を採用。標準/低速の転送停止は複数条件で再発したが、高速側では通常Qtライブ90秒968枚と4接続1,126枚で復帰0回。最新の検証記録はvalidation.mdを参照。
