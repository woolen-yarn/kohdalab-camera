"""Experimental volatile driver sequence; one raw frame, never firmware writes."""
from pathlib import Path
import json,time
import usb.core,usb.util,usb.backend.libusb1,libusb_package
from kohdalab_camera.legacy_protocol import parse_register_reply

out=Path('captures/legacy-experimental');out.mkdir(parents=True,exist_ok=True)
backend=usb.backend.libusb1.get_backend(find_library=libusb_package.find_library)
d=usb.core.find(idVendor=0x0547,idProduct=0x4d33,backend=backend)
if d is None:raise SystemExit('Camera not visible to libusb')
trace=[]; claimed=False; chunks=[]
def control(rt,request,value,index,length):
 result=d.ctrl_transfer(rt,request,value,index,length,timeout=2000)
 trace.append({'type':rt,'request':request,'value':value,'index':index,'reply':bytes(result).hex() if rt&128 else result})
 return result

def reg(address,value):
 reply=control(0xc0,0xb,value,address,1)
 if bytes(reply)!=b'\x08':raise RuntimeError(f'Register {address:x} rejected: {bytes(reply).hex()}')

try:
 cfg=d.get_active_configuration()
 interface=cfg[(0,0)]
 if not any(e.bEndpointAddress==0x82 and e.bmAttributes&3==2 for e in interface):
  raise RuntimeError('Expected bulk-IN endpoint 0x82 absent')
 usb.util.claim_interface(d,0);claimed=True
 for value in [1,0,1]:control(0x40,1,value,0xf,b'');time.sleep(.1)
 reg(7,2)
 chip=parse_register_reply(control(0xc0,0xa,0,0,3))
 if chip&0xff00!=0x1600:raise RuntimeError(f'Unexpected sensor family {chip:04x}')
 reg(0xa,0x8000);reg(0xd,1);time.sleep(.1);reg(0xd,0);time.sleep(.1)
 for address,value in [(1,0x15),(2,0x21),(0x20,0),(0x1e,0x8040),(0x4e,0x20),
  (4,0x7ff),(3,0x5ff),(0x2b,0x60),(0x2c,0x460),(0x2d,0x60),(0x2e,0x60),
  (0xa,0x8001),(0x49,0x80),(0x22,0),(0x23,0),(8,0),(9,0x619)]:reg(address,value)
 reg(5,0x150);time.sleep(.4)
 control(0x40,1,3,0xf,b'')
 deadline=time.monotonic()+8
 empty_packets=0
 needed=2048*1536
 while sum(map(len,chunks))<needed:
  data=bytes(d.read(0x82,min(0x4000,needed-sum(map(len,chunks))),timeout=3000))
  if not data:
   empty_packets+=1
   trace.append({'bulk_zlp':empty_packets})
   if time.monotonic()>deadline or empty_packets>32:raise RuntimeError('No image payload after bounded ZLP retry')
   continue
  chunks.append(data)
 raw=b''.join(chunks)
 (out/'frame.bin').write_bytes(raw)
 print(json.dumps({'chip_id':f'{chip:04X}','bytes':len(raw),'chunks':[len(c) for c in chunks],
 'raw_file':str(out/'frame.bin'),'shape_candidate':[1536,2048],'pixel_format':'unconfirmed'},indent=2))
finally:
 for op in [(0x40,1,1,0xf,b''),(0x40,1,0,0xf,b''),(0xc0,0xb,0,7,1)]:
  try:control(*op)
  except Exception as e:trace.append({'cleanup_error':str(e)})
 if claimed:usb.util.release_interface(d,0)
 usb.util.dispose_resources(d)
 (out/'trace.json').write_text(json.dumps(trace,indent=2))
 if chunks:(out/'partial.bin').write_bytes(b''.join(chunks))
 print('payload_received',sum(map(len,chunks)))
