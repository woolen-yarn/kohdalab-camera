# Camera support and validation

## Current physical backend

### SHODENSHA GR300

The current supported commercial camera is **SHODENSHA GR300**, identified by the
owner of the tested laboratory unit. Manufacturer and camera model are the
primary identifiers; sensor and USB information are supplementary details.

The tested unit uses a **Micron MT9T001** sensor and USB ID **`0547:4d33`**. It
reports `3.1M USB2.0 Camera` as its USB product name and is handled by the current
ScopeTek-family backend. The backend family is a technical classification, not
the product's manufacturer name.

Windows driver metadata currently uses "KohdaLab" as a registration name; it
does not replace the SHODENSHA product identity. Support is scoped to this tested
unit. Other GR300 revisions, GR300-series variants, or devices sharing the same
USB identifiers or sensor are not automatically qualified.

| Property | Current scope |
| --- | --- |
| Manufacturer / model | SHODENSHA GR300 (tested unit, owner-confirmed identity) |
| Distributed operating system | Windows x64 |
| USB access | Existing WinUSB binding; no automatic driver installation or replacement |
| Live mode | 1024 × 768 |
| Full-sensor capture | 2048 × 1536, followed by restoration of applied Live settings |
| Sensor data | GRBG Bayer; upper 8 bits of the sensor's 10-bit data path |
| Image controls | Exposure and gain; processed-image brightness, white balance, gamma, rotation, flips, and monochrome |
| Exports | Processed PNG/TIFF, original Bayer TIFF, RAW, and metadata |
| Timestamp semantics | Host UTC at complete-frame acceptance; not sensor exposure time |
| Frame-rate semantics | Acquisition and preview update rates reported separately; no guaranteed or optically calibrated rate |

The GUI filters physical candidates by USB ID. Enumeration is not proof of a
compatible sensor, successful acquisition, or hardware qualification. Confirm
USB identity, sensor, firmware compatibility, and driver setup before treating a
second unit as supported.

## Validation status

The exact published version 0.1.9 binaries completed 399.4 seconds of physical
Live acquisition, image adjustment and Fit controls, a pixel-checked endpoint
export, and normal GUI/helper shutdown. A single supported camera was
automatically selected. See [the scoped record](../validation/validation-0.1.9.json).

The owner confirmed manual mouse use, arbitrary resizing and retention of the
changed size during earlier checks. See [the earlier record](../validation/validation-0.1.4.json).

The owner subsequently reported acceptable color and field of view by visual
inspection. Optical/color calibration has not been performed. Full-sensor capture/Live restoration were checked in pre-publication
feature code but not repeated in this finite run. Picker round-trip size
retention was not repeated. Earlier records retain their original scope and
runtime hashes.

## Other cameras and operating systems

Simulation is an explicit synthetic source and does not imply support for a
physical device. UVC webcams, other ScopeTek variants, and cameras using vendor
SDKs currently have no supported backend. No macOS or Linux physical-camera
release is offered.

The project aims to support multiple camera families through a common GUI and
acquisition model. Each future backend must document its supported device and
firmware identifiers, operating systems, dependency redistribution terms,
controls, pixel formats, timestamp semantics, cleanup behavior, and validation
limits. Hardware support will be added to this matrix only after scoped testing.

---

# カメラ対応と検証 / Camera support and validation（日本語）

## 現在の物理 backend / Current physical backend

現在対応する商用 camera は、検証済み laboratory unit の所有者が **SHODENSHA GR300** と確認したものです。製造元と camera model を主識別子とし、sensor と USB 情報は補助情報です。検証済み unit は **Micron MT9T001**、USB ID **`0547:4d33`**、USB product name `3.1M USB2.0 Camera` を報告し、現行 ScopeTek-family backend で扱います。backend family は技術分類であって製造元名ではありません。Windows driver metadata の "KohdaLab" は登録名であり SHODENSHA 製品名を置き換えません。

対応範囲はこの tested unit に限ります。別の GR300 revision、GR300-series variant、同じ USB identifier/sensor を持つ装置は自動的には qualified されません。配布 OS は Windows x64、USB access は既存 WinUSB binding（自動 driver install/replace なし）、Live は 1024 × 768、full-sensor capture は 2048 × 1536 後に適用済み Live settings へ復帰します。sensor data は 10-bit path の上位 8 bits を使う GRBG Bayer です。image controls は exposure/gain、処理画像の brightness/white balance/gamma/rotation/flips/monochrome、exports は処理済み PNG/TIFF、original Bayer TIFF、RAW、metadata です。timestamp は complete-frame acceptance 時の host UTC で sensor exposure time ではなく、acquisition/preview update rate は別々に表示され、保証済み・光学的に校正済みの rate ではありません。

GUI は USB ID で物理候補を filter します。enumeration は互換 sensor、成功した acquisition、hardware qualification の証明ではありません。二台目を対応扱いする前に USB identity、sensor、firmware compatibility、driver setup を確認してください。

## 検証状況 / Validation status

公開済み **0.1.9** binaries は SHODENSHA GR300 で 399.4 秒の physical Live acquisition、image adjustment/Fit controls、pixel-checked endpoint export、通常の GUI/helper shutdown を完了しました。対応 camera 一台は自動選択されました。[scoped record](../validation/validation-0.1.9.json) を参照してください。所有者は過去の確認で manual mouse、任意サイズへの resize、変更サイズの保持も確認しています（[earlier record](../validation/validation-0.1.4.json)）。色と field of view は視覚的に acceptable と報告されていますが optical/color calibration は行っていません。full-sensor capture/Live restoration は公開前 feature code で確認済みでもこの有限 run では再実施しておらず、picker round-trip size retention も再実施していません。以前の記録は元の scope と runtime hash を保ちます。

## 他のカメラと OS / Other cameras and operating systems

Simulation は明示的な synthetic source で、物理 device の対応を意味しません。UVC webcam、別の ScopeTek variant、vendor SDK を使う camera に対応 backend はありません。macOS/Linux 向け physical-camera release も提供していません。将来 backend ごとに対応 device/firmware identifier、OS、dependency redistribution terms、controls、pixel format、timestamp semantics、cleanup behavior、validation limits を文書化し、限定検証後にのみこの matrix に追加します。
