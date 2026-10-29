# KohdaLab Camera

A Rust desktop platform for laboratory camera acquisition, live imaging, image
adjustment, and reproducible exports.

The long-term goal is a common application for cameras from multiple vendors,
with device-specific backends and a consistent capture workflow. **The current
release supports one physical camera configuration only**; broader camera
support is a roadmap, not an existing capability.

## Current camera support

| Manufacturer / camera model | Sensor / USB identity | Current support |
| --- | --- | --- |
| **SHODENSHA GR300** | Micron MT9T001; USB `0547:4d33`; ScopeTek-family backend. | Tested on one laboratory unit. Windows x64 with an existing WinUSB binding; 1024 × 768 Live and 2048 × 1536 full-sensor capture. |
| Simulation / demo | Synthetic source; no camera sensor | Available without hardware for exploring the interface. |
| Other camera models | UVC webcams, other USB cameras, and vendor SDK devices | Not supported yet; multi-camera support is planned. |

The tested camera is identified by its owner as **SHODENSHA GR300**. Its USB
product string is `3.1M USB2.0 Camera`; the sensor and USB ID above describe this
tested unit. Other GR300 revisions and similarly named models have not been
validated.

A matching USB ID alone does not establish compatibility with another camera,
firmware revision, or sensor. See [camera support and validation](../reference/hardware.md).

## Downloads and quick start

The current published baseline is **0.1.11**, for **Windows x64**.

