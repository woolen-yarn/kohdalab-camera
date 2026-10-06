"""Explicit real-device validation, including the Windows private worker pipe."""
import argparse, json, time
from pathlib import Path
import numpy as np
from PIL import Image
from kohdalab_camera.process_camera import ProcessCamera
from kohdalab_camera.storage import save_frame

parser=argparse.ArgumentParser()
parser.add_argument('--execute',action='store_true')
parser.add_argument('--seconds',type=float,default=30)
parser.add_argument('--output',default='captures/windows-validation')
args=parser.parse_args()
if not args.execute:
    parser.error('--execute is required for real USB capture and control changes')
if not 1 <= args.seconds <= 300:
    parser.error('--seconds must be 1..300')
out=Path(args.output);out.mkdir(parents=True,exist_ok=True)
camera=ProcessCamera();frames=0;result={'success':False}
try:
    camera.open()
    start=time.monotonic()
    first=camera.read();frames+=1
    assert first.controls['chip_id']=='1621'
    assert first.pixels.shape==(768,1024,3)
    for fmt in ('png','tiff'):
        path=save_frame(first,out,fmt)
        assert np.array_equal(np.asarray(Image.open(path)),first.pixels)
        assert np.array_equal(np.asarray(Image.open(path.with_suffix('.sensor.tiff'))),first.sensor_raw)
    camera.set_controls({'exposure':256,'gain':16})
    adjusted=camera.read();frames+=1
    assert adjusted.controls['exposure']['readback']==256
    assert adjusted.controls['gain']['readback']==16
    camera.set_controls({'exposure':128,'gain':8})
    last=adjusted
    while time.monotonic()-start < args.seconds:
        last=camera.read();frames+=1
    process=camera.process
    camera.close()
    assert process.poll()==0
    result={'success':True,'elapsed_seconds':time.monotonic()-start,'frames_received':frames,
            'last_sequence':last.sequence,'shape':list(last.pixels.shape),'chip_id':last.controls['chip_id'],
            'usb_recoveries':last.controls['usb_recoveries'],'controls_verified':True,
            'png_tiff_and_sensor_pixels_verified':True,'worker_exit_code':process.returncode,
            'last_controls':last.controls}
except Exception as exc:
    result['error']=repr(exc)
    raise
finally:
    camera.close()
    (out/'validation.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2))
