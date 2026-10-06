import os
os.environ.setdefault('QT_QPA_PLATFORM', 'offscreen')
import time
from PySide6.QtWidgets import QApplication
from kohdalab_camera.gui import CameraWindow, CaptureWorker
from kohdalab_camera.backends import SimulatedCamera


def pump(app, predicate, timeout=5):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        app.processEvents()
        if predicate(): return
        time.sleep(0.01)
    raise AssertionError('GUI timeout')


def test_gui_capture_save_stop_restart(tmp_path):
    app = QApplication.instance() or QApplication([])
    window = CameraWindow()
    window.directory.setText(str(tmp_path))
    window.start_capture()
    pump(app, lambda: window.worker.snapshot() is not None)
    window.refresh()
    assert '640' in window.stats.text()
    window.enqueue(('save', str(tmp_path), 'png'))
    pump(app, lambda: len(list(tmp_path.glob('*.json'))) == 1)
    window.stop_capture()
    pump(app, lambda: window.worker is None)
    window.start_capture()
    pump(app, lambda: window.worker.snapshot() is not None)
    window.stop_capture()
    pump(app, lambda: window.worker is None)
    assert window.start_button.isEnabled()
    window.close()


def test_worker_failure_closes_camera():
    app = QApplication.instance() or QApplication([])
    class Failing(SimulatedCamera):
        def read(self): raise RuntimeError('unplugged')
    camera = Failing()
    worker = CaptureWorker(camera)
    worker.start()
    assert worker.wait(2000)
    assert not camera.opened


def test_legacy_defaults_ranges_and_os_api_disabled():
    app=QApplication.instance() or QApplication([])
    window=CameraWindow();window.backend.setCurrentText('legacy-tca')
    assert window.exposure.value()==128 and window.gain.value()==1
    assert window.gain.minimum()==1 and window.gain.maximum()==8
    assert not window.api.isEnabled() and not window.index.isEnabled()
    window.exposure.setValue(300);window.gain.setValue(4);window.update_units()
    assert window.exposure.value()==300 and window.gain.value()==4
    window.backend.setCurrentText('simulated')
    assert window.api.isEnabled() and window.gain.value()==1
    window.close()


def test_close_window_stops_worker_and_closes_automatically():
    app=QApplication.instance() or QApplication([])
    window=CameraWindow();window.show();window.start_capture()
    pump(app,lambda:window.worker.snapshot() is not None)
    window.close()
    pump(app,lambda:window.worker is None and not window.isVisible())


def test_english_tabs_and_image_options():
    app=QApplication.instance() or QApplication([])
    w=CameraWindow();w.backend.setCurrentText('legacy-tca')
    assert [w.tabs.tabText(i) for i in range(w.tabs.count())]==['Capture','Image','Export','Advanced']
    w.mirror.setChecked(True);w.flip.setChecked(True)
    assert w.image_values()==dict(brightness_ev=1.5,mirror=True,flip=True,rotation=180)
    w.gain.setValue(8);assert w.control_values()['gain']==96
    assert w.exposure.maximum()==1536
    w.close()


def test_fixed_orientation_and_collapsible_iv_panels():
    app=QApplication.instance() or QApplication([])
    w=CameraWindow();w.show();app.processEvents()
    assert w.image_values()['rotation']==180
    assert not hasattr(w,'rotation')
    assert w.tabs.isVisible() and not w.log.isVisible()
    w.panel_toggle.click();assert not w.tabs.isVisible()
    w.panel_toggle.click();assert w.tabs.isVisible()
    w.log_toggle.click();assert w.log.isVisible()
    w.close()


def test_export_dropdown_has_readable_width_and_scrollable_tabs():
    from PySide6.QtWidgets import QScrollArea
    app=QApplication.instance() or QApplication([])
    w=CameraWindow();w.backend.setCurrentText('legacy-tca')
    assert w.format.minimumWidth()>=140
    assert w.format.view().minimumWidth()>=220
    assert all(isinstance(w.tabs.widget(i),QScrollArea) for i in range(w.tabs.count()))
    assert not w.advanced_form.isRowVisible(w.api)
    w.close()
