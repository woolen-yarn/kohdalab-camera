import pytest
from kohdalab_camera.process_camera import ProcessCamera

def test_frame_protocol_and_clean_child_shutdown():
    camera=ProcessCamera('simulated');camera.open();process=camera.process
    try:
        first=camera.read();second=camera.read()
        assert first.pixels.shape==(480,640,3)
        assert second.sequence>first.sequence
    finally:camera.close()
    assert camera.process is None and process.poll()==0

def test_child_crash_is_reported_and_resources_closed():
    camera=ProcessCamera('simulated');camera.open();process=camera.process
    process.kill();process.wait()
    with pytest.raises(RuntimeError,match='not running'):camera.read()
    camera.close()
    assert camera.process is None and process.stdout.closed

def test_stop_interrupts_read_and_reaps_child():
    camera=ProcessCamera('simulated');camera.open();process=camera.process
    with pytest.raises(InterruptedError):camera.read_interruptible(lambda:True)
    camera.close()
    assert process.poll() is not None and camera.process is None

def test_saved_frame_conditions_do_not_change_when_next_settings_apply():
    camera=ProcessCamera('simulated');camera.open()
    try:
        frame=camera.read()
        camera.set_control('exposure',25)
        assert frame.controls['exposure']==10
        assert camera.read().controls['exposure']==25
    finally:camera.close()



def test_child_exit_mid_response_is_reported_without_hanging():
    import subprocess,sys
    camera=ProcessCamera()
    camera.process=subprocess.Popen([sys.executable,'-c',"import sys;sys.stdin.buffer.readline();sys.stdout.buffer.write(b'xx');sys.stdout.buffer.flush()"],stdin=subprocess.PIPE,stdout=subprocess.PIPE)
    process=camera.process
    try:
        with pytest.raises(RuntimeError,match='exited during acquisition'):
            camera._rpc('read')
    finally:camera.close()
    assert process.poll() is not None and camera.process is None

def test_cancel_partial_response_reaps_blocked_reader():
    import subprocess,sys,time
    camera=ProcessCamera()
    camera.process=subprocess.Popen([sys.executable,'-c',
        "import sys,struct,time;sys.stdin.buffer.readline();"
        "sys.stdout.buffer.write(struct.pack('!I',1048576)+b'x'*4096);"
        "sys.stdout.buffer.flush();time.sleep(30)"],stdin=subprocess.PIPE,stdout=subprocess.PIPE)
    process=camera.process;start=time.monotonic()
    try:
        with pytest.raises(InterruptedError):camera._rpc('read',cancelled=lambda:time.monotonic()-start>.25)
        reader=camera.response_thread
    finally:camera.close()
    assert not reader.is_alive() and process.poll() is not None
    assert time.monotonic()-start<5

@pytest.mark.parametrize('size',[0,64*1024*1024+1])
def test_invalid_response_size_terminates_child_without_close_deadlock(size):
    import subprocess,sys
    camera=ProcessCamera()
    camera.process=subprocess.Popen([sys.executable,'-c',
        f"import sys,struct,time;sys.stdin.buffer.readline();sys.stdout.buffer.write(struct.pack('!I',{size}));sys.stdout.buffer.flush();time.sleep(30)"],
        stdin=subprocess.PIPE,stdout=subprocess.PIPE)
    process=camera.process
    try:
        with pytest.raises(RuntimeError,match='response size'):camera._rpc('read')
        reader=camera.response_thread
    finally:camera.close()
    assert not reader.is_alive() and process.poll() is not None

def test_sensor_transport_preserves_pixels_and_snapshot_conditions():
    import hashlib,numpy as np
    from kohdalab_camera.backends import SensorFrame
    raw=np.random.default_rng(42).integers(0,256,(65,67),dtype=np.uint8)
    controls={'display_mode':'color','bayer_pattern':'GRBG',
              'white_balance_rgb':np.array([1.351,.701,2.33],np.float32).tolist(),
              'display_gamma':2.2,'image_options':dict(brightness_ev=1.5,mirror=True,flip=True,rotation=90)}
    reply=SensorFrame(raw,123,'capture timestamp','legacy-usb','0547:4D33',controls)
    frame=ProcessCamera._materialize(reply)
    assert hashlib.sha256(frame.pixels.tobytes()).hexdigest()=='622fb1a09fc0df32e135dd57249f07172ec579722c5b0fa0ee46899925042b96'
    assert frame.sequence==123 and frame.timestamp=='capture timestamp'
    assert frame.controls==controls and np.array_equal(frame.sensor_raw,raw)