- [Latest release and portable downloads](https://github.com/woolen-yarn/kohdalab-camera/releases/latest)
- [Windows Setup (unsigned)](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.11/KohdaLab-Camera-0.1.11-Setup.exe)
- [Windows portable ZIP](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.11/kohdalab-camera-0.1.11-windows-x64.zip)
- [Source ZIP](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.11/kohdalab-camera-0.1.11-source.zip)
- [SHA-256 checksums](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.11/SHA256SUMS.txt)

Run Setup for a per-user installation, or extract the entire Windows ZIP and launch `KohdaLab Camera.exe`. Keep
`continuous-helper.exe` and the license files alongside it. Rust and Python are
not required to run the portable application. The physical backend requires an
existing WinUSB setup; the application does not install or change drivers.
No macOS or Linux executable is distributed in this release.

See [Windows installation, drivers and signing](windows-installation.md)
for the installed-driver policy, Setup workflow, signature status and execution
warning diagnosis. Official SDK integration is deferred; existing manufacturer
drivers are preserved rather than replaced.

### Code signing policy

Current Windows downloads are **unsigned**. Free SignPath Foundation signing is
being prepared; no application has been submitted or accepted. See the
[Code signing policy](../reference/code-signing.md) and
[application preparation](../development/signpath-application.md) for current status,
privacy, intended approval roles and outstanding build requirements.

For the current workflow and control reference, see the
[Quick start guide](quick-start.md).

The guide includes current Simulation screenshots. See
[responsiveness measurements](../validation/responsiveness.md) for measured rendering
costs and observed interaction times, including their scope and limitations.

1. Select the supported USB camera, then choose **Load camera**.
   Choose **Simulation / demo** explicitly to explore without hardware.
2. Use the **Camera / Acquisition / Image / Export / Advanced** tabs.
   **F1–F5** also select the tabs.
3. Adjust exposure and gain, then apply acquisition settings. Image adjustments
   affect the processed image while the original Bayer data remains available.
4. Use **Capture image…** to export the displayed image, or
   **Capture full sensor image…** to acquire a new full-resolution frame and
   return to the previously applied Live settings.
5. Use **Disconnect** or close the window to end the session normally.

## Imaging and data

See [interface design and references](../development/interface-design.md) for the UI principles,
design influences and interaction decisions.

- Per-pixel GRBG demosaicing, linear display sampling, white balance, gamma,
  rotation, flips, and monochrome display.
- Brightness is a positive multiplier with a default of **1.20×**.
- Processed PNG/TIFF exports, original Bayer TIFF, RAW data, and metadata.
  Existing output names are not overwritten.
- Capture FPS in the status bar, with preview timing available on hover.
- Timestamps represent **host UTC time when a complete frame was accepted**,
  rather than sensor exposure time.
- A separate acquisition helper owns the physical session. When cleanup cannot
  be established, the application retains its protective interlock rather than
  automatically clearing locks or resetting USB.

When one supported camera is found, it is selected automatically. Press **Load
camera** to open Live. With multiple cameras, choose one first.

Image navigation: wheel to zoom, drag to pan, double-click or **Image → Fit**
to fit the window, and **Image → 100%** for one source pixel per UI point.
Zooming out stops at the initial **Fit** size. Click the selected tab again to
close the inspector. **100%** is available when it falls between Fit and the
maximum zoom. Preview remains bounded to 1024 pixels; exports retain full source dimensions.

Camera/image settings, window size, save folder and capture numbering are
remembered. **Advanced → Reset all settings** restores defaults; press Apply
to update active camera acquisition. Startup still requires **Load camera**.
Save dialogs suggest numbered names such as `capture_0001.png`; existing files
and sidecars are never overwritten.

Default settings: exposure **120 sensor rows**, brightness **1.20×**, gamma
**2.2** and analog gain **1×**.

## Validation and known limitations

Version 0.1.11 is built and installation-tested in [public Windows CI](https://github.com/woolen-yarn/kohdalab-camera/actions/runs/39187983298).
GUI/helper/Setup/uninstaller metadata and unsigned status, runtime hashes,
install/update/uninstall and user-file preservation passed. The exact CI runtime
was installed and startup-checked on the workstation. See
[validation](../validation/validation-0.1.11.json) and [CI details](../development/continuous-integration.md).
No new physical capture validation is claimed for this build.

Version 0.1.10 added installed-driver preservation and startup diagnostics. Windows
Setup was checked for fresh install, update, uninstall and preservation of an
unrelated user file. Its application, helper, uninstaller and Setup are unsigned;
SignPath application preparation is available; enrollment/signing are not active. The last physical capture
validation is version 0.1.9 below, not a new 0.1.10 hardware test.

The exact published version 0.1.9 binaries completed 399.4 seconds of Live
acquisition on SHODENSHA GR300, with 15,701 accepted frames. The camera was
selected automatically, and **Load camera** opened Live. Brightness adjustment,
display-default restoration, zoom and Fit buttons, PNG/RAW/Bayer TIFF export,
Disconnect and Close were exercised. Every PNG pixel matched independent RAW
reconstruction; original Bayer TIFF matched RAW. GUI and helper exited normally,
with no remaining process or protective lock. See the
[scoped validation record](../validation/validation-0.1.9.json).

The owner confirmed normal manual mouse operation, arbitrary window resizing and
retention of the changed size in earlier checks. Held mouse clicks worked
through Windows App in the latest run; keyboard and wheel delivery have tooling
limitations. Full-sensor switching and picker round-trip resizing were not
repeated in this run. The owner previously reported that color and field of view looked
acceptable. This is a visual assessment; no optical or color calibration was performed.

The software renderer is pixel-equivalent to the previous version and passed
independent pixel tests. See [performance verification](../validation/rendering-performance.md).

Observed acquisition/preview rates around 39.5 frames/s are specific to the test
conditions and are not a guaranteed performance specification or an optical
frame-rate calibration.

## Platform roadmap

1. Complete interaction, image-quality, and device qualification for the current backend.
2. Define a shared capability model for discovery, connection, acquisition,
   controls, frame formats, timestamps, and safe shutdown.
3. Add individually validated camera backends using documented, redistributable
   interfaces; keep vendor-specific behavior behind those backends.
4. Extend the common GUI and export workflow across supported cameras, publishing
   a compatibility matrix and limitations for every backend and operating system.

See [CONTRIBUTING.md](../../CONTRIBUTING.md) before proposing a camera backend.
The [extension plan](../development/extensibility.md) describes current boundaries and the
implementation order for a camera from another manufacturer.

## Build and validation

Use Rust stable with edition 2024 support. Dependency versions are pinned in
`Cargo.lock`.

```sh
cargo build --locked --release --package camera-lab --bin camera-lab
cargo build --locked --release --package continuous-helper-entry --bin continuous-helper
cargo test --locked --lib --bin camera-lab
```

Cross-compiling the Windows GNU target additionally requires the
`x86_64-pc-windows-gnu` Rust standard library and MinGW-w64 gcc/windres:

```sh
cargo build --locked --release --target x86_64-pc-windows-gnu --package camera-lab --bin camera-lab
cargo build --locked --release --target x86_64-pc-windows-gnu --package continuous-helper-entry --bin continuous-helper
```

Unit tests do not operate a physical camera. Building for another operating
system does not establish physical-backend support or distribution readiness.

## License and provenance

The application icon is project artwork generated for KohdaLab Camera; the PNG
source and Windows ICO are included under the project license.

Project code is MIT licensed. Retain [LICENSE](../../LICENSE),
[THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md), and the texts under
`third-party-licenses/` when redistributing. The bundled HTML is a Rust runtime
copyright notice, not an HTML implementation of the application.

See [provenance and redistribution scope](../reference/provenance.md) for the current
backend's research inputs and limitations. Manufacturer software, legacy
application source/binaries, and manufacturer artwork are not bundled.

---

# KohdaLab Camera（日本語）

KohdaLab Camera は laboratory camera acquisition、live imaging、image adjustment、reproducible export のための Rust desktop platform です。長期目標は vendor ごとの backend と一貫した capture workflow を持つ共通 application ですが、**current release が対応する物理 camera configuration は一つだけ**です。より広い camera support は roadmap であり既存 capability ではありません。

## 現在の対応とダウンロード / Current support and downloads

| Camera | 現在の対応 |
| --- | --- |
| **SHODENSHA GR300** | Micron MT9T001、USB `0547:4d33`、ScopeTek-family backend。existing WinUSB binding を持つ Windows x64 の laboratory unit 一台で検証済み。1024 × 768 Live、2048 × 1536 full-sensor capture。 |
| Simulation / demo | synthetic source。hardware なしで interface を試せます。 |
| その他 | UVC webcam、他 USB camera、vendor SDK device は未対応。multi-camera support は planned。 |

tested camera は owner により **SHODENSHA GR300** と識別され、USB product string は `3.1M USB2.0 Camera` です。sensor/USB ID は tested unit の補助情報であり、別 GR300 revision/同名 model は検証していません。USB ID 一致だけでは firmware/sensor/camera compatibility を示しません（[camera support and validation](../reference/hardware.md)）。

公開 baseline は **0.1.11**、対象は **Windows x64** です。[Latest release and portable downloads](https://github.com/woolen-yarn/kohdalab-camera/releases/latest)、[Windows Setup (unsigned)](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.11/KohdaLab-Camera-0.1.11-Setup.exe)、[Windows portable ZIP](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.11/kohdalab-camera-0.1.11-windows-x64.zip)、[Source ZIP](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.11/kohdalab-camera-0.1.11-source.zip)、[SHA-256 checksums](https://github.com/woolen-yarn/kohdalab-camera/releases/download/v0.1.11/SHA256SUMS.txt) を提供します。Setup は per-user install、portable は ZIP 全体を展開して `KohdaLab Camera.exe` を起動します。`continuous-helper.exe` と license file を同じ場所に保ちます。Rust/Python は portable app 実行に不要です。physical backend は existing WinUSB setup を必要とし、app は driver を install/change しません。macOS/Linux executable は配布していません。driver/signing/execution warning は [Windows installation](windows-installation.md) を参照し、official SDK integration は deferred で existing manufacturer driver を置換しません。

Windows download は **unsigned** です。free SignPath Foundation signing を準備中ですが application は未提出/未受諾です。[Code signing policy](../reference/code-signing.md) と [application preparation](../development/signpath-application.md) に status/privacy/role/build requirement を示します。[Quick start guide](quick-start.md) は Simulation screenshot、[responsiveness measurements](../validation/responsiveness.md) は rendering cost/interaction time とその scope/limitation を示します。

## 基本ワークフロー、画像、データ / Workflow, imaging and data

1. supported USB camera を選び **Load camera** を選択します。hardware なしで試すなら **Simulation / demo** を明示的に選びます。
2. **Camera / Acquisition / Image / Export / Advanced** tab（**F1–F5** でも選択）を使います。
3. exposure/gain を調整して acquisition setting を apply します。image adjustment は processed image に影響し original Bayer data は保持します。
4. **Capture image…** は displayed image を export し、**Capture full sensor image…** は新しい full-resolution frame を取得して previously applied Live setting に戻します。
5. **Disconnect** または window close で normal session end を要求します。

[interface design and references](../development/interface-design.md) に UI principle を記録します。per-pixel GRBG demosaicing、linear display sampling、white balance、gamma、rotation、flip、monochrome display を備えます。brightness は正の multiplier（default **1.20×**）です。processed PNG/TIFF、original Bayer TIFF、RAW、metadata を export し既存 output name を上書きしません。status bar は capture FPS、hover は preview timing を示します。timestamp は sensor exposure time でなく complete frame を accepted した host UTC です。separate acquisition helper が physical session を所有し、cleanup を確立できない時は lock を自動解除/reset USB せず protective interlock を維持します。

supported camera が一台なら自動選択されますが Live を開くには **Load camera** が必要です。複数なら先に選択します。image は wheel zoom、drag pan、double-click/**Image → Fit** で window に fit、**Image → 100%** は source pixel 一つを UI point 一つで示します。zoom out は initial **Fit** で止まり、selected tab 再クリックで inspector を閉じます。**100%** は Fit と maximum zoom の間にある場合だけ有効です。preview は 1024 pixels に bounded ですが export は full source dimension を保持します。camera/image setting、window size、save folder、capture numbering は記憶し、**Advanced → Reset all settings** は default に戻します。active camera では **Apply** で acquisition を更新します。save dialog は `capture_0001.png` のような name を提案し existing file/sidecar は上書きしません。default は exposure **120 sensor rows**、brightness **1.20×**、gamma **2.2**、analog gain **1×** です。

## 検証・制限・roadmap / Validation, limitations and roadmap

0.1.11 は [public Windows CI](https://github.com/woolen-yarn/kohdalab-camera/actions/runs/39187983298) で build/install test 済みです。GUI/helper/Setup/uninstaller metadata と unsigned status、runtime hash、install/update/uninstall/user-file preservation を pass し、exact CI runtime は workstation に install/startup-check 済みです（[validation](../validation/validation-0.1.11.json)、[CI details](../development/continuous-integration.md)）。この build に new physical capture validation はありません。0.1.10 は installed-driver preservation/startup diagnostic と Windows Setup を fresh install/update/uninstall/unrelated file preservation で確認しましたが application/helper/uninstaller/Setup は unsigned、physical capture は 0.1.9 のままです。

exact published 0.1.9 binary は SHODENSHA GR300 で **399.4 seconds**、**15,701 accepted frames** の Live acquisition、automatic selection/**Load camera**、brightness/default restore/zoom/Fit/PNG-RAW-Bayer TIFF export/Disconnect/Close を完了しました。全 PNG pixel は independent RAW reconstruction と、original Bayer TIFF は RAW と一致し、GUI/helper は remaining process/protective lock なく normal exit しました（[scoped validation record](../validation/validation-0.1.9.json)）。owner は past check で manual mouse/arbitrary resize/size retention を確認し、latest run では held mouse click が Windows App で動作しました。keyboard/wheel tool limitation、full-sensor switching/picker round-trip resize の未再実施、visual color/FOV assessment は optical/color calibration ではない点は残ります。software renderer は previous version と pixel-equivalent で independent pixel test を pass しました（[performance verification](../validation/rendering-performance.md)）。観測した約 **39.5 frames/s** は test condition 固有で保証値/optical frame-rate calibration ではありません。

roadmap は current backend の interaction/image/device qualification 完了、discovery/connection/acquisition/control/frame/timestamp/safe shutdown の shared capability model 定義、documented/redistributable interface を持つ individually validated backend 追加、各 backend/OS の compatibility matrix/limitation と共通 GUI/export workflow の拡張です。camera backend 提案前に [CONTRIBUTING.md](../../CONTRIBUTING.md)、実装順は [extension plan](../development/extensibility.md) を参照してください。

## Build、license、provenance

Rust stable と edition 2024 support、`Cargo.lock` の pinned dependency を使います。

```sh
cargo build --locked --release --package camera-lab --bin camera-lab
cargo build --locked --release --package continuous-helper-entry --bin continuous-helper
cargo test --locked --lib --bin camera-lab
```

Windows GNU cross build には `x86_64-pc-windows-gnu` Rust standard library と MinGW-w64 gcc/windres が追加で必要です。

```sh
cargo build --locked --release --target x86_64-pc-windows-gnu --package camera-lab --bin camera-lab
cargo build --locked --release --target x86_64-pc-windows-gnu --package continuous-helper-entry --bin continuous-helper
```

unit test は physical camera を操作せず、別 OS build は physical-backend support/distribution readiness を示しません。application icon は KohdaLab Camera 用に生成した project artwork で PNG source/Windows ICO を project license で含めます。project code は MIT で、再配布時は [LICENSE](../../LICENSE)、[THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md)、`third-party-licenses/` を保持します。bundled HTML は application の HTML implementation でなく Rust runtime copyright notice です。[provenance and redistribution scope](../reference/provenance.md) に research input/limitation を示し、manufacturer software、legacy application source/binary、manufacturer artwork は同梱していません。
