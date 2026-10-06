# GR300BCM2 Windowsドライバの初期静的解析

2026-10-06。メーカー配布ファイルを取得し、実行・登録・インストールせずにZIP、INF、PEヘッダー、import/exportを確認。

- [配布ページ](https://www.shodensha-inc.co.jp/ja/download-software/gr300bcm2/)
- [解析対象ZIP](https://www.shodensha-inc.co.jp/ja/wp-content/uploads/sites/2/300driver240426.zip)
- ZIP SHA256: `197aa9c348ee931d6fdef59a1a8c2bbcca692ec4eec44fdfe23f944e2f81d7ae`
- ZIP内WIN10フォルダーを対象とした。ZIP名は2024年だが、INF DriverVerは2011-04-26 / 1.0.0.2。
  配布日や名前からWindows 11対応を判断しない。

## 確認結果

INFは `USB\VID_0547&PID_4D33` をx86/amd64の対応デバイスに指定する。
接続中の `3.1M USB2.0 Camera` と一致するので、通信解析の有力な対象。
INFの製品文字列は `3.0M USB2.0 Camera`、providerはShanghai Gold Room Co., Ltd.。
VID/PID一致は撮影成功や全OEM仕様の一致を保証しない。

| ファイル | 構成・役割 |
| --- | --- |
| GR300W.sys | PE x86、専用カーネルUSBドライバ |
| GR300W64.sys | PE AMD64、専用カーネルUSBドライバ |
| GR300W.ax | PE x86、COM登録とDirectShowフィルター |
| GR300W.ds | PE x86、TWAINのDS_Entryをexport |
| GR300W.inf / .cat | インストール設定 / カタログ。署名の有効性は未検証 |

.ax/.dsはCreateFile、ReadFile、DeviceIoControlをimportし、.sysはUSBDの構成処理をimportする。
独自USBドライバを介して上位モジュールが撮影を制御する構成と整合する。
SDKヘッダー、APIサンプル、Mac用ライブラリはこのZIPにはない。
Windows上でも64bitカーネルドライバの存在だけでは64bitアプリから32bit .axをロードできない。
必要なら32bit撮影ヘルパーとのIPCを使う。Windows 11動作は未検証。

## Mac実装への見通し

解析に使えるが、WindowsバイナリをMacに読み込む方式ではない。

1. .ax/.dsのDeviceIoControl呼び出し箇所からIOCTL番号、入力構造、初期化/露光/ゲイン/開始停止の対応を整理。
2. .sysのIOCTL処理からUSB control request、bulk/iso転送、endpoint、タイムアウトを追う。
3. 画像サイズ、フレーム境界、ピクセル形式、Bayer配列と補正を特定。
4. Windows実機でUSBPcap/Wiresharkにより既知の撮影操作の通信を記録し、静的解析結果と照合。
5. MacのUSBアクセス条件を解決し、確認できた最小の読取り/撮影シーケンスから実装。

現段階では通信コマンド、画像転送形式、ファームウェア要否を確定していない。
静的解析だけで十分か、Windows通信キャプチャも必要かは今後判断する。
露光・ゲイン・色補正の実装難度は最初の1フレーム取得とは分けて評価する。
手元のMac実行環境ではlibusb列挙が空だったため、USB直接アクセスの可視性も独立した未解決事項。

使用ツール: uv一時環境のpefile、Python zipfile。Windowsコードは実行していない。
取得・展開ファイルは `/private/tmp/kohda-300driver.zip` と `/private/tmp/kohda-driver-analysis/` に保存。

## 2026-10-06: tetra Windows 11 実機診断

SSHによる読取り専用診断。Windows 11 Home build 26200、Intel Core i5-1035G1、AMD64。

- USB VID_0547 / PID_4D33を認識。GR300Wサービス、oem73.inf、2011-04-26 / 1.0.0.2を使用。デバイスはエラー39、ProblemStatus 0xC000007B。
- サービスが参照するGR300W64.sysはAMD64（PE machine 0x8664）、41,672 bytes。Get-AuthenticodeSignatureはValid。Win32_PnPSignedDriverのIsSigned=falseとは判定対象が異なるため、単純な未署名判定をしない。
- GR300W64.sys SHA256: 2C3FE71F2736E4B5AC9D9FA4BD2B1621456EAF59DFACD978F0A8C390D6B3CC1B。
- CodeIntegrityイベント3111が同ファイルを「not compatible with hypervisor enforcement」として拒否。Failure bitmap 0x2、Status 0xC0000220。
- HVCI Enabled=1、SecurityServicesRunningに2。現状の読み込み拒否はメモリ整合性との互換性問題と確認。
- OS保護設定、ドライバー、デバイス構成の変更は行っていない。保護解除後のドライバー読み込み・撮影成功は未検証。

ローカル診断JSONはlocal-diagnostics-windows-driver.jsonとlocal-diagnostics-windows-integrity.json（Git対象外）。

## 2026-10-06: 低fpsの追加解析

配布ZIPを再取得し、上記SHA256と一致することを確認。GR300W.ax / GR300W64.sysをpefileとCapstoneで静的解析した。MeasureSも既知のSHA256で再取得し、既存の抽出器でMSIを復元、olefileでCABストリームを取り出し、macOSのtarで展開した。ベンダーのEXE・ドライバーを実行していない。

- GR300W.ax 0x10006730–0x1000679E: 出力モード別の寸法とReadFile要求長。1024×768は128KiB、2048×1536は512KiB。0x10006800以降の取得関数は4本のOVERLAPPED読取りを発行する。0x80200はバッファ間隔で、要求長そのものではない。
- GR300W64.sys 0x12790–0x12A12: Read IRPをURB_FUNCTION_BULK_OR_INTERRUPT_TRANSFER (9)へ変換。IN時のTransferFlagsは3（方向INとshort-transfer許可）。画像のCPU加工を行う経路ではない。
- MeasureS.msi SHA256は既存記録と一致。measures.exeではDeviceIoControl / ReadFile / CreateFile importなし。CLSID_FilterGraphがファイルoffset0x2B2A8、IAMStreamConfigが0x2B1D8、ISpecifyPropertyPagesが0x2B380にある。DirectShow経由でドライバーへ委譲するという以前の解析と一致。
- メーカーと同じ4×128KiB、WinUSB直接APIのC++受信、1本の大きな受信、ソフトウェアトリガー、行間待機の延長、対象カメラのPnP再起動は欠損解消にならなかった。
- Windows標準ETWをHeadersBusTraceで約10秒記録。261件のbulk成功、31件の終了時キャンセル。途中のUSBエラーや電源移行の記録なし。短いactual_lengthはカーネルURBにも存在し、Pythonのコールバックだけの問題ではない。テスト画像では途中で偶数/奇数列の位相が切り替わる受信もあった。欠損位置の詳細をこの記録だけで確定することはできない。
- MT9T001のメーカー資料はReg06の最小値を3、標準値を25と記載。1023→3のフレーム間待機短縮で完全フレームの速度が改善。既知の64/191交互テスト画像では、3行の20秒試験2回で742枚を取得し、全786,432画素の比較が全枚一致。1023行では20秒78枚。1msタイマー設定なしでも20秒309枚で全枚一致。

参照: [松電舎配布ページ](https://www.shodensha-inc.co.jp/ja/download-software/gr300bcm2/)、[Micron MT9T001資料（15・20ページ）](https://datasheet.octopart.com/MT9T001P12STC-Micron-datasheet-136597.pdf)、[Microsoft WinUSB pipe policies](https://learn.microsoft.com/en-us/windows-hardware/drivers/usbcon/winusb-functions-for-pipe-policy-modification)、[Microsoft USB ETW](https://learn.microsoft.com/en-us/windows-hardware/drivers/usbcon/how-to-capture-a-usb-event-trace)、[libusb RAW_IOの議論](https://github.com/libusb/libusb/issues/490)。RAW_IOで転送切替の遅延を避ける設計は維持している。

静的解析はcaptures/windows-driver-research/、実験のJSON・ETW・ソースはcaptures/windows-fps-diagnostics/。これらのベンダーバイナリ・実験DLLはportableへ再配布しない。

## 2026-10-06: 高速タイミングでのUSBキュー比較

垂直3行では以前の1023行での結果をそのまま採用せず、転送サイズ・本数を再測定した。Windowsの64本8KiB・水平336では20秒604枚を2回、どちらも欠損破棄0・全786,432画素一致。水平21では20秒707枚（35.349fps）・欠損破棄6・全画素一致。128本4KiBは711枚、128本8KiBは705枚で大きな改善にならず、コールバック回数の少ない64本8KiBを採用。小さいキューでも短パケット境界・完全サイズ判定・開始同期・キャンセル完了待ちは維持した。

128/256KiBや32KiBでは、規定の長さでもテスト画像と一致しない画像が一部あり、採用していない。8KiBで改善したことはWinUSBの転送スケジューリングに依存するという推測を支持するが、欠損発生箇所の物理的な特定には至っていない。記録はcaptures/fps-optimization/fps-fast-queues*/。

プレビューを33msから16msのPreciseTimerへ変更し、実更新回数を別計測する。[Qt QTimer仕様](https://doc.qt.io/qt-6/qtimer.html)に従い、遅延時に処理を積み上げず最新画像だけ描画する。USB取得・GUI更新・RustDeskの転送fpsは別の値である。
