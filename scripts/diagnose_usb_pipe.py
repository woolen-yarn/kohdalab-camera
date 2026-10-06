"""Opt-in USB image-pipe diagnosis; no automatic retry or settings fallback."""
import argparse
import json
import time
from pathlib import Path
from kohdalab_camera.legacy_usb import LegacyUsbCamera
from kohdalab_camera.color import gain_to_register
from kohdalab_camera.storage import save_frame

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--execute',action='store_true')
parser.add_argument('--exposure',type=int,default=300)
parser.add_argument('--gain',type=float,default=2)
parser.add_argument('--seconds',type=float,default=30)
parser.add_argument('--output',type=Path,default=Path('captures/pipe-diagnosis'))
args=parser.parse_args()
if not args.execute:parser.error('Use --execute to access the connected camera')
if not 0<args.seconds<=600:parser.error('Duration must be within 0..600 seconds')
camera=LegacyUsbCamera()
camera.configure_controls({'exposure':args.exposure,'gain':gain_to_register(args.gain)})
args.output.mkdir(parents=True,exist_ok=True)
result={'exposure_rows':args.exposure,'gain_factor':args.gain,'automatic_recovery':False}
start=time.monotonic();count=0;frame=None;error=None
try:
    camera.open()
    result['initial_sensor_state']=camera.capture_transport_state()
    while time.monotonic()-start<args.seconds:
        frame=camera.read(recover=False)
        count+=1
except Exception as exc:
    error=str(exc)
    if camera.device is not None:result['sensor_state_at_error']=camera.capture_transport_state()
finally:
    try:camera.close()
    except Exception as exc:result['cleanup_error']=str(exc)
result.update(error=error,frames=count,seconds=time.monotonic()-start)
if frame is not None:
    result['last_frame_raw_mean']=float(frame.sensor_raw.mean())
    result['last_frame_file']=str(save_frame(frame,args.output,'png'))
(args.output/'diagnosis.json').write_text(json.dumps(result,indent=2))
print(json.dumps(result,indent=2),flush=True)
if error or 'cleanup_error' in result:raise SystemExit(1)
