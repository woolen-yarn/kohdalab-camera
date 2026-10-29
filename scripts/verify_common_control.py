#!/usr/bin/env python3
"""Verify Camera's common control API against an isolated fake GUI.

This launches the native application in explicit simulation mode, uses only the
shared ``bindings/control`` SDK, and never opens a physical camera.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "bindings" / "control"))
from kohdalab_control import ControlClient, ControlError  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path, nargs="?", default=Path("target/debug/camera-lab"))
    parser.add_argument("--output", type=Path, default=Path(tempfile.gettempdir()) / "kohdalab-camera-common-control.json")
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    log_path = args.output.with_suffix(".log")
    with tempfile.TemporaryDirectory(prefix="kohdalab-camera-common-") as home:
        env = os.environ.copy()
        env["HOME"] = home
        env["USERPROFILE"] = home
        with log_path.open("w", encoding="utf-8") as log:
            process = subprocess.Popen(
                [str(args.binary.resolve()), "--continuous-fake-gui", "--gui-api"],
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
            )
        try:
            directory = Path(home) / ".kohdalab" / "control"
            endpoint = None
            for _ in range(300):
                candidates = list(directory.glob("*.json")) if directory.exists() else []
                if candidates:
                    endpoint = candidates[0]
                    break
                if process.poll() is not None:
                    raise RuntimeError(f"Camera exited before API startup: {process.returncode}")
                time.sleep(0.1)
            if endpoint is None:
                raise RuntimeError("Camera common API descriptor did not appear")
            client = ControlClient(endpoint=endpoint)
            description = client.describe()
            status = client.status()
            config = client.config()
            if description["application"] != "kohdalab-camera":
                raise RuntimeError("Unexpected application identity")
            if description["safety"]["supports_ownership"] is not False:
                raise RuntimeError("Camera must remain read-only for common API")
            if status["cleanup"] == "verified":
                raise RuntimeError("Read-only inspection must not assert cleanup proof")
            if not status or not isinstance(config, dict):
                raise RuntimeError("Invalid status/config response")
            try:
                client.invoke("capture")
            except ControlError as error:
                if error.code != "unsupported":
                    raise
            else:
                raise RuntimeError("Camera unexpectedly accepted capture")
            report = {
                "result": "passed",
                "application": description["application"],
                "methods": description["methods"],
                "unsupported_mutation": "capture",
                "hardware_connected": False,
                "simulation": True,
                "descriptor": str(endpoint),
                "log": str(log_path),
            }
            args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
            print(json.dumps(report, sort_keys=True))
            return 0
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
