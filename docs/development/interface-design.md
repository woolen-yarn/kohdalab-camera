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
