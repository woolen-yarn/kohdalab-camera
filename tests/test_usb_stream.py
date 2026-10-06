import ctypes as C
from types import SimpleNamespace
import time
import pytest
from kohdalab_camera.usb_stream import FrameAssembler, QueuedBulkReader


def test_startup_partial_is_discarded_and_exact_frame_is_delimited():
    a=FrameAssembler(8,4)
    assert a.feed(b'abcd') is None
    assert a.feed(b'x') is None
    assert a.feed(b'1234') is None
    assert a.feed(b'5678')==b'12345678'
    assert a.feed(b'') is None
    assert a.discarded==0


def test_partial_frame_is_discarded_and_optional_zlp_does_not_lose_frames():
    a=FrameAssembler(8,4);a.feed(b'')
    assert a.feed(b'xx') is None
    assert a.discarded==1
    a.feed(b'abcd');assert a.feed(b'efgh')==b'abcdefgh'
    # Consecutive frames need not have a ZLP between them.
    a.feed(b'1234');assert a.feed(b'5678')==b'12345678'
    assert a.feed(b'') is None
    a.feed(b'zzzz');assert a.feed(b'xx') is None
    assert a.discarded==2


class Function:
    def __init__(self, function):self.function=function
    def __call__(self,*args):return self.function(*args)


class Lib:
    def __init__(self, fail_at=None):
        import usb.backend.libusb1 as adapter
        self.adapter=adapter;self.allocated=[];self.pending={};self.cancelled=[];self.freed=[]
        self.submits=0;self.fail_at=fail_at
        for name in ('alloc_transfer','submit_transfer','cancel_transfer','free_transfer','handle_events_timeout'):
            setattr(self,'libusb_'+name,Function(getattr(self,name)))
    def alloc_transfer(self, _):
        p=C.pointer(self.adapter._libusb_transfer());self.allocated.append(p);return p
    def submit_transfer(self,p):
        self.submits+=1
        if self.submits==self.fail_at:return -1
        self.pending[C.addressof(p.contents)]=p;return 0
    def cancel_transfer(self,p):self.cancelled.append(p);return 0
    def free_transfer(self,p):
        key=C.addressof(p.contents)
        assert key not in self.pending, 'Active transfer freed!'
        self.freed.append(key)
    def handle_events_timeout(self,*_):
        if self.cancelled:
            p=self.cancelled.pop(0);self.pending.pop(C.addressof(p.contents))
            p.contents.status=3;p.contents.callback(p)
        else:time.sleep(.001)
        return 0


def reader(lib):
    device=SimpleNamespace(_ctx=SimpleNamespace(backend=SimpleNamespace(lib=lib,ctx=None),
                                               handle=SimpleNamespace(handle=C.c_void_p())))
    return QueuedBulkReader(device,8)


def test_shutdown_waits_for_cancellation_before_freeing_buffers():
    lib=Lib();r=reader(lib);r.start();r.close()
    assert not lib.pending and len(lib.freed)==r.depth
    assert not r.buffers and not r.thread.is_alive()


def test_failed_submission_cancels_prior_transfers_before_cleanup():
    lib=Lib(fail_at=3);r=reader(lib)
    with pytest.raises(RuntimeError,match='submit'):r.start()
    assert not lib.pending and len(lib.freed)==3


def test_active_transfer_blocks_device_release():
    r=reader(Lib());r.thread=SimpleNamespace(join=lambda _:None,is_alive=lambda:False)
    r.active={1}
    with pytest.raises(RuntimeError,match='must remain open'):r.close()


def test_user_stop_interrupts_wait_without_freeing_pending_transfers():
    lib=Lib();r=reader(lib);r.start()
    try:
        with pytest.raises(InterruptedError):r.read(cancelled=lambda:True)
        assert lib.pending and not lib.freed
    finally:r.close()
    assert not lib.pending and len(lib.freed)==r.depth



def test_raw_io_respects_endpoint_packet_and_transfer_limit():
    lib=Lib()
    r=reader(lib)
    r.transfer_bytes=524288  # Exercise a request that exceeds the device limit.
    lib.libusb_endpoint_supports_raw_io=Function(lambda *_:1)
    lib.libusb_get_max_raw_io_transfer_size=Function(lambda *_:65535)
    calls=[]
    lib.libusb_endpoint_set_raw_io=Function(lambda *args:(calls.append(args) or 0))
    r.device.get_active_configuration=lambda:{(0,0):[SimpleNamespace(bEndpointAddress=0x82,wMaxPacketSize=512)]}
    r._configure_winusb_raw_io()
    assert r.winusb_raw_io and r.transfer_bytes==65024
    assert calls==[(r.handle,0x82,1)]
    assert not lib.pending


def test_raw_io_failure_prevents_starting_usb_transfers():
    lib=Lib()
    r=reader(lib)
    lib.libusb_endpoint_supports_raw_io=Function(lambda *_:1)
    lib.libusb_get_max_raw_io_transfer_size=Function(lambda *_:65536)
    lib.libusb_endpoint_set_raw_io=Function(lambda *_:-1)
    r.device.get_active_configuration=lambda:{(0,0):[SimpleNamespace(bEndpointAddress=0x82,wMaxPacketSize=512)]}
    with pytest.raises(RuntimeError,match='RAW_IO enable failed'):
        r._configure_winusb_raw_io()
    assert not lib.pending and not r.winusb_raw_io
