import json
from types import SimpleNamespace
from kohdalab_camera.usb_stream import FrameAssembler
import numpy as np
import pytest
from PIL import Image
from kohdalab_camera.legacy_usb import LegacyUsbCamera
from kohdalab_camera.storage import save_frame


class Device:
    def __init__(self, chunks): self.chunks=iter(chunks);self.calls=[]
    def read(self,*args,**kw):return next(self.chunks)
    def ctrl_transfer(self,*args,**kw):self.calls.append(args);return b'\x08'


def camera(chunks):
    cam=LegacyUsbCamera();cam.width=4;cam.height=2;cam.chip=0x1621;cam.device=Device(chunks)
    class Reader:
        depth=32;transfer_bytes=524288;winusb_raw_io=False
        assembler=SimpleNamespace(discarded=0)
        dropped=0
        sequence=0
        def read(self,**kwargs):
            data=cam.device.read()
            while not data:data=cam.device.read()
            self.sequence+=1
            return self.sequence,'2026-10-06T00:00:00+00:00',data
    cam.reader=Reader() if chunks else None
    # Fake transport lifecycle; real open/close use libusb resources.
    cam.open=lambda:None
    cam.close=lambda:None
    return cam


def test_frames_owned_and_lossless_raw_save(tmp_path):
    cam=camera([b'',bytes(range(8)),bytes(range(8,16))])
    first=cam.read();second=cam.read()
    assert first.sequence==1 and second.sequence==2
    assert first.sensor_raw[0,0]==0 and second.sensor_raw[0,0]==8
    path=save_frame(first,tmp_path)
    meta=json.loads(path.with_suffix('.png.json').read_text())
    assert np.array_equal(np.asarray(Image.open(tmp_path/meta['sensor_raw_file'])),first.sensor_raw)
    assert meta['controls']['chip_id']=='1621'


def test_partial_frame_boundary_rejected():
    assembler=FrameAssembler(8,4)
    assembler.feed(b'')
    assert assembler.feed(b'\x01\x02') is None
    assert assembler.discarded==1


def test_write_value_and_address_not_swapped():
    cam=camera([])
    cam.set_control('exposure',100)
    assert cam.device.calls[:2]==[(0xc0,0xb,0,8,1),(0xc0,0xb,100,9,1)]
    cam.set_control('gain',16)
    assert cam.device.calls[-1]==(0xc0,0xb,16,0x35,1)
    with pytest.raises(ValueError):cam.set_control('auto_exposure',1)


def test_failed_write_does_not_update_metadata():
    cam=camera([])
    cam.device.ctrl_transfer=lambda *args,**kw:b'\x07'
    with pytest.raises(RuntimeError):cam.set_control('gain',16)
    assert cam.controls['gain']['reported']==8


def test_frame_controls_owned_after_changes():
    cam=camera([bytes(range(8))])
    frame=cam.read()
    cam.controls['gain']['reported']=32
    assert frame.controls['gain']['reported']==8


def test_invalid_batch_validated_before_any_usb_write():
    cam=camera([])
    with pytest.raises(ValueError):cam.set_controls({'exposure':100,'gain':33})
    assert cam.device.calls==[]
    with pytest.raises(ValueError):cam.set_control('exposure',float('inf'))


def test_live_controls_cancel_before_reinitialization():
    cam=camera([bytes(range(8))]);events=[]
    cam.close=lambda:events.append('close')
    cam.open=lambda:events.append('open')
    cam.set_controls({'exposure':256,'gain':32})
    assert events==['close','open']
    assert cam.controls['exposure']['requested']==256


def test_failed_reinitialization_restores_previous_profile():
    cam=camera([bytes(range(8))]);calls=[]
    def reopen():
        calls.append(1)
        if len(calls)==1:raise RuntimeError('unplugged')
    cam.open=reopen
    with pytest.raises(RuntimeError,match='unplugged'):cam.set_controls({'exposure':600,'gain':32})
    assert len(calls)==2
    assert cam.controls['exposure']['reported']==128


def test_initial_controls_validate_without_usb_access():
    cam=LegacyUsbCamera()
    cam.configure_controls({'exposure':256,'gain':16})
    assert cam.controls['exposure']['requested']==256
    assert not cam.controls['gain']['acknowledged']
    with pytest.raises(ValueError):cam.configure_controls({'exposure':100,'gain':float('inf')})
    assert cam.controls['exposure']['requested']==256


