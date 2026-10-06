"""Isolate USB acquisition from the macOS Qt event loop and its Python callbacks."""
import base64,copy,json,pickle,struct,subprocess,sys,time
from pathlib import Path
from queue import Queue,Empty
from threading import Thread


class ProcessCamera:
    def __init__(self,backend='legacy-tca'):
        self.backend=backend;self.process=None;self.controls={}
        self.initial={'exposure':128,'gain':8};self.display='color';self.gamma=2.2
        self.capture_profile="balanced"
        self.aborting=False
        self.image_options={}
        self.response_queue=None
        self.response_thread=None

    def _start_response_reader(self):
        # Blocking reads run outside the capture/UI thread. Queue wakeups avoid
        # Windows' repeated PeekNamedPipe/sleep delay while retaining cancellation.
        self.response_queue=Queue()
        pipe=self.process.stdout
        responses=self.response_queue
        def read_exact(count):
            chunks=[]
            while count:
                data=pipe.read(count)
                if not data:raise RuntimeError('Camera process exited during acquisition')
                chunks.append(data);count-=len(data)
            return b''.join(chunks)
        def pump():
            try:
                while True:
                    size=struct.unpack('!I',read_exact(4))[0]
                    if not 0<size<=64*1024*1024:raise RuntimeError('Invalid local camera response size')
                    responses.put(read_exact(size))
            except Exception as exc:responses.put(exc)
        self.response_thread=Thread(target=pump,name='camera-response',daemon=True)
        self.response_thread.start()

    def set_capture_profile(self,profile):
        if self.process is not None:raise RuntimeError('Choose the camera mode before connecting')
        from .legacy_usb import LegacyUsbCamera
        validator=LegacyUsbCamera();validator.set_capture_profile(profile)
        self.capture_profile=profile

    def configure_controls(self,values):
        from .legacy_usb import LegacyUsbCamera
        validator=LegacyUsbCamera();validator.configure_controls(values)
        self.initial.update(values)

    def _rpc(self,op,cancelled=None,**values):
        p=self.process
        if p is None or p.poll() is not None:raise RuntimeError('Camera process is not running')
        if self.response_queue is None:self._start_response_reader()
        p.stdin.write((json.dumps({'op':op,**values})+'\n').encode());p.stdin.flush()
        deadline=time.monotonic()+90
        while True:
            if cancelled is not None and cancelled():
                self.aborting=True
                p.terminate();raise InterruptedError('Capture stopped')
            if time.monotonic()>deadline:
                self.aborting=True
                p.terminate();raise RuntimeError('Camera process response deadline')
            try:data=self.response_queue.get(timeout=.1);break
            except Empty:continue
        if isinstance(data,Exception):
            self.aborting=True
            if p.poll() is None:p.terminate()
            raise data
        # Only our owned child process writes this private pipe; no external input.
        response=pickle.loads(data)
        if not response['ok']:
            if response.get('interrupted'):raise InterruptedError(response['error'])
            raise RuntimeError(response['error'])
        return response['result']

    def open(self):
        if self.process is not None:raise RuntimeError('Camera already open')
        if getattr(sys, 'frozen', False):
            worker='camera-worker.exe' if sys.platform=='win32' else 'camera-worker'
            command = [str(Path(sys.executable).with_name(worker))]
        else:
            command = [sys.executable, '-m', 'kohdalab_camera.usb_worker']
        flags = subprocess.CREATE_NO_WINDOW if sys.platform == 'win32' else 0
        self.process=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,creationflags=flags)
        try:self.controls=self._rpc('open',backend=self.backend,controls=self.initial,display=self.display,gamma=self.gamma,image_options=self.image_options,capture_profile=self.capture_profile) or {}
        except Exception:self.close();raise

    def read(self):return self.read_interruptible(None)

    def read_interruptible(self,cancelled):
        frame=self._materialize(self._rpc('read',cancelled=cancelled));self.controls=copy.deepcopy(frame.controls)
        return frame

    @staticmethod
    def _materialize(frame):
        from .backends import Frame,SensorFrame
        if not isinstance(frame,SensorFrame):return frame
        import numpy as np
        from .color import render_sensor
        controls=frame.controls
        pixels=render_sensor(frame.sensor_raw,controls['display_mode'],controls['bayer_pattern'],
                             np.asarray(controls['white_balance_rgb'],dtype=np.float32),
                             controls['display_gamma'],**controls['image_options'])
        return Frame(pixels,frame.sequence,frame.timestamp,frame.backend,frame.device,controls,frame.sensor_raw)

    def capture_full_frame(self,cancelled=None):return self._materialize(self._rpc("full_capture",cancelled=cancelled))

    def set_controls(self,values):
        result=self._rpc('controls',values=values);self.controls.update(result);return result

    def set_control(self,name,value):return self.set_controls({name:value})[name]

    def set_display_mode(self,mode):
        self.display=mode
        if self.process is not None:self._rpc('display',value=mode)

    def set_display_gamma(self,gamma):
        self.gamma=gamma
        if self.process is not None:self._rpc('gamma',value=gamma)

    def set_image_options(self,options):
        from .color import validate_image_options
        merged=validate_image_options({**dict(brightness_ev=1.5,mirror=False,flip=False,rotation=180),**self.image_options,**options})
        if self.process is not None:self._rpc('image',options=merged)
        self.image_options=merged

    def balance_white(self,raw):
        self._rpc('white',raw={'shape':list(raw.shape),'data':base64.b64encode(raw.tobytes()).decode()})

    def reset_white_balance(self):self._rpc('white-reset')
    def inject_transport_fault(self):self._rpc('fault')

    def close(self):
        p=self.process
        if p is None:return
        try:
            if self.aborting:
                p.stdin.close()
            if p.poll() is None:
                if not self.aborting:
                    try:self._rpc('close')
                    except (BrokenPipeError,RuntimeError,InterruptedError):p.terminate()
                try:p.wait(timeout=8)
                except subprocess.TimeoutExpired:p.kill();p.wait(timeout=3)
        finally:
            if self.response_thread is not None:self.response_thread.join(timeout=2)
            p.stdin.close();p.stdout.close();self.process=None
            self.aborting=False
            self.response_queue=None;self.response_thread=None
