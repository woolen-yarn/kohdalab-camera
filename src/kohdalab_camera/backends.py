from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Protocol
import math
import platform
import time
import re
from typing import Callable

import numpy as np


@dataclass(frozen=True)
class Frame:
    """Owned RGB uint8 image; never a borrowed SDK buffer."""
    pixels: np.ndarray
    sequence: int
    timestamp: str
    backend: str
    device: str
    controls: dict
    sensor_raw: np.ndarray | None = None


@dataclass(frozen=True)
class SensorFrame:
    """Private worker response: owned sensor bytes and their capture conditions."""
    sensor_raw: np.ndarray
    sequence: int
    timestamp: str
    backend: str
    device: str
    controls: dict


class Camera(Protocol):
    def open(self) -> None: ...
    def read(self) -> Frame: ...
    def set_control(self, name: str, value: float) -> dict: ...
    def close(self) -> None: ...


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


class SimulatedCamera:
    def __init__(self, width=640, height=480):
        self.width, self.height = width, height
        self.controls = {"exposure": 10.0, "gain": 1.0}
        self.sequence = 0
        self.opened = False

    def open(self):
        self.opened = True

    def read(self):
        if not self.opened:
            raise RuntimeError("Camera is closed")
        time.sleep(1 / 30)
        self.sequence += 1
        y, x = np.mgrid[:self.height, :self.width]
        brightness = self.controls["exposure"] / 10 * self.controls["gain"]
        spot = np.exp(-((x - (self.width / 2 + 100 * math.sin(self.sequence / 30))) ** 2
                        + (y - self.height / 2) ** 2) / 5000)
        gray = np.clip((20 + spot * 180) * brightness, 0, 255).astype(np.uint8)
        rgb = np.stack([gray, np.roll(gray, 10, axis=1), gray], axis=-1)
        return Frame(rgb, self.sequence, now(), "simulated", "synthetic", dict(self.controls))

    def set_control(self, name, value):
        limits = {"exposure": (0.1, 1000), "gain": (1, 16)}
        if name not in limits or not math.isfinite(value) or not limits[name][0] <= value <= limits[name][1]:
            raise ValueError("Unsupported control or value outside simulated range")
        self.controls[name] = float(value)
        return {"requested": value, "reported": value, "unit": "ms" if name == "exposure" else "factor"}

    def close(self):
        self.opened = False


class OpenCVCamera:
    """OS video device adapter. Being visible here does not prove USB UVC."""
    def __init__(self, index=0, api="auto"):
        self.index, self.api = index, api
        self.cap = None
        self.sequence = 0
        self.controls = {}

    def open(self):
        import cv2
        selected = self.api
        if selected == "auto":
            selected = {"Darwin": "avfoundation", "Windows": "msmf"}.get(platform.system(), "auto")
        apis = {"auto": cv2.CAP_ANY, "avfoundation": cv2.CAP_AVFOUNDATION,
                "msmf": cv2.CAP_MSMF, "dshow": cv2.CAP_DSHOW}
        self.cap = cv2.VideoCapture(self.index, apis[selected])
        if not self.cap.isOpened():
            self.close()
            raise RuntimeError(f"Cannot open OS camera index {self.index} via {selected}")

    def read(self):
        import cv2
        if self.cap is None:
            raise RuntimeError("Camera is closed")
        ok, bgr = self.cap.read()
        if not ok or bgr is None:
            raise RuntimeError("Frame acquisition failed: check connection and camera permission")
        self.sequence += 1
        rgb = cv2.cvtColor(bgr, cv2.COLOR_BGR2RGB)
        return Frame(rgb, self.sequence, now(), self.cap.getBackendName(),
                     f"opencv-index:{self.index}", dict(self.controls))

    def set_control(self, name, value):
        import cv2
        props = {"exposure": cv2.CAP_PROP_EXPOSURE, "gain": cv2.CAP_PROP_GAIN,
                 "auto_exposure": cv2.CAP_PROP_AUTO_EXPOSURE}
        if self.cap is None or name not in props or not math.isfinite(value):
            raise ValueError("Closed camera or unsupported control/value")
        if not self.cap.set(props[name], float(value)):
            raise RuntimeError(f"{name} is unsupported or rejected by this OS backend")
        result = {"requested": value, "reported": self.cap.get(props[name]),
                  "unit": "backend-native", "verified": False}
        # Readback can be stale or unsupported; never label it physical ms/dB.
        self.controls[name] = result
        return result

    def close(self):
        if self.cap is not None:
            self.cap.release()
            self.cap = None


class LegacyTcaCamera:
    """Intentional integration boundary, not an invented TUCAM ABI."""
    def open(self):
        raise RuntimeError("Legacy TCA SDK adapter not implemented. Obtain exact-model SDK headers, "
                           "DLL architecture, license, and driver support first; modern TUCAM compatibility is unconfirmed.")

    def read(self):
        raise RuntimeError("Legacy TCA SDK unavailable")

    def set_control(self, name, value):
        raise RuntimeError("Legacy TCA SDK unavailable")

    def close(self):
        pass


@dataclass(frozen=True)
class BackendSpec:
    """Adapter identity and support evidence; factory construction does not open hardware."""
    id: str
    name: str
    support: str
    description: str
    factory: Callable[[int, str], Camera]


_backends: dict[str, BackendSpec] = {}


def register_backend(spec: BackendSpec) -> None:
    """Explicitly register an adapter before creating the CLI or GUI."""
    if not re.fullmatch(r'[a-z][a-z0-9-]*',spec.id):
        raise ValueError('Backend IDs require lowercase letters, digits and hyphens')
    if spec.id in _backends:
        raise ValueError(f'Backend already registered: {spec.id}')
    if not callable(spec.factory):raise TypeError('Backend factory must be callable')
    _backends[spec.id] = spec


def available_backends() -> tuple[BackendSpec, ...]:
    return tuple(_backends.values())


def _legacy_factory(index, api):
    from .legacy_usb import LegacyUsbCamera
    return LegacyUsbCamera()


register_backend(BackendSpec('simulated','Simulated camera','simulated',
    'Hardware-free acquisition, controls and export.',lambda index,api:SimulatedCamera()))
register_backend(BackendSpec('opencv','OS video camera','generic-adapter',
    'OpenCV/AVFoundation/MSMF/DirectShow; each camera requires hardware verification.',OpenCVCamera))
register_backend(BackendSpec('legacy-tca','SHODENSHA GR300 / TCA3.1MP','hardware-verified',
    'USB 0547:4D33, MT9T001 chip 1621; direct USB on Mac and WinUSB on Windows.',_legacy_factory))


def make_camera(backend="simulated", index=0, api="auto") -> Camera:
    try:spec = _backends[backend]
    except KeyError:raise ValueError(f"Unknown backend: {backend}") from None
    return spec.factory(index, api)
