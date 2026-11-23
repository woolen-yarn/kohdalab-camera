#!/usr/bin/env python3
"""Verify Camera's common control API against an isolated fake GUI.

This launches the native application in explicit simulation mode, uses only the
shared ``bindings/control`` SDK, and never opens a physical camera. It checks
reads, lease admission, start_live/stop, current-ROI capture with a
non-clobbering output path, duplicate request IDs, release and heartbeat expiry.
Full-sensor capture needs acquisition metadata that Simulation does not
provide, so here it must be refused before any acquisition change.
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


IMPLEMENTED = (
    "control.describe",
    "control.status",
    "control.config",
    "control.result",
    "control.acquire",
    "control.heartbeat",
    "control.release",
    "control.start_live",
    "control.stop",
    "control.capture",
    "control.capture_full",
)
# Recording has no reusable Camera lifecycle and is deliberately not exposed.
UNSUPPORTED = "record"


def check_server_unsupported(client: ControlClient) -> None:
    """Bypass the SDK's describe() pre-check so the running GUI must refuse."""
    request_id = str(uuid.uuid4())
    try:
        # _call sends directly; invoke() would refuse locally before any I/O.
        client._call(UNSUPPORTED, request_id=request_id)
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
        raise RuntimeError(f"Camera server unexpectedly accepted {UNSUPPORTED}")


def expect_refusal(code: str, call, *args, **kwargs) -> ControlError:
    """A definite refusal: classified, and not an ambiguous outcome."""
    try:
        call(*args, **kwargs)
    except ControlError as error:
        if error.code != code or error.outcome_unknown:
            raise RuntimeError(
                f"Expected {code!r} refusal, got {error.code!r}/{error.application_code!r}: {error}"
            ) from error
        return error
    raise RuntimeError(f"Expected {code!r} refusal, but the request was accepted")


def capture_when_displayed(app: ControlClient, path: Path) -> dict:
    """Retry only definite `busy` refusals (never executed) with new request IDs."""
    for _ in range(100):
        try:
            return app.invoke("capture", path=str(path))
        except ControlError as error:
            if error.code != "busy" or error.outcome_unknown:
                raise
        time.sleep(0.1)
    raise RuntimeError("Live frame was never displayed for capture")


def check_mutations(endpoint: Endpoint, output: Path) -> dict:
    """Exercise the implemented GUI mutations through the unchanged SDK."""
    report = {}
    output.mkdir(parents=True, exist_ok=False)
    observer = ControlClient(endpoint=endpoint)
    # Mutations need a lease; the refusal happens before any action.
    expect_refusal("ownership", observer._call, "start_live", source="simulation")
    with ControlClient(endpoint=endpoint) as app:
        expect_refusal("ownership", ControlClient(endpoint=endpoint).acquire)
        status = app.status()
        if status["ownership"].get("owned") is not True:
            raise RuntimeError("Acquired lease is not reported as owned")
        # The window's source selection is authoritative; no API selection bypass.
        expect_refusal("validation", app.invoke, "start_live", source="usb")
        live = app.wait(app.invoke("start_live", source="simulation")["job_id"], timeout=30)
        if live["status"] != "completed" or live["kind"] != "start_live":
            raise RuntimeError(f"start_live did not complete: {live}")
        # Current-ROI capture through the window's save pipeline.
        roi = output / "roi.png"
        job = capture_when_displayed(app, roi)
        done = app.wait(job["job_id"], timeout=30)
        if done["native"]["saved"] is not True or not roi.is_file():
            raise RuntimeError(f"Capture receipt lacks persistence evidence: {done}")
        # The same request ID is one decision: no second export.
        request_id = str(uuid.uuid4())
        again = output / "again.png"
        first = app.invoke("capture", request_id=request_id, path=str(again))
        duplicate = app.invoke("capture", request_id=request_id, path=str(again))
        if duplicate["job_id"] != first["job_id"]:
            raise RuntimeError("Duplicate request ID started a second capture")
        expect_refusal(
            "request_id_conflict", app.invoke, "capture",
            request_id=request_id, path=str(output / "other.png"),
        )
        app.wait(first["job_id"], timeout=30)
        if (output / "other.png").exists():
            raise RuntimeError("Conflicting request ID was executed")
        # Explicit non-clobber output path.
        expect_refusal("storage", app.invoke, "capture", path=str(roi))
        # Simulation has no acquisition metadata: Full is refused, Live untouched.
        expect_refusal("busy", app.invoke, "capture_full", path=str(output / "full.tiff"))
        if (output / "full.tiff").exists() or app.status()["phase"] != "running":
            raise RuntimeError("Refused Full capture changed output or acquisition")
        stopped = app.stop_and_wait(timeout=20)
        report["stop"] = {"phase": stopped["phase"], "cleanup": stopped["cleanup"]}
        app.wait(app.invoke("start_live", source="simulation")["job_id"], timeout=30)
    # Context exit released the lease; release stops via the window's Stop.
    after = observer.status()
    if after["ownership"].get("owned") is not False or after["busy"] is not False:
        raise RuntimeError(f"Release did not return an idle, unowned GUI: {after}")
    report["release"] = {"cleanup": after["cleanup"]}
    report["outputs"] = sorted(p.name for p in output.iterdir())
    return report