def test_transient_usb_error_reinitializes_and_reports_recovery(monkeypatch):
    cam=camera([bytes(range(8))]);original=cam.reader.read;calls=[]
    def read(**kwargs):
        calls.append(1)
        if len(calls)==1:raise RuntimeError('USB async transfer failed: status=1, bytes=0')
        return original(**kwargs)
    cam.reader.read=read
    lifecycle=[];cam.close=lambda:lifecycle.append('close');cam.open=lambda:lifecycle.append('open')
    monkeypatch.setattr('kohdalab_camera.legacy_usb.time.sleep',lambda _:None)
    frame=cam.read()
    assert lifecycle==['close','open']
    assert frame.controls['usb_recoveries']==1
    assert frame.controls['last_transport_error'].startswith('USB async')
    assert np.array_equal(frame.sensor_raw,np.arange(8).reshape(2,4))


def test_white_reference_balances_colors_without_changing_sensor_bytes():
    raw=np.empty((96,96),np.uint8)
    raw[::2,::2]=80;raw[1::2,1::2]=80;raw[::2,1::2]=100;raw[1::2,::2]=40
    original=raw.copy();cam=LegacyUsbCamera();cam.balance_white(raw)
    assert np.allclose(cam.white_balance,[.8,1,2])
    assert np.array_equal(raw,original)


def test_stalled_bright_profile_returns_to_safe_settings(monkeypatch):
    cam=camera([bytes(range(8))]);cam.controls['exposure']={'requested':600};cam.controls['gain']={'requested':16}
    original=cam.reader.read;calls=[]
    def read(**kwargs):
        calls.append(1)
        if len(calls)==1:raise RuntimeError('USB async transfer failed: status=1, bytes=0')
        return original(**kwargs)
    cam.reader.read=read
    monkeypatch.setattr('kohdalab_camera.legacy_usb.time.sleep',lambda _:None)
    frame=cam.read()
    assert frame.controls['exposure']['requested']==128
    assert frame.controls['gain']['requested']==8
    assert frame.controls['rejected_hardware_profile']['exposure']['requested']==600


def test_recovery_resets_usb_after_close_before_open(monkeypatch):
    cam=camera([bytes(range(8))]);read=cam.reader.read;calls=[];lifecycle=[]
    def transient(**kwargs):
        calls.append(1)
        if len(calls)==1:raise RuntimeError('USB async transfer failed: status=1, bytes=0')
        return read(**kwargs)
    cam.reader.read=transient
    cam.close=lambda:lifecycle.append('close')
    cam.device.reset=lambda:lifecycle.append('reset')
    cam.open=lambda:lifecycle.append('open')
    monkeypatch.setattr('usb.util.dispose_resources',lambda _:lifecycle.append('dispose'))
    monkeypatch.setattr('kohdalab_camera.legacy_usb.time.sleep',lambda _:None)
    frame=cam.read()
    assert lifecycle==['close','reset','dispose','open']
    assert frame.controls['usb_bus_resets']==1


@pytest.mark.parametrize('platform,blanking',[('darwin',3),('win32',3)])
def test_camera_profiles_preserve_field_and_choose_output_size(monkeypatch,platform,blanking):
    monkeypatch.setattr('sys.platform',platform)
    monkeypatch.delenv('KOHDA_CAMERA_VBLANK',raising=False)
    monkeypatch.delenv('KOHDA_CAMERA_HBLANK',raising=False)
    cam=LegacyUsbCamera()
    cam.set_capture_profile('balanced')
    assert (cam.width,cam.height,cam.binning,cam.vertical_blanking)==(1024,768,2,blanking)
    assert cam.horizontal_blanking==(21 if platform=='win32' else 336)
    cam.set_capture_profile('full')
    assert (cam.width,cam.height,cam.binning,cam.vertical_blanking)==(2048,1536,1,3 if platform=='win32' else 1023)
    assert cam.horizontal_blanking==(1024 if platform=='win32' else 336)
    cam.set_capture_profile('balanced')
    assert cam.vertical_blanking==blanking
    with pytest.raises(ValueError):cam.set_capture_profile('unknown')
    cam.device=object()
    with pytest.raises(RuntimeError):cam.set_capture_profile('balanced')


def test_blanking_override_survives_profile_selection(monkeypatch):
    monkeypatch.setenv('KOHDA_CAMERA_VBLANK','25')
    cam=LegacyUsbCamera();cam.set_capture_profile('balanced')
    assert cam.vertical_blanking==25
    monkeypatch.setenv('KOHDA_CAMERA_VBLANK','2')
    with pytest.raises(ValueError,match='3..1023'):LegacyUsbCamera()


def test_full_sensor_capture_restores_live_profile_on_success_and_failure():
    cam=LegacyUsbCamera();events=[];cam.device=object()
    def close():events.append('close');cam.device=None
    def opened():events.append(('open',cam.capture_profile));cam.device=object()
    cam.close=close;cam.open=opened
    def captured(*args,**kw):
        assert (cam.width,cam.height)==(2048,1536)
        assert kw['recover'] is False
        return 'photo'
    cam.read=captured
    assert cam.capture_full_frame()=='photo'
    assert (cam.width,cam.height)==(1024,768)
    assert events==['close',('open','full'),'close',('open','balanced')]
    def failed(*args,**kw):raise RuntimeError('USB failed')
    cam.read=failed
    with pytest.raises(RuntimeError):cam.capture_full_frame()
    assert cam.device is not None and cam.capture_profile=='balanced'


