# Windows 11での最初の試験

Intel/AMDのWindows 11を前提とします。ARM64用メーカーUSBドライバーは未確認です。メーカー構成での最初の試験手順を記録した文書です。現在は専用WinUSBセットアップ付きの自作portable版をtetraで検証済みです。[最新手順](windows-portable.md)と[検証結果](validation.md)を参照してください。

## 1. メーカーソフトで確認

MacのアプリでDisconnectし、カメラをWindows PCへ移します。同じハブでも比較できます。

メーカー配布ページ https://www.shodensha-inc.co.jp/ja/download-software/gr300bcm2/ からドライバーとMeasureを取得し、付属のインストール手順に従います。デバイスマネージャーで対象のハードウェアIDが USB\VID_0547&PID_4D33 か確認してください。インストールがブロックされた場合は、エラーとデバイスの状態を記録してください。配布ZIPのWIN10フォルダーや64bitドライバーの存在は、Windows 11への対応保証ではありません。

Measureで対象機器を選び、まずライブ表示と保存、次に露光・ゲイン変更を確認します。高い設定をそれぞれ1〜2分維持し、映像停止、表示fps、実画像の明るさを記録してください。比較のため同じ被写体・照明・ハブを維持してください。

## 2. Pythonアプリの起動・列挙

提供ZIPをDocumentsへ展開します。ZIPにはkohdalab-cameraフォルダーが入っています。.venvはWindowsで作り直すため同梱していません。

PowerShellでuvをインストールします（公式: https://docs.astral.sh/uv/getting-started/installation/）。

```powershell
winget install --id astral-sh.uv -e
```

PowerShellを開き直し、実際の展開先へ移動して実行します。

```powershell
cd "$env:USERPROFILE\Documents\kohdalab-camera"
uv sync --frozen --python 3.13 --extra gui --extra usb
uv run --frozen kohdalab-camera doctor | Tee-Object -FilePath windows-doctor.json
uv run --frozen kohdalab-camera-gui --backend simulated --start
```

シミュレーション映像が表示されれば、Python/Qtの起動を確認できます。メーカーソフトを完全に閉じてから、任意でDirectShowの列挙・撮影試験を行います。

```powershell
uv run --frozen kohdalab-camera doctor --probe-video --api dshow --limit 5 | Tee-Object -FilePath windows-video.json
```

内蔵Webカメラが映る場合があるため対象機器と取り違えないでください。実カメラを開けた場合は、GUIを起動しAdvancedでBackend=opencv、OS API=dshow、Device index=試験で確認した番号を選びConnectします。

```powershell
uv run --frozen kohdalab-camera-gui --backend opencv
```

## 接続方式の制約

このカメラはUVCではありません。メーカーZIPでは専用.sysに加えDirectShow .axとTWAIN .dsがあり、解析した.ax/.dsは32bitです。64bit Pythonから32bit DirectShowフィルターをロードできないため、上の試験が失敗しても、USB未認識や故障とは限りません。GUIのシミュレーション起動だけで実カメラ対応とは扱いません。

Macで使うlegacy-tcaはlibusb直接アクセスです。メーカーUSBドライバーのまま同じ方法で使えるとは限りません。最初の試験ではZadig/WinUSBへの置換を行わず、まずメーカー構成の結果を確認してください。

Measureは正常、64bit Pythonでは開けない場合の次の実装候補は、メーカーDirectShowフィルターを使う32bit撮影ヘルパーと64bit GUIの分離です。現在そのヘルパーは未実装です。既存のメーカー構成を維持して撮影できるかを優先します。

次に必要な結果は、PCのCPUタイプ、Measureのライブ表示/高露光・高ゲインの成否、windows-doctor.json、windows-video.jsonまたはエラー全文です。