def check_expiry(endpoint: Endpoint) -> dict:
    """Without heartbeats the GUI revokes the lease and requests its own Stop."""
    client = ControlClient(endpoint=endpoint)
    # _call skips the SDK heartbeat thread so the lease is left to expire.
    granted = client._call("acquire")
    client._lease = granted["lease"]
    started = client._call("start_live", source="simulation")
    client.wait(started["job_id"], timeout=30)
    time.sleep(granted["heartbeat_timeout_seconds"] + 1.5)
    error = expect_refusal("ownership", client._call, "heartbeat")
    if error.application_code != "revoked":
        raise RuntimeError(f"Late heartbeat was {error.application_code!r}, not revoked")
    deadline = time.monotonic() + 20
    while True:
        state = client.status()
        if state["busy"] is False and state["ownership"].get("owned") is False:
            break
        if time.monotonic() >= deadline:
            raise RuntimeError(f"Expired lease did not stop acquisition: {state}")
        time.sleep(0.1)
    if state["ownership"].get("last_revocation") != "expired":
        raise RuntimeError("Status does not report lease expiry")
    return {"late_heartbeat": error.application_code, "cleanup": state["cleanup"]}


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
            if sorted(description["methods"]) != sorted(IMPLEMENTED):
                raise RuntimeError(f"Unexpected advertised methods: {description['methods']}")
            safety = description["safety"]
            if safety["supports_ownership"] is not True or safety["stop_requires_lease"] is not True:
                raise RuntimeError("Camera must advertise its exclusive lease")
            if safety["physical_auto_connect"] is not False:
                raise RuntimeError("Camera must not connect physical devices automatically")
            if status["cleanup"] == "verified":
                raise RuntimeError("Read-only inspection must not assert cleanup proof")
            if status["ownership"].get("owned") is not False:
                raise RuntimeError("A fresh GUI must not report an external owner")
            if not status or not isinstance(config, dict):
                raise RuntimeError("Invalid status/config response")
            check_config(config)
            check_wrong_token(client.endpoint)
            check_server_unsupported(client)
            # The SDK's own pre-dispatch refusal is a separate, local guard.
            try:
                client.invoke(UNSUPPORTED)
            except ControlError as error:
                if error.code != "unsupported":
                    raise
            else:
                raise RuntimeError(f"Camera unexpectedly accepted {UNSUPPORTED}")
            mutations = check_mutations(
                client.endpoint, Path(home) / "camera-control-output"
            )
            expiry = check_expiry(client.endpoint)
            report = {
                "result": "passed",
                "application": description["application"],
                "methods": description["methods"],
                "unsupported_mutation": UNSUPPORTED,
                "server_unsupported_mutation": UNSUPPORTED,
                "wrong_token": "authentication",
                "mutations": mutations,
                "lease_expiry": expiry,
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
