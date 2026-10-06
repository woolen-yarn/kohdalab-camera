"""Opt-in low-bandwidth GUI control sweep with strict USB error reporting."""
import argparse,json,time
from pathlib import Path
import numpy as np
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QTimer
from kohdalab_camera.gui import CameraWindow
from kohdalab_camera.storage import save_frame
from kohdalab_camera.color import render_sensor
from PIL import Image
p=argparse.ArgumentParser();p.add_argument('--execute',action='store_true');p.add_argument('--output',type=Path,default=Path('captures/mode-controls'));args=p.parse_args()
if not args.execute:raise SystemExit('Use --execute to access the camera')
args.output.mkdir(parents=True,exist_ok=True)
app=QApplication([]);w=CameraWindow();w.backend.setCurrentText('legacy-tca');w.show();w.start_capture()
started=time.monotonic();stage=0;records=[];failure=None
settings=[(128,1),(300,2),(600,4),(128,1)]
def check():
 global stage,failure
 try:
  elapsed=time.monotonic()-started
  f=w.worker.snapshot() if w.worker else None
  if w.worker is None:raise RuntimeError(w.message.text())
  if f is None:return
  assert f.controls['usb_recoveries']==0,'Spontaneous USB recovery'
  assert f.pixels.shape==(768,1024,3),'Unexpected resolution'
  if elapsed>15*(stage+1):
   expected_exposure,expected_gain=settings[stage]
   assert f.controls['exposure'].get('requested')==expected_exposure
   from kohdalab_camera.color import gain_to_register
   assert f.controls['gain'].get('requested')==gain_to_register(expected_gain)
   records.append({'sequence':f.sequence,'exposure_rows':expected_exposure,'gain':expected_gain,'mean_rgb':float(f.pixels.mean()),'recoveries':f.controls['usb_recoveries']})
   if stage==len(settings)-1:
    c=f.controls;expected=render_sensor(f.sensor_raw,c['display_mode'],c['bayer_pattern'],c['white_balance_rgb'],c['display_gamma'],**c['image_options'])
    assert np.array_equal(expected,f.pixels)
    path=save_frame(f,args.output,'png');assert np.array_equal(np.asarray(Image.open(path)),expected)
    w.tabs.setCurrentIndex(2);w.grab().save(str(args.output/'export.png'));w.close();return
   stage+=1;exposure,gain=settings[stage];w.exposure.setValue(exposure);w.gain.setValue(gain);w.apply_controls()
 except Exception as exc:failure=str(exc);w.close()
timer=QTimer();timer.setInterval(250);timer.timeout.connect(check);timer.start();app.exec()
result={'failure':failure,'stopped':w.worker is None,'records':records};(args.output/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps(result),flush=True)
if failure or len(records)!=4:raise SystemExit(failure or 'Incomplete sweep')
