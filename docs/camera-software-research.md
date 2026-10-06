# Camera software and transport research (2026-10-06)

## Primary sources

- Tucsen legacy model list: https://www.tucsen.com/uploads/Software-and-Driver-download-links_EN.pdf — TCA-3.0C has VID/PID 0547:4D33 and uses TSView6/7 on Windows. This identifies the relevant family, not every OEM variation.
- Tucsen FAQ: https://www.tucsen.com/Public/upload/download/pdf/FAQ-EN.pdf — newer TCapture excludes the TCA series. It also discusses reducing preview resolution when preview is slow or a camera is not recognized. A current Mac download is not evidence of compatibility with this legacy device.
- Micro-Manager Dhyana adapter: https://micro-manager.org/Dhyana — Tucsen adapter uses TUCam DLL and is Windows-only. This is not an available macOS backend for our device.
- Micro-Manager host setup: https://micro-manager.org/Personal_computer_setup — discusses USB bus bandwidth and separating high-bandwidth devices.
- libusb I/O documentation: https://libusb.sourceforge.io/api-1.0/libusb_io.html — asynchronous transfers support multiple pending requests and explicit cancellation; synchronous I/O internally uses asynchronous calls. Buffers must survive completion.
- MT9T001 Rev D datasheet: https://datasheet.octopart.com/MT9T001P12STC-Micron-datasheet-136597.pdf — registers 0x22/0x23 control color-preserving binning, 0x06 vertical blanking controls frame period. Binning reduces transmitted data while preserving field of view; it reduces output resolution.

## Vendor implementation comparison

GR300W.ax at 0x10006680 chooses full, 2x and 3x modes. Full writes 0 to 0x22/0x23; 2x writes 0x11 to both; 3x writes 0x22 with a 1920x1440 window. The 2x path agrees with the sensor datasheet and produces 1024x768 from the existing full field.

The PDF's official TCA-3.0C installer links no longer returned a downloadable binary on the checked .net/.com hosts. No installer was executed. Existing SHODENSHA driver/Measure analysis remains the static reference.

## Experiments and limits

UI dropdowns now use Qt Fusion instead of macOS native popup rendering under a custom stylesheet, fixed minimum widths, readable popup colors, and scrollable tab pages.

Several plausible transport alternatives did not resolve the measured error: rendering outside the acquisition child, 4 pending requests, 64KiB requests, serial reads, standalone USB reset, and longer frame blanking at full resolution. Those transport experiments were removed. The application retains the original 32x512KiB asynchronous receiver and safe cancel-before-release behavior.

The adopted default low-bandwidth profile uses the vendor's 2x binning and vertical blanking 1023. A native Qt 180.151-second run captured 2,327 frames with zero USB recoveries. High exposure/gain tests (300 rows/2x and 600 rows/4x) still caused receive errors, including when configured before opening. A device reset at the control change did not eliminate that failure; the extra reset at manual apply was removed. Recovery after a receive failure retains an owned-device reset after transfer cleanup. Do not call a recovered error a clean run. The USB topology is still the same Fresco Logic hub; a different connection has not been measured.

The Export tab offers a full-sensor still operation: stop low-bandwidth preview, initialize full 2048x1536, acquire and save an owned frame, then restore the previous preview profile even after a read error. A real Qt test saved a full-sensor PNG matching every reconstructed pixel and resumed 1024x768 live view with zero recoveries. This single-still test does not establish continuous full-resolution stability.

## Further same-hub investigation

The user requested continued diagnosis without changing the hub. Additional measured comparisons, with automatic retry disabled:

| Method | Result |
| --- | --- |
| Vendor 2x transfer size 128KiB, four queued requests, 300 rows/2x | 202 frames in 16.58s, image transfer error |
| Same receiver, 8001 clock / vendor default vertical blank 25 | 11 frames, image transfer error |
| Queue receiver before bridge streaming starts | 188 frames in 15.50s, image transfer error |
| Opposite pixel-clock polarity (0000) | 2 frames, image transfer error; not adopted |
| Sensor control synchronization bit 07[0], no reconnect | Baseline 260 frames/20s; after 300 rows/2x, image transfer error |
| libusb 1.0.26 vs installed 1.0.30, isolated dylib | Old library also failed; dependencies unchanged |
| Native Apple IOUSBHost synchronous API, outside Python/libusb | 77,197,525 bytes / 609 requests, IOReturn 0xe00002ed (not responding) |
| Pipe restart with bridge stop/start, keeping 300 rows/2x | First restart ~4ms, later repeated errors; not a clean run |
| Sensor output stop + bridge 1/0/1 resynchronization | Restarts ~13ms, repeated errors; not adopted |
| Sensor snapshot mode (1E[8], software trigger 0B[0]), 600 rows/4x | 7 frames before image transfer error; not adopted |
| Vendor 3x 640x480 mode, 80KiB/four requests, 600 rows/4x | 7 frames before image transfer error; not adopted |

Linux's official MT9T001 driver confirms the control synchronization bit and analog gain encoding: https://raw.githubusercontent.com/torvalds/linux/v4.19/drivers/media/i2c/mt9t001.c . Apple framework APIs were checked against the installed SDK headers (IOUSBHostObject.h / IOUSBHostPipe.h). These results narrow the failure but do not establish a hardware cause.

**Register-read correction:** the USB sensor address is in wIndex, not wValue. The previous chip-ID-only probe could not detect the swapped fields because address zero yields the same request. Real readback at wIndex 09 returned 012c08 (300 rows), 35 returned 001008 (2x), all four color gains returned 001008, and 0A returned 800008. The old wValue-address form returned chip ID 162108 regardless of address. The encoder and its regression test are corrected. Opening now verifies exposure/gain from the sensor, and failure recovery records register state before changing the setup. A responsive control endpoint during an image-transfer error means that this observation should not be called a complete USB disconnect.

