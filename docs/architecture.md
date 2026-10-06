# Architecture

KohdaLab Camera is a camera toolkit, not a GR300-specific application. Its initial hardware adapter is deliberately bounded to a camera that has been measured and verified.

## Boundaries

1. `api.py` exposes `Camera`, `Frame`, backend registration, construction and export.
2. `backends.py` owns adapter descriptions and factories, simulated acquisition and OS video integration. Optional dependencies and hardware are opened only when the corresponding adapter needs them.
3. `legacy_usb.py`, `legacy_protocol.py` and `usb_stream.py` implement the specific GR300 transport. They do not define the requirements for unrelated cameras.
4. `process_camera.py` / `usb_worker.py` isolate this camera's USB callbacks. The private worker sends owned sensor pixels with capture-time conditions; the receiving capture thread creates RGB. Other adapters can provide RGB directly.
5. `color.py` / `native/camera_render.c` implement equivalent Bayer processing. The original sensor buffer is never corrected, resized or rotated. Missing sensor pixels are not repaired.
6. `gui.py`, `cli.py` and `storage.py` provide common acquisition, presentation and export. GUI controls for the initial legacy device are explicitly adapter-specific.

## Adding a camera

Register a factory and support description through the public API. New registrations automatically appear in CLI choices and GUI backend selection. Implement the common open/read/set_control/close contract and test it with a fake transport before accessing hardware. A factory must not open hardware; `open()` is the boundary for active access.

Controls may retain backend-native units. A readback is not a physical calibration. Record the exact device identity and capture-time settings in each frame. Optional hooks such as interruptible reads, batched controls, white balance and full-sensor capture may be implemented by an adapter; they are not mandatory capabilities of all cameras.

For a new vendor SDK, isolate headers, ABI, architecture, license requirements and lifecycle handling in its adapter. For a new direct USB device, implement identity checks and protocol parsing separately. Registering a backend does not broaden the Windows driver's fixed allowlist.

Automatic entry-point loading, hot-plug selection, a structured capability model and multi-camera sessions are future work. Explicit registration is available now. Frozen apps must be rebuilt with an added adapter and its required binaries; arbitrary installed Python plugins are not automatically included.
