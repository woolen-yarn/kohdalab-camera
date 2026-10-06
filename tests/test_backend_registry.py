import os
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
import json
import pytest
from kohdalab_camera import backends


def test_adapter_registration_reaches_factory_and_gui(monkeypatch):
    monkeypatch.setattr(backends,'_backends',dict(backends._backends))
    calls=[]
    def factory(index,api):
        calls.append((index,api));return backends.SimulatedCamera(width=16,height=12)
    spec=backends.BackendSpec('example-sdk','Example SDK','experimental','Test extension',factory)
    backends.register_backend(spec)
    camera=backends.make_camera('example-sdk',2,'auto')
    assert calls==[(2,'auto')] and camera.width==16
    with pytest.raises(ValueError,match='already registered'):backends.register_backend(spec)
    from PySide6.QtWidgets import QApplication
    from kohdalab_camera.gui import CameraWindow
    app=QApplication.instance() or QApplication([])
    window=CameraWindow()
    assert window.backend.findText('example-sdk')>=0
    window.close()


def test_backend_list_never_constructs_or_opens_a_camera(monkeypatch,capsys):
    from kohdalab_camera.cli import main
    monkeypatch.setattr('sys.argv',['kohdalab-camera','backends'])
    monkeypatch.setattr('kohdalab_camera.cli.make_camera',lambda *_:pytest.fail('Hardware was constructed'))
    assert main()==0
    report=json.loads(capsys.readouterr().out)
    assert {b['id']:b['support'] for b in report}=={
        'simulated':'simulated','opencv':'generic-adapter','legacy-tca':'hardware-verified'}
