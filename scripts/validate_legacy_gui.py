"""Opt-in real-camera GUI smoke test; saves observations and owned Qt screenshot."""
import argparse,json,time
from pathlib import Path
from PySide6.QtWidgets import QApplication
from kohdalab_camera.gui import CameraWindow

p=argparse.ArgumentParser();p.add_argument('--execute',action='store_true');p.add_argument('--frames',type=int,default=10);p.add_argument('--check-controls',action='store_true');p.add_argument('--output',type=Path,default=Path('captures/gui-real-validation'))
a=p.parse_args()
if not a.execute:raise SystemExit('No USB access. Add --execute to capture the connected 0547:4D33 camera.')
if not 8<=a.frames<=30:raise SystemExit('frames must be 8..30')
a.output.mkdir(parents=True,exist_ok=True)
app=QApplication([]);window=CameraWindow();window.show();window.backend.setCurrentText('legacy-tca');window.directory.setText(str(a.output));window.start_capture()
rows=[];last=0;failure=None;started=time.monotonic();saved=False
first_controls=False;second_controls=False
initial_saved=len(list(a.output.glob('*.sensor.tiff')))
try:
 deadline=started+75
 while time.monotonic()<deadline:
  app.processEvents()
  if window.worker is None:raise RuntimeError(window.message.text())
  frame=window.worker.snapshot()
  if frame is not None and frame.sequence!=last:
   last=frame.sequence
   row={'sequence':last,'elapsed':round(time.monotonic()-started,3),'mean':round(float(frame.sensor_raw.mean()),3),'controls':frame.controls};rows.append(row)
   print({k:row[k] for k in ('sequence','elapsed','mean')},flush=True)
   if last>=3 and not saved:
    window.enqueue(('save',str(a.output),'png'));saved=True
   if a.check_controls and last>=3 and not first_controls:
    window.enqueue(('controls',{'exposure':256,'gain':8}));first_controls=True
   exposure=frame.controls['exposure'].get('requested',frame.controls['exposure'].get('reported'))
   if a.check_controls and last>=6 and exposure==256 and not second_controls:
    window.enqueue(('controls',{'exposure':64,'gain':8}));second_controls=True
   if last>=a.frames and (not a.check_controls or exposure==64):
    window.enqueue(('save',str(a.output),'png'))
    until=time.monotonic()+8
    while time.monotonic()<until and len(list(a.output.glob('*.sensor.tiff')))<initial_saved+2:
     app.processEvents();time.sleep(.02)
    if len(list(a.output.glob('*.sensor.tiff')))<initial_saved+2:
     raise RuntimeError('Final save did not finish before deadline')
    window.refresh();window.grab().save(str(a.output/'gui-preview.png'));break
  time.sleep(.02)
 else:raise RuntimeError('GUI validation deadline')
except Exception as exc:
 failure=str(exc);print('failure',failure,flush=True)
finally:
 window.stop_capture();deadline=time.monotonic()+12
 while window.worker is not None and time.monotonic()<deadline:app.processEvents();time.sleep(.02)
 if window.worker is not None:raise RuntimeError('GUI worker did not stop')
 (a.output/'validation.json').write_text(json.dumps({'frames':rows,'failure':failure,'stopped':True},indent=2));window.close()
 print('stopped',window.worker is None,'saved',len(list(a.output.glob('*.sensor.tiff'))),flush=True)
if failure:raise SystemExit(failure)
