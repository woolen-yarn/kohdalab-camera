# Responsiveness measurements

Version 0.1.9 was measured on the development Mac using the release build.
The results distinguish CPU rendering time from observed UI interaction time.

## Native Simulation observations

| Action | Time until the result was observed |
| --- | ---: |
| Launch to camera selection | 1.154 s |
| Load Simulation to visible image | 1.335 s |
| Toggle monochrome to visible grayscale image | 1.322 s |
| Confirm PNG save to visible Image saved message | 0.701 s |

These are single observations made with a UI automation tool. Each interval
includes input delivery and state inspection (plus screen capture for image
changes), so it is an upper
bound rather than an internal application latency measurement. Simulation uses
a 640 × 480 RGB source. Its saved PNG matched the RAW/recipe replay at every
pixel. These values do not measure Windows USB discovery or physical first-frame
latency.

## CPU rendering

Five batches of ten frames followed warmup. Both synthetic inputs used the
default version 2 recipe and a 1024-pixel preview bound.

| Bayer input | Direct ColorImage median | Cached recipe median |
| --- | ---: | ---: |
| 1024 × 768 | 3.740 ms/frame | 3.767 ms/frame |
| 2048 × 1536 | 11.331 ms/frame | 11.720 ms/frame |

Recipe compilation alone took about 2.2 microseconds. The cache removes repeated
table preparation, but these total timings show no additional throughput gain.
They do not establish camera FPS or Windows performance.

The current preview avoids an intermediate RGB buffer and retains only preview
dimensions in GUI state. Completely idle windows avoid periodic repaint polling.
See [rendering verification](rendering-performance.md) for equivalence checks and
earlier measurements.

## Further measurement

Physical startup and save observations require functioning Windows input and a
fresh finite camera run. Use the exact released binaries, retain the original
output bundle, and distinguish remote-control overhead from application work.
Optimize a measured slow stage rather than changing USB behavior or reducing
image fidelity to improve a headline number.
