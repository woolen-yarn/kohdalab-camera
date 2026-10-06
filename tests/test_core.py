import json
import numpy as np
import pytest
from PIL import Image
from kohdalab_camera.backends import SimulatedCamera, OpenCVCamera, make_camera
from kohdalab_camera.storage import save_frame
from kohdalab_camera.diagnostics import classify, mac_devices, windows_devices


def test_identity_does_not_prove_uvc():
    assert classify(0x0547, 0x4D33, [])['transport'] == 'unknown'
    assert classify(1, 2, [{'class': 14, 'subclass': 1}])['transport'] == 'unknown'
    assert classify(1, 2, [{'class': 14, 'subclass': 1}, {'class': 14, 'subclass': 2}])['transport'] == 'uvc-candidate'
    assert classify(0x0547, 0x4D33, [{'class': 255}])['transport'] == 'vendor-specific-candidate'


def test_os_parsers():
    mac = {'SPUSBDataType': [{'_items': [{'_name': 'camera', 'vendor_id': '0x0547 (Vendor)', 'product_id': '0x4d33'}]}]}
    assert mac_devices(mac)[0]['pid'] == '4D33'
    row = {'Name': 'camera', 'PNPDeviceID': r'USB\VID_0547&PID_4D33\abc', 'Service': 'vendor'}
    assert windows_devices(row)[0]['vid'] == '0547'
    assert windows_devices(None) == []


@pytest.mark.parametrize('format', ['png', 'tiff'])
def test_capture_save_lossless_metadata_and_unique(tmp_path, format):
    cam = SimulatedCamera()
    cam.open()
    cam.set_control('exposure', 12)
    frame = cam.read()
    p1 = save_frame(frame, tmp_path, format)
    p2 = save_frame(frame, tmp_path, format)
    assert p1 != p2
    assert np.array_equal(np.asarray(Image.open(p1)), frame.pixels)
    meta = json.loads(p1.with_suffix(p1.suffix + '.json').read_text())
    assert meta['controls']['exposure'] == 12
    assert meta['color'] == 'RGB'
    cam.close()
    with pytest.raises(RuntimeError):
        cam.read()


@pytest.mark.parametrize('value', [float('nan'), float('inf'), -1, 1001])
def test_invalid_exposure(value):
    with pytest.raises(ValueError):
        SimulatedCamera().set_control('exposure', value)


def test_vendor_backend_factory():
    from kohdalab_camera.legacy_usb import LegacyUsbCamera
    assert isinstance(make_camera('legacy-tca'), LegacyUsbCamera)


def test_opencv_read_failure_release(monkeypatch):
    import cv2
    class FakeCapture:
        released = False
        def isOpened(self): return True
        def read(self): return False, None
        def release(self): self.released = True
        def set(self, prop, value): return False
    cap = FakeCapture()
    monkeypatch.setattr(cv2, 'VideoCapture', lambda *args: cap)
    camera = OpenCVCamera()
    camera.open()
    with pytest.raises(RuntimeError, match='unsupported'):
        camera.set_control('gain', 1)
    with pytest.raises(RuntimeError, match='acquisition failed'):
        camera.read()
    camera.close()
    assert cap.released


def test_save_failure_leaves_no_partial_file(tmp_path, monkeypatch):
    cam = SimulatedCamera()
    cam.open()
    def fail(*args, **kwargs): raise OSError('disk error')
    monkeypatch.setattr(Image.Image, 'save', fail)
    with pytest.raises(OSError):
        save_frame(cam.read(), tmp_path)
    assert list(tmp_path.iterdir()) == []


def test_ioreg_exact_class_and_location():
    from kohdalab_camera.diagnostics import mac_ioreg_devices
    devices = '''+-o Camera  <class IOUSBHostDevice, id 0x1>
      "idVendor" = 1351
      "idProduct" = 19763
      "USB Product Name" = "3.1M USB2.0 Camera"
      "USB Vendor Name" = "DOSENSOR"
      "locationID" = 1441792
      "bDeviceClass" = 0
    +-o ChildDriver <class FakeDriver, id 0x2>
      "idVendor" = 9999
      "idProduct" = 9999
    '''
    interfaces = '''+-o Interface <class IOUSBHostInterface, id 0x3>
      "idVendor" = 0x0547
      "idProduct" = 0x4D33
      "locationID" = 1441792
      "bInterfaceNumber" = 0
      "bInterfaceClass" = 255
      "bInterfaceSubClass" = 0
      "bInterfaceProtocol" = 0
      "bNumEndpoints" = 2
    +-o SameModelElsewhere <class IOUSBHostInterface, id 0x4>
      "idVendor" = 1351
      "idProduct" = 19763
      "locationID" = 2
      "bInterfaceClass" = 14
    '''
    rows = mac_ioreg_devices(devices, interfaces)
    assert len(rows) == 1
    assert rows[0]['manufacturer'] == 'DOSENSOR'
    assert rows[0]['transport'] == 'vendor-specific-candidate'
    assert len(rows[0]['interfaces']) == 1
    assert rows[0]['interfaces'][0]['endpoint_count'] == 2


def test_mac_inventory_profiler_fallback(monkeypatch):
    from kohdalab_camera import diagnostics
    monkeypatch.setattr(diagnostics.platform, 'system', lambda: 'Darwin')
    monkeypatch.setattr(diagnostics, 'mac_registry_inventory', lambda: [])
    monkeypatch.setattr(diagnostics, 'run_json', lambda args: [{'vendor_id': '0x0547', 'product_id': '0x4D33'}])
    assert diagnostics.inventory()['devices'][0]['vid'] == '0547'