def test_sensor_readback_uses_index_and_rejects_bad_ack():
    cam=camera([])
    cam.device.ctrl_transfer=lambda *args,**kw: (cam.device.calls.append(args) or bytes.fromhex('012c08'))
    assert cam.read_register(9)==300
    assert cam.device.calls[-1]==(0xc0,0x0a,0,9,3)
    cam.device.ctrl_transfer=lambda *args,**kw:bytes.fromhex('012c07')
    with pytest.raises(RuntimeError):cam.read_register(9)


def test_transport_diagnostics_preserve_control_failure():
    cam=camera([]);cam.device.ctrl_transfer=lambda *args,**kw:bytes.fromhex('000208')
    state=cam.capture_transport_state()
    assert state['control_endpoint_responsive'] and state['registers']['07']=='0002'
    cam.device.ctrl_transfer=lambda *args,**kw:bytes.fromhex('000007')
    assert 'control_error' in cam.capture_transport_state()


def test_pipe_recovery_drains_before_clear_and_preserves_sensor(monkeypatch):
    cam=camera([bytes(range(8))]);events=[]
    cam.controls['exposure']={'requested':600};cam.controls['gain']={'requested':32}
    original=cam.reader.read;cam.reader.close=lambda:events.append('drain')
    cam.device.clear_halt=lambda endpoint:events.append(('clear',endpoint))
    class Restarted:
        depth=32;transfer_bytes=524288;winusb_raw_io=False
        assembler=SimpleNamespace(discarded=0);dropped=0
        def __init__(self,device,size):events.append(('allocate',size))
        def start(self):events.append('start')
        def read(self,**kwargs):return original(**kwargs)
    monkeypatch.setattr('kohdalab_camera.usb_stream.QueuedBulkReader',Restarted)
    cam.reader.read=lambda **kw: (_ for _ in ()).throw(RuntimeError('USB async transfer failed: status=1, bytes=0'))
    cam.capture_transport_state=lambda:{'control_endpoint_responsive':True}
    frame=cam.read()
    assert events==['drain',('clear',0x82),('allocate',8),'start']
    assert frame.controls['exposure']['requested']==600 and frame.controls['gain']['requested']==32
    assert frame.controls['usb_pipe_recoveries']==1 and frame.controls['usb_bus_resets']==0
    assert 'rejected_hardware_profile' not in frame.controls


@pytest.mark.parametrize('error',['USB async transfer failed: status=1, bytes=0','USB async transfer failed: status=4, bytes=0','USB submit failed: -9'])
def test_burst_of_pipe_errors_keeps_controls_until_time_or_count_limit(monkeypatch,error):
    cam=camera([bytes(range(8))]);cam.controls['exposure']={'requested':600};cam.controls['gain']={'requested':32}
    read=cam.reader.read;calls=[];restarts=[]
    def transient(**kwargs):
        calls.append(1)
        if len(calls)<=3:raise RuntimeError(error)
        return read(**kwargs)
    cam.reader.read=transient
    cam.capture_transport_state=lambda:{'control_endpoint_responsive':True}
    cam.restart_image_pipe=lambda:(restarts.append(1) or True)
    frame=cam.read()
    assert len(restarts)==3 and frame.controls['exposure']['requested']==600
    assert frame.controls['gain']['requested']==32 and cam.bus_resets==0


def test_repeated_pipe_failure_has_bounded_retry_then_full_fallback(monkeypatch):
    cam=camera([bytes(range(8))]);cam.controls['exposure']={'requested':600};cam.controls['gain']={'requested':32}
    read=cam.reader.read;calls=[];pipes=[];lifecycle=[]
    def transient(**kwargs):
        calls.append(1)
        if len(calls)<=33:raise RuntimeError('USB async transfer failed: status=1, bytes=0')
        return read(**kwargs)
    cam.reader.read=transient;cam.capture_transport_state=lambda:{'control_endpoint_responsive':True}
    cam.restart_image_pipe=lambda:(pipes.append(1) or True)
    cam.close=lambda:lifecycle.append('close');cam.open=lambda:lifecycle.append('open')
    monkeypatch.setattr('kohdalab_camera.legacy_usb.time.sleep',lambda _:None)
    frame=cam.read()
    assert len(pipes)==32 and lifecycle==['close','open']
    assert frame.controls['exposure']['requested']==128 and frame.controls['gain']['requested']==8
