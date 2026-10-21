# KohdaLab Camera interface design

The interface keeps the camera image prominent and makes common actions easy to
find. Controls use restrained colors, concise labels and predictable behavior.
This document records the references and decisions behind version 0.1.7.

## Design references

| Reference | Influence on KohdaLab Camera |
| --- | --- |
| [Apple Human Interface Guidelines — Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars) | A restrained toolbar and clear action hierarchy, with attention focused on the image. |
| Earlier KohdaLab Camera GUI, compared on the Windows desktop | The camera selection workflow and the Camera, Acquisition, Image, Export and Advanced groups. |
| Owner feedback during development | Fit as the minimum zoom, removal of the gap before Cursor, simpler status information, and useful imaging defaults. |

These references inform the project's design decisions. The application uses
egui; it does not reproduce a particular Apple product interface. The earlier
GUI was a visual and interaction reference, not a source of proprietary internal
implementation. Backend provenance is documented separately in
[provenance and redistribution scope](../reference/provenance.md).

## Interaction decisions

| Area | Implemented behavior | Purpose |
| --- | --- | --- |
| Startup | Automatically select a single supported camera; require **Load camera** to connect. Simulation is an explicit choice. | Reduce selection work while keeping connection intentional. |
| Toolbar | Five neutral tabs; clicking the selected tab closes its inspector. F1–F5 open the corresponding inspector. | Keep controls accessible and let the image occupy the workspace. |
| Main actions | Available Load, Apply and Capture actions use a restrained blue accent; unavailable actions remain muted. | Make the next useful action easy to identify. |
| Image navigation | Wheel zoom anchors at the pointer; drag pans; Fit is the minimum zoom and 32× Fit is the maximum. | Prevent the image from becoming too small or getting lost. |
| Zoom controls | Fit, minus, magnification, plus and 100%. The 100% action is enabled only when reachable within the zoom range. | Make scale changes visible and predictable. |
| Status bar | Cursor coordinates and RGB values start at the left edge without a reserved empty column. | Remove distracting space and improve scanning. |
| Details | Concise timing labels with detailed semantics on hover; duplicated display presets removed from Advanced. | Keep everyday use simple while retaining technical information. |
| Defaults | Exposure 120 sensor rows, brightness 1.20×, gamma 2.2 and analog gain 1×. | Start from the owner's preferred imaging settings. |

## Responsiveness and image fidelity

Idle windows no longer schedule continuous repaint polling. Acquisition and
pending operations retain their update scheduling. The preview worker reuses
compiled display recipes until image controls change, avoiding repeated lookup
table preparation. These changes preserve image and export semantics.

Verification included 33 GUI tests, 17 targeted renderer and preview tests,
native Simulation interaction checks, and exact reconstruction of all 786,432
RGB pixels in a saved GR300 endpoint image. Version 0.1.7 did not repeat physical
camera acquisition. The local CPU measurements do not establish a physical FPS
improvement; see [rendering verification](../validation/rendering-performance.md).

## Principles for future changes

- Keep the live image visually dominant and the normal workflow short.
- Use emphasis for meaningful actions and states, with explicit labels.
- Put technical detail behind a deliberate disclosure or hover when practical.
- Preserve RAW data, export fidelity, safe shutdown and truthful status reporting.
- Measure processing changes and verify output equivalence before claiming gains.
- Keep common interactions consistent as individually validated camera backends
  are added.

---

# KohdaLab Camera インターフェース設計 / Interface design（日本語）

interface は camera image を主役にし、一般的な action を見つけやすくします。control は抑えた色、簡潔な label、予測可能な挙動を使います。本書は version 0.1.7 の参照と判断を記録します。

## 設計上の参照 / Design references

[Apple Human Interface Guidelines — Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars) は image に attention を集める抑制した toolbar/明確な action hierarchy の参考です。Windows desktop で比較した以前の KohdaLab Camera GUI は camera selection workflow と Camera/Acquisition/Image/Export/Advanced group、開発中の owner feedback は Fit を最小 zoom とすること、Cursor 前の gap 除去、簡潔な status、実用的 imaging default の参考です。これらは project の設計判断を導くもので、アプリは egui を使い特定 Apple product interface を再現しません。以前の GUI は visual/interaction reference で proprietary internal implementation の source ではありません。backend の来歴は [provenance and redistribution scope](../reference/provenance.md) に別途記載します。

## interaction の判断 / Interaction decisions

startup は一台の対応 camera を自動選択しますが connection には **Load camera** を必須にし、Simulation も明示選択です。toolbar は neutral な 5 tab で、選択 tab の再クリックで inspector を閉じ、F1–F5 は対応 inspector を開きます。利用可能な Load/Apply/Capture は抑制した blue accent、利用不可 action は muted で、次の有用 action を識別しやすくします。wheel zoom は pointer に anchor、drag は pan、Fit は最小・**32× Fit** は最大です。Fit/minus/magnification/plus/**100%** を表示し、100% は zoom range 内で到達可能な時だけ有効です。status bar の cursor coordinate/RGB は reserved empty column なしに左端から始めます。timing label は簡潔にし詳細 semantics は hover、Advanced の重複 display preset は除去します。default は exposure **120 sensor rows**、brightness **1.20×**、gamma **2.2**、analog gain **1×** です。

## 応答性と画像 fidelity / Responsiveness and image fidelity

idle window は continuous repaint polling を schedule せず、acquisition/pending operation は update scheduling を保ちます。preview worker は image control が変わるまで compiled display recipe を再利用し、lookup table preparation の繰返しを避けます。image/export semantics は保持します。検証には **33 GUI tests**、**17** targeted renderer/preview tests、native Simulation interaction、保存済み GR300 endpoint image の **786,432 RGB pixels** 全再構成を含みます。0.1.7 は physical camera acquisition を再実施せず local CPU measurement は physical FPS improvement の根拠ではありません。詳細は [rendering verification](../validation/rendering-performance.md) を参照してください。

## 将来変更の原則 / Principles for future changes

- live image を視覚的に優位にし、通常 workflow を短く保ちます。
- 意味のある action/state を明示 label と強調で示します。
- 可能なら technical detail は意図的な disclosure/hover の奥に置きます。
- RAW data、export fidelity、安全な shutdown、正確な status reporting を保持します。
- 処理変更は gain を主張する前に測定し output equivalence を確認します。
- camera backend を個別に検証して追加しても共通 interaction を一貫させます。
