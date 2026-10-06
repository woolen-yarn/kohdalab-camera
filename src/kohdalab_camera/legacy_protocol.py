"""Experimental GR300/TCA wire candidates recovered from the vendor binaries.

Encoding is statically evidenced, not validated against a USB trace. This module
offers an explicit bridge preparation probe; firmware is never written.
"""
from dataclasses import dataclass
import struct

VID, PID = 0x0547, 0x4D33
IOCTL_VENDOR = 0x222059


@dataclass(frozen=True)
class ControlRequest:
    request_type: int
    request: int
    value: int
    index: int
    length: int
    meaning: str

    def windows_block(self) -> bytes:
        """10-byte driver input. Reserved bytes zeroed instead of uninitialized."""
        if self.request_type != 0xC0:
            raise ValueError('Only vendor/device IN candidates supported')
        return struct.pack('<BBBBBBHH', 1, 2, 0, 0, self.request, 0, self.value, self.index)


def register_read(register=0) -> ControlRequest:
    if not 0 <= register <= 0xFFFF:
        raise ValueError('Register must be 16bit')
    return ControlRequest(0xC0, 0x0A, 0, register, 3, 'validated sensor-register read; address in wIndex')


def register_write_candidate(register, value) -> ControlRequest:
    """IN request with write side effect; encoder only, never executed here."""
    if not (0 <= register <= 0xFFFF and 0 <= value <= 0xFFFF):
        raise ValueError('Register and value must be 16bit')
    return ControlRequest(0xC0, 0x0B, value, register, 1, 'candidate sensor-register write with acknowledgement')


def parse_register_reply(data) -> int:
    data = bytes(data)
    if len(data) != 3 or data[2] != 8:
        raise RuntimeError(f'Unexpected register reply: {data.hex()}')
    return int.from_bytes(data[:2], 'big')


def probe_chip(prepare=False):
    """Read chip ID, optionally applying observed volatile bridge/output setup."""
    import usb.core
    import usb.util
    import usb.backend.libusb1
    try:
        import libusb_package
        backend = usb.backend.libusb1.get_backend(find_library=libusb_package.find_library)
    except ImportError:
        backend = usb.backend.libusb1.get_backend()
    if backend is None:
        raise RuntimeError('libusb backend unavailable')
    device = usb.core.find(idVendor=VID, idProduct=PID, backend=backend)
    if device is None:
        raise RuntimeError('0547:4D33 is not visible to libusb; compare doctor IORegistry inventory. No USB command sent.')
    request = register_read(0)
    prepared = False
    trace = []
    def ctrl(rt, req, value, index, payload):
        result = device.ctrl_transfer(rt, req, value, index, payload, timeout=1000)
        trace.append({"request_type": rt, "request": req, "value": value, "index": index,
                      "reply": bytes(result).hex() if rt & 0x80 else result})
        return result
    try:
        if prepare:
            import time
            prepared = True
            for value in (1, 0, 1):
                ctrl(0x40, 1, value, 0x0F, b"")
                time.sleep(0.1)
            ack = bytes(ctrl(0xC0, 0x0B, 2, 7, 1))
            if ack != b"\x08":
                raise RuntimeError(f"Output preparation rejected: {ack.hex()}")
        data = device.ctrl_transfer(request.request_type, request.request, request.value,
                                    request.index, request.length, timeout=1000)
        chip_id = parse_register_reply(data)
        return {'chip_id': f'{chip_id:04X}', 'expected_family_match': chip_id & 0xFF00 == 0x1600,
                'reply': bytes(data).hex(), 'prepared': prepare, 'preparation_trace': trace, 'note': 'Static protocol candidate; streaming not implemented'}
    finally:
        cleanup_errors = []
        if prepared:
            for args in ((0xC0, 0x0B, 0, 7, 1), (0x40, 1, 1, 0x0F, b""), (0x40, 1, 0, 0x0F, b"")):
                try:
                    ctrl(*args)
                except Exception as exc:
                    cleanup_errors.append(str(exc))
        usb.util.dispose_resources(device)
        if cleanup_errors:
            raise RuntimeError(f"Probe cleanup failed: {cleanup_errors}")
