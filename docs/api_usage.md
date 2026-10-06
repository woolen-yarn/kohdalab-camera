# API and adapter extensions

```python
from kohdalab_camera.api import make_camera, save_frame
camera = make_camera("simulated")
try:
    camera.open()
    frame = camera.read()
    save_frame(frame, "captures", "png")
finally:
    camera.close()
```

`Frame.pixels` is an owned contiguous RGB uint8 array. `sequence`, UTC `timestamp`, `backend`, `device` and capture-time `controls` travel with the image. Optional `sensor_raw` is the original 2D sensor data. Export retains that data in a separate sensor TIFF, and capture conditions in JSON.

Register an adapter before starting the CLI or GUI:

```python
from kohdalab_camera.api import BackendSpec, register_backend
from my_camera_adapter import MyCamera

register_backend(BackendSpec(
    id="my-camera", name="My camera", support="experimental",
    description="Exact model and connection; hardware verification pending.",
    factory=lambda index, api: MyCamera(index=index),
))
```

A `Camera` implements `open()`, `read() -> Frame`, `set_control(name, value) -> dict`, and `close()`. Close must release owned resources on normal exit and after failures. Use the selected adapter's actual control units and acknowledge unsupported controls with a clear error.

List adapters without accessing devices with `available_backends()` or `kohdalab-camera backends`. Factories are lazy. Duplicate backend IDs are rejected; extensions cannot silently replace a built-in driver.

Test a new adapter's acquisition, control metadata, invalid requests, disconnects, timeouts, cleanup and pixel ownership. Then record real-device validation separately. A generic integration or successful simulated test is not evidence that a particular camera works.