For repeatable diagnosis, `uv run --frozen --extra usb --extra gui python scripts/diagnose_usb_pipe.py --execute --exposure 300 --gain 2` disables automatic retry, retains the last image and sensor state at the first failure, and exits nonzero on failure. This is a diagnostic tool, not evidence that the error is resolved.

## Endpoint-only recovery

macOS kernel logs report endpoint 82 `0xe00002ed (transaction error)` after a partial transfer (196608 bytes in two inspected cases), followed by cancellation of 31 requests initiated by our fatal-error handling. A simple resubmit after status 1 returned LIBUSB_ERROR_PIPE (-9), so blindly continuing the same queue is insufficient.

The successful narrower recovery drains/cancels the failed queue, calls standard clear_halt(82), and creates a fresh queue **without stopping the bridge, resetting the sensor, closing the device, or changing exposure/gain**. In a same-hub 60.056s trial at 300 rows/2x, 756 frames were acquired and all ten image-pipe failures recovered. Nine restarts were about 3–4ms; one took 109ms. This is continuity with frame loss/recovery, not error-free USB operation. The sensor configuration and USB device remain intact.

The backend adopts this endpoint recovery for transaction error status 1 and explicit pipe stalls (status 4 / submit -9) when the control endpoint remains responsive. The receiver queue is prepared before bridge streaming starts, so startup errors can be handled by read() instead of terminating during allocation. Sensor readbacks before and after failed transfers match the requested exposure/gain, so there is no basis to treat ACK alone as calibrated exposure or to assume register corruption. It resynchronizes image assembly at the next short transfer and publishes owned complete frames only. If endpoint recovery fails, consecutive retries do not produce a frame, or the control endpoint stops responding, the existing full-reset/fallback path remains. A burst of endpoint errors is retried within a two-second budget (at most 32 local restarts); after that, at most three full reinitializations are allowed. The first two-retry implementation preserved high settings in a 20-second sweep but fell back during the longer 40-second hold; it was not treated as a successful endurance result. The retry budget was subsequently corrected and regression-tested. Pipe recovery and bus resets are recorded separately. The default low-exposure/low-bandwidth profile remains conservative; high settings are tested independently.

A subsequent longer sweep retained 600 rows/4x through 40 seconds but stopped at 1200 rows/8x on `USB submit failed: -9`. That error was previously omitted from the recoverable-pipe classification. It is now included, with regression tests for error, stall, and submit-pipe cases. Failed sweeps are not recorded as successful acceptance tests.

Final same-hub Qt acceptance sweep after the corrections: 173.905s, 2100 complete frames, five settings (128/1x, 300/2x, 600/4x, 1200/8x, 128/1x), high settings held for 40s each, no profile fallback, no USB device resets, normal stop, and per-stage lossless RGB reconstruction checks. There were 71 endpoint recoveries; this result demonstrates continuity and retained controls, not an error-free bus. Automatic tests: 74 passed.

## Reducing recovery while retaining 10–13 fps

The user explicitly prioritizes smooth live display over a roughly 6fps profile. Further same-hub trials on 2026-10-06 therefore do not change the application clock or receiver defaults.

At 600 rows/4x, short 25–30s trials compared 16–512KiB transfers and 2–64 queued requests. Some initial trials appeared better, but repeating 32KiB/eight requests changed from 20 recoveries/25s to 62/30s. Inserting 20 or 50ms before clear_halt still produced 31–41 recoveries/30s, with roughly 10.5–11fps. This is not reliable evidence of reduced error frequency. One baseline startup failed on submit -9; one other baseline fell back after a bus reset. These failures are retained in the result files rather than excluded as clean runs.

A matched-brightness half-clock trial (8001, 300 rows/4x instead of 8000, 600 rows/4x) produced 153 frames/25.05s and eight recoveries. Its 6.1fps does not meet the requested live-display priority. Clock phase 8101 and quarter clock 8002 were also unsuitable; no experimental clock is adopted.

The more useful alternating comparison at the normal 300 rows/2x setting was:

| Queue | First 40s trial | Second 40s trial | Delivered fps |
| --- | --- | --- | --- |
| Existing 32 × 512KiB | 8 endpoint recoveries / 511 frames | 5 / 516 | 12.75–12.88 |
| Candidate 8 × 512KiB | 7 endpoint recoveries / 513 frames | 12 / 509 | 12.70–12.80 |

All four trials retained the requested sensor settings with zero bus resets. The candidate's apparent first-trial improvement did not repeat; it had 19 recoveries in total against the baseline's 13. Keep the original queue, accurate counters, and bounded endpoint recovery. Reducing a counter or increasing its reporting threshold would not reduce actual failures.

Raw result files: `captures/recovery-matrix/results.json`, `captures/recovery-matrix-refined/results.json`, `captures/recovery-delay/results.json`, and `captures/recovery-normal/results.json`. These were sequential acquisition trials, not native GUI endurance tests. Time/scene variability prevents treating different short trials as a calibrated causal comparison.

For repeatable alternating trials, close the app and run `uv run --frozen --extra usb python scripts/compare_usb_queues.py --execute`. It records delivered fps, longest frame gap, recovery timestamps, separate pipe/reset counts, sensor readbacks, and profile fallback. Candidate settings apply only to that diagnostic process; they do not modify application defaults. It stops reopening after transfer-cleanup failure.
