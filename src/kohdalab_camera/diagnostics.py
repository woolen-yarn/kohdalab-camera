"""Read-only OS inventory plus optional USB descriptor inspection."""
import json
import platform
import re
import subprocess


def classify(vid, pid, interfaces):
    """Identity is a hint; interface descriptors determine UVC candidates."""
    video = any(i.get("class") == 0x0E and i.get("subclass") == 1 for i in interfaces)
    stream = any(i.get("class") == 0x0E and i.get("subclass") == 2 for i in interfaces)
    hint = "TCA-3.0C candidate (OEM identity unconfirmed)" if (vid, pid) == (0x0547, 0x4D33) else None
    return {"identity_hint": hint,
            "transport": "uvc-candidate" if video and stream else
                         "vendor-specific-candidate" if interfaces and any(i.get("class") == 0xFF for i in interfaces) else "unknown",
            "note": "Confirm descriptors, bound driver, and successful video streaming; VID/PID alone is insufficient."}


def run_json(args):
    result = subprocess.run(args, capture_output=True, text=True, encoding="utf-8", check=True, timeout=30)
    return json.loads(result.stdout.lstrip("\ufeff"))


def mac_devices(tree):
    result = []
    def walk(node):
        if isinstance(node, dict):
            if "vendor_id" in node and "product_id" in node:
                def number(value):
                    m = re.search(r"0x[0-9a-fA-F]+", str(value))
                    return int(m.group(), 16) if m else int(value)
                vid, pid = number(node["vendor_id"]), number(node["product_id"])
                result.append({"name": node.get("_name"), "vid": f"{vid:04X}", "pid": f"{pid:04X}",
                               "location": node.get("location_id"), "serial": node.get("serial_num"),
                               **classify(vid, pid, [])})
            for value in node.values():
                walk(value)
        elif isinstance(node, list):
            for value in node:
                walk(value)
    walk(tree)
    return result


def windows_devices(rows):
    result = []
    for row in ([rows] if isinstance(rows, dict) else rows or []):
        match = re.search(r"VID_([0-9A-F]{4})&PID_([0-9A-F]{4})", row.get("PNPDeviceID", ""), re.I)
        if match:
            vid, pid = (int(v, 16) for v in match.groups())
            result.append({**row, "vid": f"{vid:04X}", "pid": f"{pid:04X}", **classify(vid, pid, [])})
    return result


def ioreg_records(text, class_name):
    """Parse only scalar properties of exact-class nodes, excluding child drivers."""
    records = []
    current = None
    for line in text.splitlines():
        header = re.search(r"\+-o .*<class ([^,]+),", line)
        if header:
            current = {} if header.group(1) == class_name else None
            if current is not None:
                records.append(current)
        elif current is not None:
            match = re.search(r'"([^"\n]+)"\s*=\s*("[^"\n]*"|0x[0-9a-fA-F]+|[0-9]+)\s*$', line)
            if match:
                key, value = match.groups()
                current[key] = value[1:-1] if value.startswith('"') else int(value, 16 if value.startswith('0x') else 10)
    return records


def mac_ioreg_devices(device_text, interface_text):
    interfaces = ioreg_records(interface_text, "IOUSBHostInterface")
    devices = []
    for record in ioreg_records(device_text, "IOUSBHostDevice"):
        if "idVendor" not in record or "idProduct" not in record:
            continue
        vid, pid = record["idVendor"], record["idProduct"]
        matched = [i for i in interfaces if
                   (i.get("idVendor"), i.get("idProduct"), i.get("locationID")) ==
                   (vid, pid, record.get("locationID"))]
        details = [{"number": i.get("bInterfaceNumber"), "class": i.get("bInterfaceClass"),
                    "subclass": i.get("bInterfaceSubClass"), "protocol": i.get("bInterfaceProtocol"),
                    "alternate": i.get("bAlternateSetting"), "endpoint_count": i.get("bNumEndpoints")}
                   for i in matched]
        devices.append({"name": record.get("USB Product Name"), "manufacturer": record.get("USB Vendor Name"),
                        "vid": f"{vid:04X}", "pid": f"{pid:04X}", "location": record.get("locationID"),
                        "device_class": record.get("bDeviceClass"), "interfaces": details,
                        "source": "ioreg", **classify(vid, pid, details)})
    return devices


