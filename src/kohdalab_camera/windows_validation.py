"""Opt-in frozen GUI validation against the connected camera."""
import json,time
from pathlib import Path
import numpy as np
from PIL import Image
from PySide6.QtCore import QTimer
from PySide6.QtWidgets import QApplication
from .gui import CameraWindow
from .storage import save_frame


def validate_gui(output, seconds=20):
    if not 1<=seconds<=180:raise ValueError('Validation duration must be 1..180 seconds')
    out=Path(output);out.mkdir(parents=True,exist_ok=True)
    app=QApplication.instance() or QApplication([])
    w=CameraWindow();w.backend.setCurrentText('legacy-tca');w.directory.setText(str(out))
    w.show()
    result={'success':False};state={'start':None,'phase':0,'frames':set(),'stopping':False,
                                  'steady_start':None,'steady_sequence':None}
    timer=QTimer();timer.setInterval(100)
    def fail(reason):
        result['error']=reason
        state['stopping']=True
        w.close()
    def poll():
        if state['stopping']:
            if w.worker is None and getattr(w,'setup_process',None) is None:
                timer.stop();app.quit()
            return
        try:
            if w.worker is None:
                if state['start'] is not None:fail('Capture worker stopped: '+w.message.text())
                return
            f=w.worker.snapshot()
            if f is None:return
            state['frames'].add(f.sequence)
            if state['start'] is None:
                state['start']=time.monotonic()
                assert f.pixels.shape==(768,1024,3)
                w.exposure.setValue(256);w.gain.setValue(2);w.apply_controls();state['phase']=1
            if state['phase']==1 and f.controls['exposure'].get('readback')==256 and f.controls['gain'].get('readback')==16:
                w.exposure.setValue(128);w.gain.setValue(1);w.apply_controls();state['phase']=2
            if state['phase']==2 and f.controls['exposure'].get('readback')==128 and f.controls['gain'].get('readback')==8:
                state['phase']=3
                state['steady_start']=time.monotonic();state['steady_sequence']=f.sequence
                state['steady_preview_frames']=w.preview_frames
            if state['phase']==3 and time.monotonic()-state['steady_start']>=seconds:
                elapsed=time.monotonic()-state['steady_start']
                preview_frames=w.preview_frames-state['steady_preview_frames']
                for fmt in ('png','tiff'):
                    path=save_frame(f,out,fmt)
                    assert np.array_equal(np.asarray(Image.open(path)),f.pixels)
                    assert np.array_equal(np.asarray(Image.open(path.with_suffix('.sensor.tiff'))),f.sensor_raw)
                w.grab().save(str(out/'gui.png'))
                result.update(success=True,elapsed_seconds=time.monotonic()-state['start'],
                    displayed_sequences=len(state['frames']),last_sequence=f.sequence,
                    steady_elapsed_seconds=elapsed,
                    steady_complete_frames=f.sequence-state['steady_sequence'],
                    steady_capture_fps=(f.sequence-state['steady_sequence'])/elapsed,
                    steady_preview_frames=preview_frames,steady_preview_fps=preview_frames/elapsed,
                    shape=list(f.pixels.shape),controls_verified=True,lossless_save_verified=True,
                    last_controls=f.controls)
                state['stopping']=True;w.close()
        except Exception as exc:fail(repr(exc))
    def timeout():
        if not state['stopping']:fail('GUI capture validation deadline: '+w.message.text())
    timer.timeout.connect(poll);timer.start()
    QTimer.singleShot((seconds+55)*1000,timeout)
    QTimer.singleShot(0,w.start_capture)
    app.exec()
    (out/'validation.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    return 0 if result['success'] else 1
