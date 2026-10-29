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

---

# 応答性測定 / Responsiveness measurements（日本語）

Version 0.1.9 は development Mac の release build で測定し、CPU rendering time と観測した UI interaction time を区別しています。

## Native Simulation の観測 / Native Simulation observations

UI automation tool による一回の観測では、camera selection まで **1.154 s**、Simulation を読み visible image まで **1.335 s**、monochrome を visible grayscale に切替えるまで **1.322 s**、PNG save の確認から visible `Image saved` message まで **0.701 s** でした。各 interval は input delivery/state inspection と image change の screen capture を含むので、internal application latency ではなく上限です。Simulation は 640 × 480 RGB source を使い、保存 PNG は RAW/recipe replay と全 pixel で一致しました。Windows USB discovery/physical first-frame latency は測定していません。

## CPU rendering

warmup 後に 10 frame × 5 batch を測定し、両 synthetic input は default version 2 recipe と 1024-pixel preview bound を使いました。1024 × 768 Bayer input は Direct ColorImage median **3.740 ms/frame**、Cached recipe median **3.767 ms/frame**、2048 × 1536 は **11.331 ms/frame** と **11.720 ms/frame** でした。recipe compilation 単体は約 **2.2 microseconds** です。cache は table preparation の繰返しを除きますが、total timing は追加 throughput gain を示しません。camera FPS/Windows performance の根拠ではありません。current preview は intermediate RGB buffer を避け GUI state には preview dimensions だけを保持し、完全 idle window は periodic repaint polling を避けます。等価性と過去の測定は [rendering verification](rendering-performance.md) にあります。

## Windows physical observations

公開済み 0.1.9 binaries は有限の SHODENSHA GR300 run を完了しました。この session では mouse click を **200 ms** 保持すると Windows App が input を安定して配送しました。Load から visible physical Live image は **within 21.460 s**、brightness slider click から visibly changed image は **within 0.997 s**、PNG save 確認から `Image saved` は **within 1.519 s**、Disconnect から Disconnected state は **within 0.940 s** でした。**1.011 s** 時点ではまだ Connecting で、次の image observation は 21.460 s です。その間に transport check/inspection を行ったため、21 秒の application startup cost を示す値ではありません。selection window は owner が手動で開いたため process launch からの camera discovery/startup は測定していません。

全 interval は remote input、inspection、screenshot overhead を含みます。image endpoint は acquisition 開始から **399.389 seconds**、accepted frame は **15,701**、rolling counter は **39.5 accepted frames/s** でした。PNG pixel と original Bayer TIFF は RAW と独立に一致し、両 process は normal exit しました。[scoped validation record](validation-0.1.9.json) を参照してください。headline number のために USB behavior や image fidelity を変えるのではなく、実測された遅い stage を最適化してください。この観測は追加 CPU bottleneck や新しい physical FPS improvement を示しません。
