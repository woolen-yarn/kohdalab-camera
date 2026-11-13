# KohdaLab Camera quick start

KohdaLab Camera is currently distributed for Windows x64. Its supported physical
configuration is the owner-confirmed **SHODENSHA GR300**, using the tested
Micron MT9T001 / USB `0547:4d33` configuration with an existing WinUSB binding.
Other GR300 units, cameras with the same USB ID, and other camera models are not
automatically supported. See [camera support](../reference/hardware.md) for the exact
scope.

## Start a session

![Camera selection with Simulation selected](../images/camera-selection.png)

The screenshots show the native development preview in Simulation on macOS.
The distributed executable is Windows x64; window decorations differ.

1. Extract the complete Windows portable ZIP. Keep `KohdaLab Camera.exe`,
   `continuous-helper.exe`, and the included license files in the same folder.
2. Start **KohdaLab Camera**. The title bar shows the installed application
   version.
3. If one supported USB camera is found, it is selected automatically. Select
   **Load camera** to connect. If more than one candidate is listed, choose the
   intended camera first.
4. Select **Simulation / demo** and then **Load camera** to explore the UI
   without connecting hardware.

The application does not install, replace, reset, or otherwise change camera
drivers. The physical configuration requires the existing WinUSB setup.

## Everyday controls

![Image controls in Simulation](../images/image-controls.png)

The Live window keeps the image in the centre. Use the toolbar, or **F1–F5**, to
open **Camera**, **Acquisition**, **Image**, **Export**, and **Advanced**.
Clicking the selected toolbar tab closes its panel.

- **Camera**: inspect the selected camera and disconnect when finished.
- **Acquisition**: set exposure and analog gain, then use **Apply** to send
  active acquisition changes to the camera.
- **Image**: adjust display brightness, white balance, gamma, rotation, flips,
  monochrome, and image scale. These controls affect the processed preview and
  processed exports; original Bayer data remains available in the export bundle.
- **Export**: save the displayed processed image, or capture a new full-sensor
  frame and then restore the applied Live settings.
- **Advanced**: review diagnostics and restore default settings.

The initial defaults are exposure **120 sensor rows**, brightness **1.20×**,
gamma **2.2**, and analog gain **1×**.

## Navigate the image

Use the mouse wheel over the image to zoom around the pointer, and drag to pan.
Double-click, or use **Fit**, to return to the initial whole-image view.

The scale readout shows **Fit** at the whole-image size and, for example,
**2.0× Fit** when enlarged. **Fit** is the smallest allowed scale: zooming out never makes the image smaller
than its initial whole-image presentation. **100%** means one source pixel per
UI point. It is offered only when that size is reachable between Fit and the
maximum zoom. It is unavailable when Fit already enlarges the image beyond
100%, or when reaching 100% would require more than 32× Fit.

## Save and end normally

Use **Capture image…** to save a processed PNG or TIFF. The dialog proposes
numbered names such as `capture_0001.png`; existing images and their sidecars
are never overwritten.

An export bundle can contain the processed PNG/TIFF, original Bayer TIFF, RAW
data, and metadata. The metadata timestamp is the host UTC time when a complete
frame was accepted; it is not the sensor exposure time.

Choose **Disconnect** when the session is complete, or close the Live window.
Either path requests normal shutdown. Do not remove the helper executable while
the application is running.

Settings, the Live window size, save folder, and capture numbering are
remembered. **Advanced → Reset all settings** returns to the defaults. When a
camera is already active, use **Apply** after resetting settings to update
acquisition controls.

---

# KohdaLab Camera クイックスタート / Quick start（日本語）

KohdaLab Camera の配布対象は Windows x64 です。対応する物理構成は、所有者が確認した既存 WinUSB binding の **SHODENSHA GR300**（Micron MT9T001 / USB `0547:4d33`）です。同一 USB ID の別個体、別の GR300、その他のカメラが自動的に対応するわけではありません。正確な範囲は [camera support](../reference/hardware.md) を参照してください。

## セッション開始 / Start a session

画面写真は macOS 上の Simulation における native development preview です。配布 executable は Windows x64 で、window decoration は異なります。

1. Windows portable ZIP 全体を展開し、`KohdaLab Camera.exe`、`continuous-helper.exe`、同梱ライセンスファイルを同じ folder に置きます。
2. **KohdaLab Camera** を起動します。title bar にインストール済み application version が表示されます。
3. 対応 USB camera が一台なら自動選択されます。接続するには **Load camera** を選択します。候補が複数なら目的の camera を先に選びます。
4. ハードウェアを接続せず UI を試すには **Simulation / demo**、続いて **Load camera** を選択します。

アプリケーションは camera driver を install、replace、reset、変更しません。物理構成には既存の WinUSB setup が必要です。

## 日常の操作 / Everyday controls

Live window は画像を中央に保ちます。toolbar または **F1–F5** で **Camera**、**Acquisition**、**Image**、**Export**、**Advanced** を開きます。選択済み toolbar tab をもう一度クリックすると panel を閉じます。

- **Camera**: 選択 camera を確認し、終了時に disconnect します。
- **Acquisition**: exposure と analog gain を設定し、**Apply** で active camera に送ります。
- **Image**: display brightness、white balance、gamma、rotation、flip、monochrome、scale を調整します。処理済み preview/export にだけ影響し、元の Bayer data は export bundle に残ります。
- **Export**: 表示中の処理済み画像を保存するか、新しい full-sensor frame を取得して適用済み Live settings に戻します。
- **Advanced**: diagnostic を確認し、default settings を復元します。

初期値は exposure **120 sensor rows**、brightness **1.20×**、gamma **2.2**、analog gain **1×** です。

## 画像の移動 / Navigate the image

画像上の mouse wheel は pointer 中心で zoom、drag は pan、double-click または **Fit** は最初の全画像表示に戻します。scale 表示の **Fit** は全画像サイズ、たとえば **2.0× Fit** は拡大表示です。**Fit** が最小 scale なので、最初の全画像表示より小さく zoom out することはありません。**100%** は source pixel 1 個を UI point 1 個で表示する意味で、Fit と最大 zoom の間で到達可能なときだけ表示されます。Fit が既に 100% より大きい場合、または 100% が **32× Fit** より必要な場合は利用できません。

## 保存と通常終了 / Save and end normally

**Capture image…** で処理済み PNG/TIFF を保存します。dialog は `capture_0001.png` のような番号付き名を提案し、既存画像と sidecar は上書きしません。export bundle は処理済み PNG/TIFF、original Bayer TIFF、RAW、metadata を含められます。metadata timestamp は完全 frame を受理した host UTC であり、sensor exposure time ではありません。

セッション完了時は **Disconnect** を選ぶか Live window を閉じます。どちらも normal shutdown を要求します。実行中は helper executable を移動・削除しないでください。settings、Live window size、save folder、capture numbering は記憶されます。**Advanced → Reset all settings** は default に戻し、camera が active の場合は acquisition control を反映するため **Apply** を選択します。
