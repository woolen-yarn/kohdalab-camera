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

## Windows physical observations

The exact published version 0.1.9 binaries subsequently completed a finite
SHODENSHA GR300 run. Holding mouse clicks for 200 ms allowed Windows App to
deliver input reliably in this session.

| Action | Time until the result was observed |
| --- | ---: |
| Load to visible physical Live image | within 21.460 s |
| Brightness slider click to visibly changed image | within 0.997 s |
| Confirm PNG save to Image saved message | within 1.519 s |
| Disconnect to Disconnected state | within 0.940 s |

The first observation at 1.011 s still showed Connecting; the next image
observation was at 21.460 s. Transport checks and inspection occurred between
those observations. The interval does not establish a 21-second application
startup cost. Camera discovery/startup from process launch was not timed because
the owner opened the selection window manually.

All intervals include remote input, inspection and screenshot overhead. The
image endpoint was 399.389 seconds into acquisition with 15,701 accepted frames;
the rolling counter at that point was 39.5 accepted frames/s. PNG pixels and
original Bayer TIFF matched RAW independently, and both processes exited
normally. See [the scoped validation record](validation-0.1.9.json).

Optimize a measured slow stage rather than changing USB behavior or reducing
image fidelity to improve a headline number. These observations do not identify
an additional CPU bottleneck or demonstrate a new physical FPS improvement.
