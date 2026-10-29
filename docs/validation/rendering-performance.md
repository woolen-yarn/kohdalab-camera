# Rendering verification

Version 0.1.3 computes only the interpolation values required for each GRBG
site. A validated immutable storage view avoids repeating layout validation
inside the render loop. Public pixel access keeps its checked behavior. No USB,
acquisition, queue, RAW storage or display-recipe semantics changed.

Synthetic release-mode measurements on the development Mac, with the default
recipe and a 1024-pixel preview bound (five batches of ten frames after warmup):

| Bayer input | Prior renderer, median | New renderer, median | Time reduction |
| --- | ---: | ---: | ---: |
| 1024 × 768 | 4.975 ms | 3.438 ms | 30.9% |
| 2048 × 1536 | 15.926 ms | 10.377 ms | 34.8% |

These are local CPU timings, not Windows throughput or physical-camera FPS.
The first generic cached-view attempt was slower and was replaced before release.

Reproduce with `cargo run --release --bin cpu-bench -- --recipe-v2`.
For a saved PNG/RAW/JSON bundle, `--verify-export-bundle path/to/image.png`
compares every output pixel. The optimized renderer reproduced all 786,432 RGB
pixels of the saved GR300 five-minute endpoint image exactly.

Tests additionally cover RGB and Bayer (including the smallest 2 × 2 Bayer
image), all rotations/reflections, monochrome, non-neutral gains/gamma and
preview bounds. Literal independent border and color-plane tests remain in use.

## Version 0.1.6 allocation reduction

The preview now writes Color32 pixels directly through the same traversal used
by RGB exports. It no longer builds a temporary RGB byte vector. The GUI retains
only preview dimensions instead of cloning the entire ColorImage for cursor
mapping. For a 1024 × 768 preview these buffers were 2.25 MiB and 3 MiB,
respectively; these are buffer sizes, not a process-RSS measurement.

The direct path matches `ColorImage::from_rgb(Recipe::render(...))` exactly for
RGB/Bayer, versions 1/2, all orientations, monochrome and preview bounds.
The paired benchmark reports separate timings; samples varied, so this update
makes no guaranteed CPU-throughput or physical-FPS improvement claim.

## Version 0.1.7 interaction and rendering refinement

The cursor readout starts at the left edge without a reserved empty column.
Zoom has Fit as its minimum, with explicit zoom controls and a reachable 100%
mode. Neutral toolbar controls, a collapsible inspector, and concise timing
labels keep the image prominent. The toolbar design takes inspiration from
[Apple Human Interface Guidelines](https://developer.apple.com/design/human-interface-guidelines/toolbars); this remains an egui application.

Idle windows no longer schedule a continuous repaint timer. Acquisition, scans,
exports, shutdown and pending preference writes retain their update scheduling.
The preview worker caches compiled display recipes and rebuilds the lookup table
when the recipe changes. Direct and compiled rendering have equivalent pixels.
Compilation costs approximately 2.1 microseconds on the development host; total
render timings were effectively unchanged (about 3.65 ms at 1024 × 768 and
10.9 ms at 2048 × 1536). These measurements do not establish physical camera
FPS improvement.

---

# レンダリング検証 / Rendering verification（日本語）

Version 0.1.3 は各 GRBG site に必要な interpolation value だけを計算します。validated immutable storage view により render loop 内で layout validation を繰り返しません。public pixel access は checked behavior を保ち、USB、acquisition、queue、RAW storage、display-recipe semantics は変更しません。

development Mac の synthetic release-mode 測定（default recipe、1024-pixel preview bound、warmup 後 10 frame × 5 batch）では、1024 × 768 Bayer input の median は prior **4.975 ms** から new **3.438 ms**（**30.9%** reduction）、2048 × 1536 は **15.926 ms** から **10.377 ms**（**34.8%** reduction）でした。これは local CPU timing であり Windows throughput/physical-camera FPS ではありません。最初の generic cached-view は遅かったため release 前に置き換えました。

再現は `cargo run --release --bin cpu-bench -- --recipe-v2`、保存済み PNG/RAW/JSON bundle は `--verify-export-bundle path/to/image.png` で全 output pixel を比較します。optimized renderer は保存済み GR300 five-minute endpoint image の **786,432 RGB pixels** を正確に再現しました。test は RGB/Bayer（最小 2 × 2 を含む）、全 rotation/reflection、monochrome、non-neutral gain/gamma、preview bound、independent border/color-plane literal test を含みます。

## Version 0.1.6 allocation reduction

preview は RGB export と同じ traversal で Color32 pixel を直接書込み、temporary RGB byte vector を作りません。GUI は cursor mapping 用に ColorImage 全体を clone せず preview dimensions のみを保持します。1024 × 768 preview ではこれらの buffer は **2.25 MiB** と **3 MiB** で、process RSS measurement ではありません。direct path は RGB/Bayer、version 1/2、全 orientation、monochrome、preview bound で `ColorImage::from_rgb(Recipe::render(...))` と完全一致します。paired benchmark は別 timing を示しますが sample は変動し、CPU throughput/physical FPS の保証改善は主張しません。

## Version 0.1.7 interaction and rendering refinement

cursor readout は左端から開始し reserved empty column を持ちません。zoom の最小は Fit で、explicit zoom control と到達可能な **100%** mode があります。neutral toolbar、collapsible inspector、簡潔な timing label が画像を中心に保ちます。[Apple Human Interface Guidelines](https://developer.apple.com/design/human-interface-guidelines/toolbars) を toolbar design の参考にしましたが、これは egui application です。idle window は continuous repaint timer を schedule せず、acquisition、scan、export、shutdown、pending preference write は update scheduling を保ちます。preview worker は compiled display recipe を cache し recipe 変更時に lookup table を rebuild します。direct/compiled rendering の pixel は等価です。development host で compilation cost は約 **2.1 microseconds**、total render timing は実質不変（1024 × 768 で約 **3.65 ms**、2048 × 1536 で **10.9 ms**）で、physical camera FPS 改善の根拠にはなりません。
