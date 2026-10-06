## 現行実装（2026-10-06）

USB独自方式0547:4D33・MT9T001 chip1621。`legacy_usb.py` / `usb_stream.py`で直接制御し、メーカーのWindowsバイナリは実行しません。センサー設定0A=8000（メーカー高速クロック）、グローバルゲイン35=8、露光09=128。32本の512KiB非同期bulk-IN82で受信し、開始時の短い転送を捨てて3,145,728byteごとにフレームを切り出します。GRBGカラー並びは赤・青ゲインの実機応答で確認しました。USB転送エラーはキャンセル完了後に最大3回再初期化して再取得し、復帰を記録します。露光変更もキャンセル・ハンドル解放完了後に再接続します。

8008の低速実験設定では、macOSログに`device not responding (0xe00002ed)`が出てライブが停止しました。標準8001で改善しました。USBの根本原因が低速設定だけとは断定せず、待機バッファ増加・明示的キャンセル・有限回復処理も含めて対策しています。

以下は解析中の履歴です。現状の検証は[validation.md](validation.md)を参照してください。

# 独自USB通信の静的解析 — 2026-10-06

対象ZIP・ハッシュは[配布ドライバ解析](windows-driver-analysis.md)参照。
Windowsバイナリは実行していない。以下は逆アセンブルから復元した候補で、実機検証結果は末尾参照。

## 制御転送

GR300W.dsの `0x100011C0` はレジスタ書込み、`0x10001230` はレジスタ読取りに対応する候補。
両方がIOCTL `0x222059` を発行し、GR300W.sysの `0x121C3 → 0x12250 → 0x10836` へ進む。
.sysは10-byte入力構造をUSB vendor/class URBに変換する。

| 入力オフセット | 内容 |
| --- | --- |
| 0 | direction: 0=OUT / 非0=IN |
| 1 | type: 1=class / 2=vendor |
| 2 | recipient: 0=device / 1=interface / 2=endpoint / 3=other |
| 3 | 未使用 |
| 4 | bRequest |
| 5 | 未使用 |
| 6–7 | wValue（little endian） |
| 8–9 | wIndex（little endian） |

入力 `(1,2,0,*,request,*,value,index)` はURBのvendor-device、IN転送に対応し、
USB bmRequestTypeの候補は `0xC0`。
URBのRequest/Value/IndexとTransferFlagsを照合した。
[Microsoft URB定義](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/usb/ns-usb-_urb_control_vendor_or_class_request)

| 操作候補 | bRequest | wValue | wIndex | IN長 | 応答 |
| --- | --- | --- | --- | --- | --- |
| sensor register read（実機で訂正） | 0x0A | 0 | register | 3 | valueの上位byte、下位byte、0x08 |
| sensor register write | 0x0B | value | register | 1 | 0x08 |
| 別の16bit値読取り | 0x11 | 0 | 呼出し引数 | 3 | 上記と同じ形式候補 |

**USB INだから機器状態を変更しないとは限らない。0x0Bは書込み副作用のあるIN要求。**
実装したprobeは0x0A/register0だけで、0x0Bはパケット生成のみ。初期化も送信していない。

## センサー初期化の候補

.ds `0x100012C0`:

1. register 0x07 ← 0x0002
2. register 0x00を読取り、`chip_id & 0xFF00 == 0x1600`を確認。
3. register 0x0A ← 0x8000、0x0D ← 1、100ms待ち、0x0D ← 0、100ms待ち。
4. 書込み列:
   `01=0015, 02=0021, 20=0000, 1E=8040, 4E=0020, 04=07FF, 03=05FF,`
   `2B=0060, 2C=0460, 2D=0060, 2E=0060, 0A=8001, 49=0080, 22=0000,`
   `23=0000, 08=0000, 09=0619`。

これは条件付きで実行されるドライバ処理からの復元であり、実機での有効性や全前処理は未検証。
チップIDの実読取りが済んでいないため、センサー型番は断定していない。
露光msへの変換はピクセルクロック・blanking・row timeの確認が必要。

## サイズ・画像転送

.ds `0x10001AB0` にあるサイズ定義:

| モード | サイズ | フレーム領域byte数 | 1回の読取りサイズ候補 |
| --- | --- | --- | --- |
| 0 | 2048×1536 | 3,145,728 | 0x80000 |
| 1 | 1024×768 | 786,432 | 0x20000 |
| 2 | 640×480 | 307,200 | 0x14000 |

ReadFile経路は4個のoverlapped要求を使用。画素数に一致するbyte数は8bit生画像を示唆するが、
Bayer順序、ヘッダー、フレーム境界、色変換の正確な対応は未確定。
.sysにはbulk/interrupt URBとisochronous URBの双方の処理がある。
**実機endpointの転送種別・アドレスを未取得なので、bulk転送だけのカメラとは決めつけない。**

