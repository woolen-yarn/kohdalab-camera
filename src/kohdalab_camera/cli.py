import argparse
import json
from .backends import make_camera,available_backends
from .diagnostics import inventory, usb_descriptors, video_probe, mac_video_inventory
from .storage import save_frame


def main():
    parser = argparse.ArgumentParser(description="KohdaLab camera diagnostics and capture")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser('backends',help='List adapter support without opening hardware')
    doctor = sub.add_parser("doctor")
    doctor.add_argument("--usb-descriptors", action="store_true")
    doctor.add_argument("--list-video", action="store_true", help="List macOS AVFoundation identities without capture; requires mac extra")
    doctor.add_argument("--probe-video", action="store_true", help="Opens cameras; may request OS camera permission")
    doctor.add_argument("--limit", type=int, default=5)
    doctor.add_argument("--api", choices=["auto", "avfoundation", "msmf", "dshow"], default="auto")
    capture = sub.add_parser("capture")
    capture.add_argument("--backend", choices=[b.id for b in available_backends()], default="simulated")
    capture.add_argument("--camera-mode",choices=["balanced","full"],default="balanced",help="Legacy USB: 1024x768 low bandwidth or 2048x1536 full sensor")
    capture.add_argument("--index", type=int, default=0)
    capture.add_argument("--api", choices=["auto", "avfoundation", "msmf", "dshow"], default="auto")
    capture.add_argument("--output", default="captures")
    capture.add_argument("--format", choices=["png", "tiff"], default="png")
    args = parser.parse_args()
    try:
        if args.command=='backends':
            print(json.dumps([dict(id=b.id,name=b.name,support=b.support,description=b.description)
                              for b in available_backends()],ensure_ascii=False,indent=2))
            return 0
        if args.command == "doctor":
            if not 1 <= args.limit <= 32:
                parser.error("limit must be 1..32")
            report = inventory()
            if args.list_video:
                try:
                    report["avfoundation_devices"] = mac_video_inventory()
                    report["video_inventory_note"] = "An empty list can reflect visibility or permission limits; compare USB interfaces separately."
                except Exception as exc:
                    report["errors"].append(f"Video inventory unavailable: {exc}")
            if args.usb_descriptors:
                try:
                    report["usb_descriptors"] = usb_descriptors()
                except Exception as exc:
                    report["errors"].append(f"Descriptor inspection unavailable: {exc}")
            if args.probe_video:
                report["video_probe"] = video_probe(args.limit, args.api)
            print(json.dumps(report, ensure_ascii=False, indent=2))
            return 1 if report["errors"] else 0
        camera = make_camera(args.backend, args.index, args.api)
        try:
            if hasattr(camera,"set_capture_profile"):camera.set_capture_profile(args.camera_mode)
            camera.open()
            print(save_frame(camera.read(), args.output, args.format))
        finally:
            camera.close()
        return 0
    except Exception as exc:
        parser.exit(1, f"Error: {exc}\n")


if __name__ == "__main__":
    raise SystemExit(main())
