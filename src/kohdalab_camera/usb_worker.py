"""Private local camera process protocol. stdout is reserved for binary replies."""
import base64,json,pickle,signal,struct,sys
from threading import Event
from .backends import make_camera

def main():
    stopped=Event();camera=None
    for signum in (signal.SIGINT,signal.SIGTERM):signal.signal(signum,lambda *_:stopped.set())
    try:
        for line in sys.stdin.buffer:
            request=json.loads(line);op=request['op']
            try:
                if op=='open':
                    camera=make_camera(request['backend'])
                    if hasattr(camera,'set_capture_profile'):camera.set_capture_profile(request.get('capture_profile','balanced'))
                    if hasattr(camera,'configure_controls'):camera.configure_controls(request['controls'])
                    if hasattr(camera,'set_display_mode'):
                        camera.set_display_mode(request['display']);camera.set_display_gamma(request['gamma'])
                        camera.set_image_options(request.get('image_options',{}))
                    camera.open();result=camera.controls if hasattr(camera,'controls') else None
                elif op=='read':
                    if hasattr(camera,'read_sensor_frame'):result=camera.read_sensor_frame(stopped.is_set)
                    else:result=camera.read_interruptible(stopped.is_set) if hasattr(camera,'read_interruptible') else camera.read()
                elif op=='full_capture':
                    result=camera.capture_full_frame(stopped.is_set,render=False) if hasattr(camera,'read_sensor_frame') else camera.capture_full_frame(stopped.is_set)
                elif op=='controls':
                    values=request['values']
                    result=camera.set_controls(values) if hasattr(camera,'set_controls') else {name:camera.set_control(name,value) for name,value in values.items()}
                elif op=='display':result=camera.set_display_mode(request['value'])
                elif op=='image':result=camera.set_image_options(request['options'])
                elif op=='gamma':result=camera.set_display_gamma(request['value'])
                elif op=='white':
                    import numpy as np
                    reference=request['raw'];raw=np.frombuffer(base64.b64decode(reference['data']),np.uint8).reshape(reference['shape'])
                    result=camera.balance_white(raw)
                elif op=='white-reset':result=camera.reset_white_balance()
                elif op=='fault':
                    reader=camera.reader
                    with reader.condition:
                        reader.frames.clear();reader.error=RuntimeError('USB async transfer failed: status=1, bytes=0 (injected validation fault)')
                        reader.stopping=True;reader.condition.notify_all()
                    result=None
                elif op=='close':camera.close();result=None
                else:raise ValueError('Unknown private camera command')
                reply={'ok':True,'result':result}
            except Exception as exc:reply={'ok':False,'error':str(exc),'interrupted':isinstance(exc,InterruptedError)}
            data=pickle.dumps(reply,protocol=5)
            sys.stdout.buffer.write(struct.pack('!I',len(data)));sys.stdout.buffer.write(data);sys.stdout.buffer.flush()
            if op=='close' or stopped.is_set():break
    except BrokenPipeError:
        # Parent stopped mid-frame. Discard buffered bytes, then close USB below.
        import os
        sink=os.open(os.devnull,os.O_WRONLY)
        try:os.dup2(sink,sys.stdout.fileno())
        finally:os.close(sink)
    finally:
        if camera is not None:camera.close()

if __name__=='__main__':main()