## 実装と検証

- `legacy_protocol.py`: 推定パケットの生成、応答の長さ/status確認、big endian値の復元。
- `scripts/probe_legacy.py`: 既定はdry run。`--execute`はチップID読取り候補1回のみ。
- `scripts/analyze_driver.py`: uvのpefile/capstoneで再現可能な静的解析。
- 23件のテスト成功。うち8件はパケット形式、応答、範囲に関する回帰テスト。
- 実機probeはlibusbがカメラを列挙できず失敗。USBコマンド送信には到達していない。
- libusbデバッグログはDarwinのdevice plugin作成で `out of resources` を表示。
  カメラ以外のUSB機器も同様だった。環境側のアクセス/プラグイン作成条件の問題が候補だが、原因は未確定。

## 次の実機確認

repoフォルダーで以下を実行する。dry runで送る予定のパケットが確認できる。

```sh
uv run python scripts/probe_legacy.py
uv run --with pyusb --with libusb-package python scripts/probe_legacy.py --execute
```

通常のMacターミナルでも同じ列挙エラーかを比較する必要がある。
読取りが可能になれば、チップID→endpoint記述子→確認済み初期化→1フレーム取得の順で検証する。
ライブビュー用legacy backendの完成・実機画像の保存はまだ未達。

## 実機更新: 前処理後のチップID取得に成功

ユーザーの通常ターミナルと承認済みの直接アクセス実行で、前処理なしの応答 `000007` を再現した。
status 0x07の意味自体は未確定。0x08以外を成功扱いしない。

Windows .dsの接続処理にある vendor OUT `40/01`、wIndex=0x000F、wValue=1→0→1 と、
sensor register 0x07 ← 0x0002 を適用すると、書込みACK `08`、chip読取り `162108` が得られた。
実機chip IDは **0x1621**。終了時にregister 0x07 ← 0、bridge値1→0で停止する。
前処理が必要だったことを確認した。ファームウェアへの書込みはしていない。

実機endpoint記述子も確認:

- interface 0 / alternate 0 / class 0xFF
- endpoint 0x81: interrupt IN / max packet 64
- endpoint 0x82: bulk IN / max packet 512

修正したprobeの再現:

```sh
uv run --with pyusb --with libusb-package python scripts/probe_legacy.py --execute --prepare
```

初期化後の1フレームbulk取得も試したが、空パケットの後にタイムアウトし、画像payloadは0byte。
512KiB/16KiBへの分割を試した。制御通信とchip-ID取得は成功、映像取得は未達という状態。
追加の開始処理・割込み通知・画像転送の同期について引き続き確認が必要。
`capture_legacy_experimental.py` はその検証用で、本番GUIのバックエンドにはまだ接続していない。


## 更新: 書込み引数訂正と画像取得

上の過去の未達記録を更新する。逆アセンブルを再照合し、書込みはwValue=データ、wIndex=レジスタアドレスと確定。以前の逆順解釈を訂正した。ACKだけでは目的のレジスタ設定を検証できない。
訂正した初期化とbridge start（40/01、value=3、index=15）後、bulk 0x82から2048×1536=3,145,728byteの取得に成功。16KiBずつ読み、開始時の空パケットをスキップした。

明るい被写体で画素値が上昇することを確認したが飽和している。連続取得は2フレーム後にUSB Overflow、その後の試験ではInput/Output Errorも発生。フレーム境界、転送同期、Bayer順序、実露光時間は未確認。画像取得成功と安定ライブ動作は区別する。

`legacy_usb.py`をGUI/CLIの実験用legacy-tcaバックエンドとして追加した。露光はsensor rows、ゲインは未校正のレジスタコード。PNG/TIFF表示画像に加えて8bit元データの.sensor.tiffとJSONを保存する。エラー時は停止し、画像を捏造・補完しない。macOS実機で試験、Windows実機動作は未検証。

低速/標準クロックの通常イベントループではUSB転送停止が再発しました。高速8000で通常ライブ180秒1,976枚・復帰0回、4接続1,126枚・復帰0回を確認し、この設定を採用しています。`process_camera.py`と`usb_worker.py`でUSB処理をGUIから分離し、起動時に標準clear_halt(82)でパイプを初期化しています。設定・保存・停止の合格と連続運転の安定性は分けて記録します。

## 実機によるレジスター読取り訂正（2026-10-06）

0x0AのセンサーアドレスはwIndex、wValueは0。以前の候補表は逆であり、register_readとテストを訂正した。アドレス0だけのチップID試験では判別できなかった。09を読むと012c08（300行）、35は001008（ゲイン2倍）、0Aは800008が返った。旧形式ではすべて162108となる。上記の古い候補表より、この実機確認を優先する。
