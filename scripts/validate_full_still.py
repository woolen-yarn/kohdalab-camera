"""Opt-in full-sensor still capture and low-bandwidth preview resumption."""
import json,time
from pathlib import Path
import numpy as np
from PIL import Image
from PySide6.QtCore import QTimer
from PySide6.QtWidgets import QApplication
from kohdalab_camera.gui import CameraWindow
from kohdalab_camera.color import render_sensor
import argparse
p=argparse.ArgumentParser();p.add_argument('--execute',action='store_true');p.add_argument('--exposure',type=int,default=128);p.add_argument('--gain',type=float,default=1);p.add_argument('--output',type=Path,default=Path('captures/full-still'));a=p.parse_args()
if not a.execute:raise SystemExit('Use --execute for hardware access')
out=a.output;out.mkdir(parents=True,exist_ok=True)
old_paths=set(out.glob('*.png.json'))
app=QApplication([]);w=CameraWindow();w.backend.setCurrentText('legacy-tca');w.tabs.setCurrentIndex(2);w.exposure.setValue(a.exposure);w.gain.setValue(a.gain);w.show();w.start_capture()
start=time.monotonic();stage=0;before=0;failure=None;records=[]
def check():
 global stage,before,failure
 try:
  if stage==3:return
  elapsed=time.monotonic()-start
  if elapsed>70:raise RuntimeError('Still capture deadline: '+w.message.text())
  if w.worker is None:raise RuntimeError(w.message.text())
  f=w.worker.snapshot()
  if f is None:return
  if stage==0 and elapsed>4:
   before=f.sequence;w.enqueue(('save-full',str(out),'png'));stage=1
  elif stage==1:
   paths=[path for path in out.glob('*.png.json') if path not in old_paths]
   if paths:
    meta=json.loads(paths[-1].read_text());raw=np.asarray(Image.open(out/meta['sensor_raw_file']))
    controls=meta['controls'];expected=render_sensor(raw,controls['display_mode'],'GRBG',controls['white_balance_rgb'],controls['display_gamma'],**controls['image_options'])
    saved=np.asarray(Image.open(Path(str(paths[-1])[:-5])))
    assert saved.shape[:2]==(1536,2048) and np.array_equal(saved,expected)
    assert controls['capture_profile']=='full'
    from kohdalab_camera.color import gain_to_register
    assert controls['exposure']['readback']==a.exposure and controls['gain']['readback']==gain_to_register(a.gain)
    records.append({'full_sensor_saved':True,'shape':list(saved.shape),'exposure_rows':a.exposure,'gain_factor':a.gain,'sequence':meta.get('sequence')});stage=2
  elif stage==2 and f.sequence>before+10 and f.controls['capture_profile']=='balanced':
   assert f.pixels.shape[:2]==(768,1024)
   records.append({'live_resumed':True,'sequence':f.sequence,'recoveries':f.controls['usb_recoveries']});w.grab().save(str(out/'gui-preview.png'));stage=3;timer.stop();w.close()
 except Exception as exc:failure=str(exc);w.close()
timer=QTimer();timer.setInterval(250);timer.timeout.connect(check);timer.start();app.exec()
result={'failure':failure,'records':records,'complete':stage==3};(out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps(result),flush=True)
if failure or stage!=3:raise SystemExit(failure or 'Incomplete')
