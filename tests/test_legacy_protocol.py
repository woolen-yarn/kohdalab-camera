import pytest
from kohdalab_camera.legacy_protocol import register_read,register_write_candidate,parse_register_reply


def test_recovered_read_packet_and_endianness():
    request=register_read(0x1234)
    assert request.windows_block()==bytes.fromhex('010200000a0000003412')
    assert (request.request_type,request.length)==(0xC0,3)
    assert parse_register_reply(bytes.fromhex('162108'))==0x1621


def test_write_is_in_request_with_side_effect():
    request=register_write_candidate(7,2)
    assert request.windows_block()==bytes.fromhex('010200000b0002000700')
    assert request.request_type==0xC0 and request.length==1


@pytest.mark.parametrize('reply',[b'',b'\x16\x21',b'\x16\x21\x00',b'\x16\x21\x08\x00'])
def test_bad_ack_rejected(reply):
    with pytest.raises(RuntimeError):parse_register_reply(reply)


@pytest.mark.parametrize('value',[-1,65536])
def test_register_range(value):
    with pytest.raises(ValueError):register_read(value)
    with pytest.raises(ValueError):register_write_candidate(7,value)


@pytest.mark.parametrize('reply,success',[(bytes.fromhex('162108'),True),(bytes.fromhex('000007'),False)])
def test_prepared_probe_cleanup(monkeypatch,reply,success):
    pytest.importorskip('usb.core')
    pytest.importorskip('libusb_package')
    import usb.core,usb.util,usb.backend.libusb1,libusb_package,time
    from kohdalab_camera.legacy_protocol import probe_chip
    calls=[]
    class Device:
        def ctrl_transfer(self,rt,request,value,index,payload,timeout):
            calls.append((rt,request,value,index,payload))
            return reply if request==0x0a else b'\x08' if rt==0xc0 else 0
    device=Device()
    monkeypatch.setattr(usb.backend.libusb1,'get_backend',lambda **kw: object())
    monkeypatch.setattr(usb.core,'find',lambda **kw: device)
    monkeypatch.setattr(usb.util,'dispose_resources',lambda d: None)
    monkeypatch.setattr(time,'sleep',lambda t: None)
    if success:
        result=probe_chip(prepare=True)
        assert result['chip_id']=='1621'
    else:
        with pytest.raises(RuntimeError,match='Unexpected register reply'):
            probe_chip(prepare=True)
    assert calls[:3]==[(0x40,1,v,15,b'') for v in (1,0,1)]
    assert calls[-3:]==[(0xc0,0xb,0,7,1),(0x40,1,1,15,b''),(0x40,1,0,15,b'')]
