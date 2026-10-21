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
