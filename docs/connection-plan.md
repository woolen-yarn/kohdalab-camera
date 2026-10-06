# 接続方式と実装方針

調査日: 2026-10-06。対象のSHODENSHA機種名・OEM元・USB記述子・付属CDは未確認。

## 現時点で確定できること

Tucsen公式の旧CMOS一覧は **TCA-3.0C = VID 0547 / PID 4D33**、専用ドライバ、Windows用TSView6/7を掲載。
これはTCA機種の識別候補であって、SHODENSHA実機が同じ機種・同じプロトコルという証明ではありません。
専用ドライバの存在から旧式独自USBを有力候補と推定します。UVCの有無は記述子とOS動画デバイスを確認します。
[旧機種の公式一覧](https://www.tucsen.com/Home/Product/info/dataid/35.html)

現行TucsenサイトにはMac用Mosaic、Windows/Linux SDKと旧CMOSリンクがあります。
**Mac用ソフトの掲載はTCA-3.0C対応、macOS用SDK提供、Apple Silicon対応を意味しません。**
現行SDKと旧TSView/TCA SDKを同じものとして扱いません。
[公式ダウンロード](https://www.tucsen.com/download-software/)
[公式SDK説明](https://www.tucsen.net/sdk-software-support/)

SHODENSHAのGR300BCM3資料はドライバ導入を案内していますが、対象の「3.1M USB2.0 Camera」と同一型番か不明。
この資料から対象実機の方式・Windows 11対応を断定しません。
[メーカーのGR300BCM3セットアップ](https://www.shodensha-inc.co.jp/microscope/gr300bcm3_20180219.pdf)

## 実機判定の順序

1. ラベルの完全な型番、付属CD/ソフト名、接続前後のUSB一覧を記録。
2. VID/PID、USBシリアル、device/interface記述子、結び付いたドライバ名を記録。
3. UVCなら通常interface class `0x0E`、VideoControl subclass `0x01`、VideoStreaming subclass `0x02`がある。
   device class `0x00`/`0xEF`でも複合機器のinterfaceにVideoクラスがあり得るのでdevice classだけで判定しない。
4. class `0xFF`のみなら独自USBが候補。ブート/ファームウェアロード前後にPIDや記述子が変わる可能性も考慮する。
5. OS動画デバイスとして開き、対象カメラからのフレームを確認。
   メーカーのDirectShowブリッジでもOS動画APIに出るので「OpenCVで開けた = USB UVC」とは限らない。
6. `0547:4D33`ならメーカーにTCA-3.0C/OEM対応と旧SDKの入手を問い合わせる。

### Windows 11

デバイスマネージャー→対象機器→プロパティ→詳細→ハードウェアID:
`USB\VID_0547&PID_4D33` の形式を確認。ドライバタブで提供元・版・署名、詳細でserviceを記録。
USBViewでConfiguration/Interface/Endpoint記述子も保存。
[Microsoft USBView](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/usbview)

初期実装の `diagnostics.py` は次のPowerShellをPython subprocessから呼び、VID/PIDを抽出します。
これはUSB/PnP列挙であり、動画API内のカメラ一覧や記述子の完全な取得ではありません。

```powershell
Get-CimInstance Win32_PnPEntity |
  Where-Object { $_.PNPDeviceID -match 'VID_[0-9A-F]{4}&PID_' } |
  Select-Object Name,PNPDeviceID,Service,Status,ConfigManagerErrorCode |
  ConvertTo-Json -Depth 4
```

UVCはWindows標準Usbvideo.sysを利用し、新しいカーネルドライバは不要。
MSMF、必要に応じDirectShowをPython側の入口にします。
[Microsoft UVCドライバ](https://learn.microsoft.com/en-us/windows-hardware/drivers/stream/usb-video-class-driver-overview)

独自USBは正確な型番用のメーカー署名済みドライバ＋SDKを優先。
旧ドライバのWindows 11 x64・署名・メモリ整合性対応は未確認。対応版をメーカーから確認する。
古いSDKが32bit DLLのみなら64bit Pythonに直接ロードできないため、32bitのヘルパープロセスとIPCが必要。
Windows ARM64対応も未確認。
[ドライバ署名](https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/signing-a-driver)
[メモリ整合性互換性](https://learn.microsoft.com/en-us/windows-hardware/test/hlk/testref/driver-compatibility-with-device-guard)

### macOS

システム情報→USBで名前・製造元・製品ID・製造元IDを確認。

```sh
system_profiler SPUSBDataType -json
# より詳しいIORegistry情報（確認用。キーはOS/接続状態で異なります）
ioreg -p IOUSB -l -w 0
```

PythonコードはIORegistryの機器/インターフェース情報を優先し、同一VID/PIDとlocationIDで対応付けます。
system_profilerは代替として使用します。2026-10-06の実機確認では、system_profilerは空でもIORegistryは接続機器を表示しました。
任意のPyUSB診断はconfiguration/interfaceを読み、設定変更やdriver detachは行いません。
権限やlibusbバックエンドが不足していた場合は「unknown/検査不可」として残します。

OSに認識される動画機器はAVFoundation経由で試験。`uv run --extra mac kohdalab-camera doctor --list-video`でPyObjC経由の名前・uniqueID・modelIDを列挙します。
現実装のOpenCV番号走査は一時番号で、AVFoundation uniqueIDとVID/PIDの対応付けは未実装です。
[Apple capture device selection](https://developer.apple.com/documentation/avfoundation/choosing-a-capture-device)
[OpenCV OSバックエンド・プロパティ](https://docs.opencv.org/4.x/d4/d15/group__videoio__flags__base.html)

独自USBなら正確な旧機種のmacOS用SDKと対応CPU/OSが提供されているか確認。
Windows DLLやWindowsドライバはmacOSで直接利用できません。Intel専用Mac SDKがある場合もABI/CPUの制約を確認。
なければWindows機で撮影し、LAN越しにMacへ転送する構成が実用候補。
新しいUSB実装はファームウェア、コマンド、転送形式、フレーム境界、Bayer形式、ライセンス情報を得てから評価する。
libusbを導入するだけでは画像取得プロトコルは実装されません。

## 参考repoと設計

確認時の `woolen-yarn/kohdalab-iv` main: `757a4d98317dfe0d9f2b93e29f2e58bbe72f427d`。
`src/kohdalab_iv/api`、`instruments`、`interfaces`、`apps`、`resources`、`tests`を持ち、
CLI/GUIの共通コア、模擬機器、任意GUI依存、撮影条件に相当する来歴記録の考え方を参考にしました。
カメラ用の新規実装で、I-V計測用ドライバやコードの流用はしていません。
[参考repo](https://github.com/woolen-yarn/kohdalab-iv)

共通操作 `open/read/set_control/close` と所有済みRGB Frameを用意。
現段階ではsimulatedとOpenCVが動作する入口、legacy-tcaは未実装を明示して失敗する統合境界。
今後のSDKバックエンドは列挙/接続/設定範囲取得/開始/タイムアウト付きフレーム取得/停止/解放を提供します。
SDKのヘッダー、calling convention、struct packing、CPU、エラーコードを確認してctypes/cffiまたは専用helperを選びます。
SDKのバッファを解放する前に画像をコピーし、露光ms・ゲインdB/倍率・auto可否・範囲・刻みを能力情報として返します。
旧SDKの関数名やABIを推測したラッパーは作りません。

GUI案は上部の接続方式/API/番号/開始停止、中央のライブビュー、下部の露光・ゲイン・自動露光と保存先。
初期実装はこの構成を提供し、実機で拒否された設定を表示します。
次段階では正式なデバイス選択、解像度/FPS選択、能力に基づくコントロール無効化、ヒストグラム、ROI、
SDKからの16bit/Bayer保存、連続保存/動画、設定プロファイル、別プロセス化を順に追加します。
UVCの帯域やSDK仕様が不明なので、3MP時のFPSは保証しません。

## メーカーに確認する内容

- SHODENSHA完全型番、VID/PIDと記述子を提示してTCA-3.0C由来か確認。
- Windows 11 x64で使用可能な署名済みドライバ、旧TCA SDKのヘッダー/DLL/サンプル/配布条件。
- 旧TCA SDKと現行TUCAM SDK、TSView、Mosaic、DirectShowとの対応表。
- macOSの対応有無、Apple Silicon/Intel、対象OS、SDKライブラリの提供可否。
- 露光・ゲイン・最大FPS・ピクセル形式とファームウェアロードの必要性。

実機での判定と次の実装条件は[Mac実機記録](mac-hardware.md)を参照。
