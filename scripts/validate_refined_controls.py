"""Opt-in real GUI: software appearance, lossless export and hardware fallback."""
import argparse,json,time
from pathlib import Path
import numpy as np
from PIL import Image
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QTimer
from kohdalab_camera.gui import CameraWindow
from kohdalab_camera.color import render_sensor
from kohdalab_camera.storage import save_frame
p=argparse.ArgumentParser();p.add_argument('--execute',action='store_true');p.add_argument('--output',type=Path,default=Path('captures/refined-controls'));a=p.parse_args()
if not a.execute:raise SystemExit('Use --execute for hardware access')
a.output.mkdir(parents=True,exist_ok=True)
app=QApplication([]);w=CameraWindow();w.backend.setCurrentText('legacy-tca');w.show();w.start_capture()
started=time.monotonic();stage=0;records=[];failure=None

def check():
 global stage,failure
 try:
  elapsed=time.monotonic()-started
  if w.worker is None:raise RuntimeError(w.message.text())
  f=w.worker.snapshot()
  if f is None:return
  if stage==0 and elapsed>4:
   w.brightness.setValue(2);w.mirror.setChecked(True);w.flip.setChecked(True);w.tabs.setCurrentIndex(1);stage=1
  elif stage==1 and f.controls['image_options']==dict(brightness_ev=2.0,mirror=True,flip=True,rotation=180):
   options=f.controls['image_options'];expected=render_sensor(f.sensor_raw,f.controls['display_mode'],'GRBG',f.controls['white_balance_rgb'],f.controls['display_gamma'],**options)
   assert np.array_equal(f.pixels,expected)
   path=save_frame(f,a.output,'png');meta=json.loads(path.with_suffix('.png.json').read_text())
   assert np.array_equal(np.asarray(Image.open(path)),expected)
   assert np.array_equal(np.asarray(Image.open(a.output/meta['sensor_raw_file'])),f.sensor_raw)
   w.grab().save(str(a.output/'image-tab.png'));records.append({'appearance_and_save':True,'sequence':f.sequence,'recoveries':f.controls['usb_recoveries']})
   w.exposure.setValue(600);w.gain.setValue(2);w.apply_controls();stage=2
  elif stage==2 and elapsed>25:
   assert f.controls['exposure'].get('requested') in (128,600)
   records.append({'sequence':f.sequence,'exposure':f.controls['exposure'],'gain':f.controls['gain'],'recoveries':f.controls['usb_recoveries'],'fallback':f.controls.get('rejected_hardware_profile')})
   w.grab().save(str(a.output/'after-hardware-settings.png'));stage=3
  elif stage==3 and elapsed>45:
   records.append({'final_sequence':f.sequence,'recoveries':f.controls['usb_recoveries']});w.close()
  if elapsed>75:raise RuntimeError('Validation deadline')
 except Exception as exc:failure=str(exc);w.close()
timer=QTimer();timer.setInterval(250);timer.timeout.connect(check);timer.start();app.exec()
result={'failure':failure,'stopped':w.worker is None,'records':records};(a.output/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps(result),flush=True)
if failure or stage!=3:raise SystemExit(failure or 'Incomplete validation')
