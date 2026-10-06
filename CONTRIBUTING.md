# Contributing

Use the shared API and backend registry for new camera integrations. Keep SDK and device protocol assumptions inside their adapters. Do not infer compatibility from a related vendor model or OS enumeration alone.

Run `uv sync --frozen --extra gui --extra usb --extra dev` and `uv run pytest -q`. Rendering changes must preserve established pixel hashes and original sensor bytes. Transport changes must preserve exact frame-size checks, cancellation completion and owned buffers. Real-device work requires a separate opt-in validation record; simulated tests cannot certify hardware.

Keep captures, device diagnostics, credentials, vendor installer downloads and build environments out of Git. Portable archives belong in GitHub Releases. Include matching application/native source and third-party notices in every distribution. See docs/architecture.md and docs/portable_release.md.
