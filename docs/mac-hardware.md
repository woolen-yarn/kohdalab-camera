# Mac実機確認 — 2026-10-06

macOS arm64、接続中のカメラをIORegistryで確認した。

| 項目 | 確認値 |
| --- | --- |
| 製品文字列 | 3.1M USB2.0 Camera |
| 製造元文字列 | DOSENSOR |
| VID / PID | 0547 / 4D33 |
| USBリンク | 480 Mbps（IORegistry表示） |
| device class | 0x00（インターフェース側を確認する機器） |
| interface 0 | class 0xFF / subclass 0 / protocol 0 |
| alternate | 0 |
| endpoint数 | 2（アドレス・転送種別は未取得） |

現在のインターフェースはメーカー独自USB。UVC VideoControl/VideoStreamingは確認されない。
VID/PIDはTucsenのTCA-3.0C公式一覧と一致。ただしSHODENSHA完全型番・OEM仕様までは確定できない。
[Tucsen旧機種一覧](https://www.tucsen.com/Home/Product/info/dataid/35.html)

AVFoundationの動画デバイス一覧は空だった。可視性・権限制限の可能性もあるため、この結果単独で方式は判断しない。
独自インターフェースの情報と合わせ、現状の標準動画API/OpenCV接続では取得できないと判断。
**実カメラからの映像取得・露光/ゲイン変更・撮影保存は未達。模擬映像を実機映像として扱っていない。**

## 再現手順

```sh
uv sync --extra gui --extra mac --extra dev --frozen
uv run kohdalab-camera doctor --list-video
uv run pytest -q
```

- IORegistry機器とインターフェースを読み、同じVID/PID/locationIDに対応付ける列挙を追加。
- system_profilerのUSB JSONが空でも実機が存在するケースを修正。
- PyObjC/AVFoundationの非撮影一覧を追加し、mac extraとuv.lockで管理。
- 回帰テスト: 15件成功（IORegistryパーサー、代替列挙を追加）。
- 詳細診断JSONはrepoルートの `local-diagnostics-mac.json` に保存しGit対象外にした。

## USB直接アクセスの試験

PyUSBと同梱libusbを `uv run --with pyusb --with libusb-package` で試験。
libusbバックエンド自体はロードできたが、機器一覧は空。
IORegistryには実機が存在するため、この実行環境におけるlibusbの可視性/アクセス条件を別途確認する必要がある。
権限が原因と断定はしていない。configuration変更、driver detach、reset、独自制御コマンド、ファームウェア書き込みは行っていない。

## 次の実装に必要な情報

1. 付属CDまたはTSView/TCA SDK内のヘッダー、DLL/ライブラリ、サンプル、USBドライバINF。
2. このVID/PIDに対応するmacOS用ライブラリの有無、Intel/arm64対応とライセンス。
3. Mac SDKがない場合は、旧Windowsドライバ/SDKから撮影開始・センサー設定・フレーム転送の仕様を調べ、
   自前のユーザーモードUSBバックエンドが実装可能か評価。

USB一覧に出ることだけでは映像取得コマンドはわからない。現行TUCAM APIの関数を推測で呼ぶ実装はしていない。
