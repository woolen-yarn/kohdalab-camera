"""Shared portable entry; the USB worker keeps binary stdout isolated."""
from pathlib import Path
import sys

if __name__ == '__main__':
    name = Path(sys.executable).name.lower()
    if name in ('camera-worker.exe','camera-worker'):
        from kohdalab_camera.usb_worker import main
        main()
    elif name in ('camera-cli.exe','camera-cli'):
        from kohdalab_camera.cli import main
        raise SystemExit(main())
    elif '--validate-hardware' in sys.argv:
        import argparse
        from kohdalab_camera.windows_validation import validate_gui
        parser=argparse.ArgumentParser()
        parser.add_argument('--validate-hardware',action='store_true')
        parser.add_argument('--validate-seconds',type=int,default=20)
        args=parser.parse_args()
        output=Path(sys.executable).parent/'validation' if sys.platform=='win32' else Path.home()/'Library/Application Support/KohdaLab Camera/validation'
        raise SystemExit(validate_gui(output,args.validate_seconds))
    elif '--smoke-test' in sys.argv:
        import json, tempfile
        from PySide6.QtWidgets import QApplication
        from kohdalab_camera.gui import CameraWindow
        from kohdalab_camera.process_camera import ProcessCamera
        from kohdalab_camera.storage import save_frame
        from kohdalab_camera.color import render_sensor,_native_renderer
        import hashlib,numpy as np
        assert _native_renderer() is not None
        sensor=np.random.default_rng(42).integers(0,256,(65,67),dtype=np.uint8)
        rendered=render_sensor(sensor,white_balance=np.array([1.351,.701,2.33],np.float32),
                               brightness_ev=1.5,gamma=2.2,mirror=True,flip=True,rotation=90)
        assert hashlib.sha256(rendered.tobytes()).hexdigest()=='622fb1a09fc0df32e135dd57249f07172ec579722c5b0fa0ee46899925042b96'
        app=QApplication([])
        window=CameraWindow(); window.show(); app.processEvents()
        camera=ProcessCamera('simulated')
        try:
            camera.open(); process=camera.process; frame=camera.read()
            assert frame.pixels.shape == (480,640,3)
            with tempfile.TemporaryDirectory() as output:
                assert save_frame(frame,output,'png').exists()
            window.close()
        finally:
            camera.close()
        assert process.poll() == 0
        Path(sys.executable).with_name('portable-smoke.json').write_text(json.dumps({'ok':True,'shape':list(frame.pixels.shape),'worker_exit':True,'native_renderer_verified':True}))
    else:
        from kohdalab_camera.gui import main
        raise SystemExit(main())
