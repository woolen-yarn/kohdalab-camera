"""Opt-in sustained real Qt event-loop capture; does not manually pump events."""
import argparse,json,time,signal
from pathlib import Path
from PySide6.QtCore import QTimer
from PySide6.QtWidgets import QApplication
from kohdalab_camera.gui import CameraWindow

p=argparse.ArgumentParser();p.add_argument('--execute',action='store_true')
p.add_argument('--seconds',type=int,default=180)
p.add_argument('--require-clean',action='store_true',help='Fail if a spontaneous USB recovery occurs')
p.add_argument('--output',type=Path,default=Path('captures/live-event-loop'))
a=p.parse_args()
if not a.execute:raise SystemExit('Add --execute for the connected camera.')
a.output.mkdir(parents=True,exist_ok=True)
app=QApplication([]);w=CameraWindow();w.backend.setCurrentText('legacy-tca');w.show();w.start_capture()
started=time.monotonic();rows=[];failure=None;ending=False
def sample():
    global ending,failure
    elapsed=time.monotonic()-started
    f=w.worker.snapshot() if w.worker else None
    if f is not None:
        rows.append({'elapsed':round(elapsed,3),'sequence':f.sequence,'recoveries':f.controls['usb_recoveries'],
            'last_transport_error':f.controls['last_transport_error']})
    if w.worker is None and not ending:
        failure=w.message.text();ending=True;w.close()
    elif elapsed>=a.seconds and not ending:
        ending=True;w.grab().save(str(a.output/'gui-preview.png'));w.close()
    if int(elapsed)%15==0 and f is not None:print(rows[-1],flush=True)
timer=QTimer();timer.setInterval(1000);timer.timeout.connect(sample);timer.start()
for signum in (signal.SIGINT,signal.SIGTERM):signal.signal(signum,lambda *_:w.close())
app.exec()
if not rows or rows[-1]['sequence']<a.seconds*1.5:failure=failure or 'Too few live frames'
if a.require_clean and rows and rows[-1]['recoveries']:
    failure=failure or f"Spontaneous USB recoveries: {rows[-1]['recoveries']}"
result={'failure':failure,'stopped':w.worker is None,'samples':rows}
(a.output/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k!='samples'}),flush=True)
if failure:raise SystemExit(failure)
