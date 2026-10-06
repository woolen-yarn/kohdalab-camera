from pathlib import Path
import json
import uuid
from PIL import Image
from .backends import Frame


def save_frame(frame: Frame, directory: str | Path, format="png") -> Path:
    """Lossless full-resolution RGB capture with sidecar metadata; unique filenames."""
    if format not in {"png", "tiff"}:
        raise ValueError("Supported formats: png, tiff")
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / f"capture-{uuid.uuid4().hex}.{format}"
    metadata = {"schema_version": 1, "sequence": frame.sequence, "timestamp": frame.timestamp,
                "backend": frame.backend, "device": frame.device, "controls": frame.controls,
                "shape": list(frame.pixels.shape), "dtype": str(frame.pixels.dtype), "color": "RGB",
                "note": "OS video frames may be processed; this is not sensor RAW."}
    sidecar = path.with_suffix(path.suffix + ".json")
    raw_path = path.with_suffix(".sensor.tiff") if frame.sensor_raw is not None else None
    if raw_path is not None:
        metadata["sensor_raw_file"] = raw_path.name
        metadata["sensor_raw_shape"] = list(frame.sensor_raw.shape)
        metadata["note"] = "Original 8bit sensor bytes preserved in companion TIFF; preview processing recorded in controls."
    try:
        with path.open("xb") as stream:
            Image.fromarray(frame.pixels).save(stream, format=format.upper())
        if raw_path is not None:
            with raw_path.open("xb") as stream:
                Image.fromarray(frame.sensor_raw).save(stream, format="TIFF")
        with sidecar.open("x", encoding="utf-8") as stream:
            json.dump(metadata, stream, ensure_ascii=False, indent=2)
    except Exception:
        path.unlink(missing_ok=True)
        sidecar.unlink(missing_ok=True)
        if raw_path is not None:
            raw_path.unlink(missing_ok=True)
        raise
    return path
