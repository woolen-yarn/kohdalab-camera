"""Opt-in GUI endurance check on the connected legacy camera; no firmware writes."""
import argparse
import json
import time
from pathlib import Path
import numpy as np
from PIL import Image
from PySide6.QtWidgets import QApplication
from kohdalab_camera.gui import CameraWindow
from kohdalab_camera.color import render_sensor

p=argparse.ArgumentParser()
p.add_argument('--execute',action='store_true')
p.add_argument('--cycles',type=int,default=3)
p.add_argument('--seconds',type=int,default=60)
p.add_argument('--output',type=Path,default=Path('captures/endurance'))
a=p.parse_args()
if not a.execute:raise SystemExit('Add --execute to access the connected camera.')
if not 1<=a.cycles<=20 or not 20<=a.seconds<=600:raise SystemExit('Invalid duration/cycle count')
a.output.mkdir(parents=True,exist_ok=True)
app=QApplication([])
w=CameraWindow();w.show();w.backend.setCurrentText('legacy-tca');w.directory.setText(str(a.output))
observations=[];failure=None
def pump():
    app.processEvents();time.sleep(.01)
try:
    for cycle in range(a.cycles):
        w.exposure.setValue(128);w.gain.setValue(1)
        w.start_button.click();started=time.monotonic();last=0;count=0
        while time.monotonic()-started<a.seconds:
            pump()
            if w.worker is None:raise RuntimeError(w.message.text())
            frame=w.worker.snapshot()
            if frame is None or frame.sequence==last:continue
            last=frame.sequence;count+=1
            observations.append({'cycle':cycle+1,'sequence':last,'elapsed':round(time.monotonic()-started,3),
                'mean':round(float(frame.sensor_raw.mean()),3),'controls':frame.controls})
            if count in (3,8,13):
                exposure,gain={3:(128,16),8:(256,8),13:(64,8)}[count]
                w.exposure.setValue(exposure);w.gain.setValue(gain/8);w.apply_button.click()
            if count in (6,17):
                w.format.setCurrentText('png' if count==6 else 'tiff');w.save_button.click()
            if count%25==0:print('cycle',cycle+1,'frames',count,'seconds',round(time.monotonic()-started,1),flush=True)
        if count<20:raise RuntimeError(f'Only {count} frames in cycle {cycle+1}')
        w.refresh();w.grab().save(str(a.output/f'gui-cycle-{cycle+1}.png'))
        w.stop_button.click();deadline=time.monotonic()+12
        while w.worker is not None and time.monotonic()<deadline:pump()
        if w.worker is not None:raise RuntimeError('Stop deadline')
        print('cycle',cycle+1,'stopped; frames',count,flush=True)
    files=list(a.output.glob('capture-*.png.json'))+list(a.output.glob('capture-*.tiff.json'))
    if len(files)!=a.cycles*2:raise RuntimeError(f'Expected {a.cycles*2} captures, got {len(files)}')
    for sidecar in files:
        meta=json.loads(sidecar.read_text())
        raw=np.asarray(Image.open(a.output/meta['sensor_raw_file']))
        rgb=np.asarray(Image.open(Path(str(sidecar)[:-5])))
        controls=meta['controls']
        expected=render_sensor(raw,controls.get('display_mode','gray'),controls.get('bayer_pattern','GRBG'),
                               controls.get('white_balance_rgb',[1,1,1]),controls.get('display_gamma',1),**controls.get('image_options',{}))
        if raw.shape!=(1536,2048) or not np.array_equal(rgb,expected):
            raise RuntimeError(f'Saved image mismatch: {sidecar}')
    for cycle in range(1,a.cycles+1):
        groups={}
        for row in observations:
            if row['cycle']!=cycle:continue
            controls=row['controls']
            key=tuple(controls[n].get('requested',controls[n].get('reported')) for n in ('exposure','gain'))
            groups.setdefault(key,[]).append(row['mean'])
        baseline=np.median(groups[(128,8)])
        if np.median(groups[(128,16)])<=baseline*1.2:raise RuntimeError('Gain did not visibly increase brightness')
        if np.median(groups[(256,8)])<=np.median(groups[(64,8)])*1.5:raise RuntimeError('Exposure brightness check failed')
except Exception as exc:
    failure=str(exc);print('FAIL',failure,flush=True)
finally:
    w.stop_capture();deadline=time.monotonic()+12
    while w.worker is not None and time.monotonic()<deadline:pump()
    stopped=w.worker is None
    (a.output/'validation.json').write_text(json.dumps({'failure':failure,'stopped':stopped,'frames':observations},indent=2))
    if stopped:w.close()
if failure or not stopped:raise SystemExit(failure or 'Worker still running')
print('PASS: repeated GUI controls, PNG/TIFF, full-size saved pixels, stop/reconnect',flush=True)
