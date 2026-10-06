# Roadmap

## Available in 0.1.0

Common acquisition/export API, explicit backend registry, GUI and CLI, simulated and OS-video adapters, an exact-device GR300 USB adapter, native pixel-equivalent rendering, Mac arm64 and Windows x64 portable apps, and recorded hardware evidence.

## Next camera integrations

- Verify additional UVC/OS camera models with identity and control-unit records.
- Add vendor SDK adapters with exact headers, version/architecture checks and transport tests.
- Introduce camera discovery and selection that preserves device identity across reconnects.
- Publish capabilities for controls, pixel formats, trigger modes and supported resolutions.
- Extend worker isolation and recovery per adapter; add multi-camera sessions after lifecycle tests.
- Validate the new Mac timing on the connected legacy camera.

## Distribution and capture workflows

Versioned editable configuration, reproducible build kits, automated portable builds with platform-native helpers, optional recording/trigger workflows and richer acquisition metadata. These are planned capabilities, not present hardware support.
