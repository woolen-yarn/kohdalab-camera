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
