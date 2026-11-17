#!/usr/bin/env python3
"""Verify Camera's common control API against an isolated fake GUI.

This launches the native application in explicit simulation mode, uses only the
shared ``bindings/control`` SDK, and never opens a physical camera.
"""

from __future__ import annotations

import argparse
from dataclasses import replace
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import sys
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "bindings" / "control"))
from kohdalab_control import ControlClient, ControlError, Endpoint  # noqa: E402


def check_config(config: dict) -> None:
    """Legacy top-level fields stay the GUI draft; applied is null until proved."""
    draft = config.get("draft")
    if not isinstance(draft, dict):
        raise RuntimeError("control.config lacks an explicit draft")
    for key in ("mode", "exposure_rows", "gain_eighths"):
        if key not in config or config[key] != draft.get(key):
            raise RuntimeError(f"control.config top-level {key} is not the draft")
    if "applied" not in config or "applied_generation" not in config:
        raise RuntimeError("control.config lacks applied/applied_generation")
    # The fake GUI never streams hardware-checked settings.
    if config["applied"] is not None or config["applied_generation"] is not None:
        raise RuntimeError("Simulation must not report applied camera settings")


def check_wrong_token(endpoint: Endpoint) -> None:
    """A real server refusal must be a terminated record classified as authentication."""
    wrong = replace(endpoint, token=str(uuid.uuid4()))
    try:
        ControlClient(endpoint=wrong).status()
    except ControlError as error:
        if error.code != "authentication" or error.outcome_unknown:
            raise RuntimeError(
                f"Wrong token was classified as {error.code!r}, not authentication"
            ) from error
    else:
        raise RuntimeError("Camera accepted a wrong token")


def check_server_unsupported(client: ControlClient) -> None:
    """Bypass the SDK's describe() pre-check so the running GUI must refuse."""
    request_id = str(uuid.uuid4())
    try:
        # _call sends directly; invoke() would refuse locally before any I/O.
        client._call("capture", request_id=request_id)
    except ControlError as error:
        if (
            error.code != "unsupported"
            or error.application_code != "unsupported"
            or error.request_id != request_id
            or error.outcome_unknown
        ):
            raise RuntimeError(
                f"Server refusal was {error.code!r}/{error.application_code!r}"
            ) from error
    else:
        raise RuntimeError("Camera server unexpectedly accepted capture")


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
            directory = Path(home) / ".kohdalab" / "camera" / "run"
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
            check_config(config)
            check_wrong_token(client.endpoint)
            check_server_unsupported(client)
            # The SDK's own pre-dispatch refusal is a separate, local guard.
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
                "server_unsupported_mutation": "capture",
                "wrong_token": "authentication",
                "config_apply_state": config["apply_state"],
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
