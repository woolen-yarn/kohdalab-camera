"""Real Qt live test: retain hardware controls through image-pipe recovery."""
import argparse,json,time
from pathlib import Path
import numpy as np
from PIL import Image
from PySide6.QtCore import QTimer
from PySide6.QtWidgets import QApplication
from kohdalab_camera.gui import CameraWindow
from kohdalab_camera.color import gain_to_register,render_sensor
from kohdalab_camera.storage import save_frame
p=argparse.ArgumentParser();p.add_argument('--execute',action='store_true');p.add_argument('--hold-seconds',type=float,default=20);a=p.parse_args()
if not a.execute:raise SystemExit('Use --execute to access the connected camera')
if not 10<=a.hold_seconds<=60:raise SystemExit('Hold duration must be 10..60 seconds')
out=Path('captures/pipe-controls');out.mkdir(parents=True,exist_ok=True)
app=QApplication([]);w=CameraWindow();w.backend.setCurrentText('legacy-tca');w.tabs.setCurrentIndex(3);w.show();w.start_capture()
settings=[(128,1),(300,2),(600,4),(1200,8),(128,1)]
stage=0;started=time.monotonic();deadline=started+12;records=[];failure=None

def check():
 global stage,deadline,failure
 try:
  if stage>=len(settings):return
  if w.worker is None:raise RuntimeError(w.message.text())
  f=w.worker.snapshot()
  if f is None:return
  if time.monotonic()<deadline:return
  exposure,gain=settings[stage];c=f.controls
  assert c['exposure'].get('requested')==exposure and c['gain'].get('requested')==gain_to_register(gain),'Settings were not retained'
  assert c['exposure']['readback']==exposure and c['gain']['readback']==gain_to_register(gain)
  assert c['usb_bus_resets']==0,'Device reset was needed'
  assert 'rejected_hardware_profile' not in c,'Hardware profile fell back'
  expected=render_sensor(f.sensor_raw,c['display_mode'],'GRBG',c['white_balance_rgb'],c['display_gamma'],**c['image_options'])
  path=save_frame(f,out,'png');assert np.array_equal(np.asarray(Image.open(path)),expected)
  record={'exposure_rows':exposure,'gain':gain,'sequence':f.sequence,'pipe_recoveries':c['usb_pipe_recoveries'],'bus_resets':c['usb_bus_resets'],'raw_mean':float(f.sensor_raw.mean())};records.append(record);print(json.dumps(record),flush=True)
  if stage==len(settings)-1:
   w.grab().save(str(out/'gui.png'));stage+=1;timer.stop();w.close();return
  stage+=1;exposure,gain=settings[stage];w.exposure.setValue(exposure);w.gain.setValue(gain);w.apply_controls();deadline=time.monotonic()+a.hold_seconds
 except Exception as exc:
  failure=str(exc)
  if w.worker and w.worker.snapshot():records.append({'failure_controls':w.worker.snapshot().controls})
  timer.stop();w.close()
timer=QTimer();timer.setInterval(250);timer.timeout.connect(check);timer.start();app.exec()
result={'failure':failure,'records':records,'seconds':time.monotonic()-started,'stopped':w.worker is None};(out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps(result),flush=True)
if failure or len(records)!=len(settings):raise SystemExit(failure or 'Incomplete settings sweep')
