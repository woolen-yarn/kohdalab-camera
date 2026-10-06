"""Opt-in real GUI checks for display settings, recovery, bounded-exposure stop."""
import argparse,json,time
from pathlib import Path
import numpy as np
from PIL import Image
from PySide6.QtWidgets import QApplication
from kohdalab_camera.gui import CameraWindow
from kohdalab_camera.color import render_sensor

p=argparse.ArgumentParser();p.add_argument('--execute',action='store_true')
p.add_argument('--output',type=Path,default=Path('captures/features-final'))
a=p.parse_args()
if not a.execute:raise SystemExit('Add --execute to access the camera.')
a.output.mkdir(parents=True,exist_ok=True)
app=QApplication([]);w=CameraWindow();w.show();w.backend.setCurrentText('legacy-tca')
w.directory.setText(str(a.output));w.exposure.setValue(128)
results={};failure=None
def wait(predicate,seconds=20):
    deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        app.processEvents()
        if predicate():return
        time.sleep(.01)
    raise RuntimeError('GUI deadline: '+w.message.text())
def frame():return w.worker.snapshot() if w.worker else None
def setting(predicate):wait(lambda:frame() is not None and predicate(frame()))
def save(format):
    initial=set(a.output.glob('capture-*.'+format+'.json'))
    w.format.setCurrentText(format);w.save_button.click()
    wait(lambda:len(set(a.output.glob('capture-*.'+format+'.json'))-initial)==1)
    sidecar=next(iter(set(a.output.glob('capture-*.'+format+'.json'))-initial))
    meta=json.loads(sidecar.read_text());raw=np.asarray(Image.open(a.output/meta['sensor_raw_file']))
    rgb=np.asarray(Image.open(Path(str(sidecar)[:-5])));c=meta['controls']
    expected=render_sensor(raw,c['display_mode'],c['bayer_pattern'],c['white_balance_rgb'],c['display_gamma'],**c.get('image_options',{}))
    if not np.array_equal(rgb,expected):raise RuntimeError('Saved pixel mismatch')
    return sidecar.name
try:
    w.start_button.click();setting(lambda f:f.sequence>=3)
    if not w.white_button.isEnabled():raise RuntimeError('White balance button disabled after connection')
    if frame().controls['exposure']['requested']!=128:raise RuntimeError('Initial exposure not applied')
    w.white_button.click();setting(lambda f:f.controls['white_balance_rgb']!=[1,1,1])
    results['white_balance_rgb']=frame().controls['white_balance_rgb']
    w.color_mode.setCurrentText('Monochrome');w.gamma.setValue(1)
    setting(lambda f:f.controls['display_mode']=='gray' and f.controls['display_gamma']==1)
    results['gray_png']=save('png')
    w.color_mode.setCurrentText('Color');w.gamma.setValue(2.2);w.white_reset_button.click()
    setting(lambda f:f.controls['display_mode']=='color' and f.controls['display_gamma']==2.2 and f.controls['white_balance_rgb']==[1,1,1])
    results['color_tiff']=save('tiff')
    # Deterministic fault injection exercises real cancel/close/open and USB acquisition.
    camera=w.worker.camera
    w.enqueue(('fault',))
    setting(lambda f:f.controls['usb_recoveries']>=1)
    results['injected_usb_fault_recovered']=True
    w.refresh();w.grab().save(str(a.output/'gui-preview.png'))
    w.exposure.setValue(1536);w.apply_button.click()
    wait(lambda:camera.controls['exposure'].get('requested')in (1536,128))
    started=time.monotonic();w.stop_button.click();wait(lambda:w.worker is None,5)
    results['bounded_exposure_stop_seconds']=round(time.monotonic()-started,3)
    w.exposure.setValue(128);w.start_button.click();setting(lambda f:f.sequence>=3)
    w.close();wait(lambda:w.worker is None and not w.isVisible(),5)
    results['restart_and_close']=True
except Exception as exc:
    failure=str(exc);print('FAIL',failure,flush=True)
finally:
    w.stop_capture();wait(lambda:w.worker is None,12);w.close()
    results['failure']=failure
    (a.output/'validation.json').write_text(json.dumps(results,indent=2));print(json.dumps(results,indent=2),flush=True)
if failure:raise SystemExit(failure)