def mac_registry_inventory():
    def read(class_name):
        return subprocess.run(["ioreg", "-r", "-c", class_name, "-l", "-w", "0"],
                              capture_output=True, text=True, encoding="utf-8", check=True, timeout=30).stdout
    return mac_ioreg_devices(read("IOUSBHostDevice"), read("IOUSBHostInterface"))


def mac_video_inventory():
    """AVFoundation identities without opening a capture stream (optional mac extra)."""
    if platform.system() != "Darwin":
        raise RuntimeError("AVFoundation inventory requires macOS")
    import AVFoundation as av
    # Broad compatibility API; unlike VideoCapture index probing this does not
    # start acquisition. An empty list can reflect visibility/permission limits.
    return [{"name": str(d.localizedName()), "unique_id": str(d.uniqueID()),
             "model": str(d.modelID())}
            for d in av.AVCaptureDevice.devicesWithMediaType_(av.AVMediaTypeVideo)]


def inventory():
    os_name = platform.system()
    report = {"platform": platform.platform(), "devices": [], "errors": []}
    try:
        if os_name == "Darwin":
            # Registry includes interface classes and avoids empty system_profiler
            # inventories observed on current macOS. Retain the profiler fallback.
            try:
                report["devices"] = mac_registry_inventory()
            except (OSError, ValueError, subprocess.SubprocessError) as exc:
                report["errors"].append(f"IORegistry unavailable: {exc}")
            if not report["devices"]:
                report["devices"] = mac_devices(run_json(["system_profiler", "SPUSBDataType", "-json"]))
        elif os_name == "Windows":
            script = ("[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new(); "
                      "@(Get-CimInstance Win32_PnPEntity | Where-Object { $_.PNPDeviceID -match 'VID_[0-9A-F]{4}&PID_' } "
                      "| Select-Object Name,PNPDeviceID,Service,Status,ConfigManagerErrorCode) | ConvertTo-Json -Depth 4")
            report["devices"] = windows_devices(run_json(["powershell.exe", "-NoProfile", "-Command", script]))
        else:
            report["errors"].append("Native USB inventory implemented for macOS and Windows only")
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        report["errors"].append(str(exc))
    return report


def usb_descriptors():
    """Requires PyUSB and libusb. Never sets configuration or detaches drivers."""
    import usb.core
    import usb.util
    devices = []
    for dev in usb.core.find(find_all=True):
        item = {"vid": f"{dev.idVendor:04X}", "pid": f"{dev.idProduct:04X}", "interfaces": [], "errors": []}
        try:
            for cfg in dev:
                for interface in cfg:
                    item["interfaces"].append({"configuration": cfg.bConfigurationValue,
                        "number": interface.bInterfaceNumber, "alternate": interface.bAlternateSetting,
                        "class": interface.bInterfaceClass, "subclass": interface.bInterfaceSubClass,
                        "protocol": interface.bInterfaceProtocol,
                        "endpoints": [{"address": ep.bEndpointAddress, "attributes": ep.bmAttributes,
                                       "max_packet": ep.wMaxPacketSize} for ep in interface]})
        except usb.core.USBError as exc:
            item["errors"].append(str(exc))
        finally:
            usb.util.dispose_resources(dev)
        item.update(classify(dev.idVendor, dev.idProduct, item["interfaces"]))
        devices.append(item)
    return devices


def video_probe(limit=5, api="auto"):
    from .backends import OpenCVCamera
    results = []
    for index in range(limit):
        camera = OpenCVCamera(index, api)
        try:
            camera.open()
            frame = camera.read()
            results.append({"index": index, "backend": frame.backend, "shape": list(frame.pixels.shape),
                            "note": "Index is transient and is not mapped to a USB VID/PID."})
        except RuntimeError as exc:
            results.append({"index": index, "error": str(exc)})
        finally:
            camera.close()
    return results
