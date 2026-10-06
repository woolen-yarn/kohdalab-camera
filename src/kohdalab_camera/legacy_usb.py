"""GR300/TCA USB backend, validated chip 0x1621 and GRBG USB parity."""
import copy,math,time,os,sys
import numpy as np
from .backends import Frame,SensorFrame
from .legacy_protocol import VID,PID,parse_register_reply


class LegacyUsbCamera:
    width,height=2048,1536
    def __init__(self):
        self.binning=int(os.environ.get("KOHDA_CAMERA_BINNING","2"))
        if self.binning not in (1,2):raise ValueError("Unsupported sensor binning")
        self.width,self.height=2048//self.binning,1536//self.binning
        self.capture_profile='balanced' if self.binning==2 else 'full'
        self.vertical_blanking=self._profile_blanking()
        self.horizontal_blanking=self._profile_horizontal_blanking()
        self.device=None
        self.claimed=False
        self.sequence=0
        self.chip=None
        self.controls={'exposure':{'reported':128,'unit':'sensor rows'},
                       'gain':{'reported':8,'unit':'raw register 0x35', 'verified':False}}
        self.trace=[]
        self.reader=None
        self.stream_sequence_base=0
        self.display_mode='color'
        self.white_balance=np.ones(3,dtype=np.float32)
        self.display_gamma=2.2
        self.image_options={'brightness_ev':1.5,'mirror':False,'flip':False,'rotation':180}
        self.bus_resets=0
        self.pipe_recoveries=0
        self.last_pipe_restart_error=None
        self.last_reset_error=None
        self.recoveries=0
        self.last_transport_error=None
        self.last_transport_state=None
        self.activity=None
        self.clock_register=int(os.environ.get('KOHDA_CAMERA_CLOCK','8000'),16)
        if self.clock_register not in (0x8000,0x8001,0x8002,0x8008):raise ValueError('Unsupported experimental camera clock')

    def _profile_blanking(self):
        # MT9T001 register 06 permits a minimum of three rows. Windows live
        # capture was losing throughput with the long 1023-row interframe gap.
        default=3 if self.capture_profile=='balanced' or sys.platform=='win32' else 1023
        value=int(os.environ.get('KOHDA_CAMERA_VBLANK',str(default)))
        if not 3<=value<=1023:raise ValueError('Invalid vertical blanking (3..1023 rows)')
        return value

    def set_capture_profile(self,profile):
        if self.device is not None:raise RuntimeError('Choose the camera mode before connecting')
        if profile not in ('balanced','full'):raise ValueError('Unknown capture profile')
        self.capture_profile=profile;self.binning=2 if profile=='balanced' else 1
        self.width,self.height=2048//self.binning,1536//self.binning
        self.vertical_blanking=self._profile_blanking()
        self.horizontal_blanking=self._profile_horizontal_blanking()

    def _profile_horizontal_blanking(self):
        default=(21 if self.capture_profile=='balanced' else 1024) if sys.platform=='win32' else 336
        value=int(os.environ.get('KOHDA_CAMERA_HBLANK',str(default)))
        if not 21<=value<=2047:raise ValueError('Invalid horizontal blanking (21..2047 clocks)')
        return value

    def set_display_gamma(self,gamma):
        if not math.isfinite(gamma) or not .5<=gamma<=3:raise ValueError('Gamma must be .5..3')
        self.display_gamma=float(gamma)

    def set_display_mode(self,mode):
        if mode not in ('color','gray'):raise ValueError('Unknown display mode')
        self.display_mode=mode

    def balance_white(self,raw):
        """Use a frame filled with a neutral white/gray surface as the reference."""
        from .color import demosaic
        rgb=demosaic(raw,'GRBG')[::8,::8].astype(np.float32)
        valid=np.all((rgb>8)&(rgb<245),axis=2)
        if valid.sum()<100:raise ValueError('Fill the view with a brighter neutral surface without clipping highlights.')
        levels=rgb[valid].mean(axis=0)
        self.white_balance=np.clip(levels[1]/levels,.25,4)

    def reset_white_balance(self):
        self.white_balance[:]=1

    def set_image_options(self,options):
        from .color import validate_image_options
        self.image_options=validate_image_options({**self.image_options,**options})

    def control(self,rt,request,value,index,data):
        result=self.device.ctrl_transfer(rt,request,value,index,data,timeout=1500)
        self.trace.append({'type':rt,'request':request,'value':value,'index':index,
                           'reply':bytes(result).hex() if rt&0x80 else result})
        return result

    def read_register(self,address):
        if not 0<=address<=0xffff:raise ValueError('Invalid sensor address')
        return parse_register_reply(self.control(0xc0,0x0a,0,address,3))

    def capture_transport_state(self):
        registers={}
        for address in (7,8,9,0x35,10,5,6,0x1e):
            try:registers[f'{address:02X}']=f'{self.read_register(address):04X}'
            except Exception as exc:
                return {'registers':registers,'control_error':str(exc)}
        return {'registers':registers,'control_endpoint_responsive':True}

    def write_register(self,address,value):
        # Critical: wValue is the data, wIndex is the sensor address.
        ack=bytes(self.control(0xc0,0x0b,value,address,1))
        if ack!=b'\x08':raise RuntimeError(f'Sensor write {address:02X} rejected: {ack.hex()}')

    def open(self):
        import usb.core,usb.util,usb.backend.libusb1
        import libusb_package
        if self.device is not None:raise RuntimeError('Camera already open')
        backend=usb.backend.libusb1.get_backend(find_library=libusb_package.find_library)
        if backend is None:raise RuntimeError('libusb unavailable; install usb extra')
        self.device=usb.core.find(idVendor=VID,idProduct=PID,backend=backend)
        if self.device is None:raise RuntimeError('0547:4D33 is not visible to libusb. On Windows, run Camera USB Setup to assign WinUSB; on Mac, use the normal terminal.')
        try:
            from .mac_activity import CaptureActivity
            self.activity=CaptureActivity()
            interface=self.device.get_active_configuration()[(0,0)]
            if interface.bInterfaceClass!=0xff or not any(e.bEndpointAddress==0x82 and e.bmAttributes&3==2 for e in interface):
                raise RuntimeError('Unexpected camera USB descriptors')
            usb.util.claim_interface(self.device,0);self.claimed=True
            for value in (1,0,1):
                self.control(0x40,1,value,15,b'');time.sleep(.1)
            # Matches the driver's reset-pipe/clear-stall operation; also resets
            # host endpoint state after cancelled reads from an earlier stream.
            self.device.clear_halt(0x82)
            self.write_register(7,2)
            self.chip=parse_register_reply(self.control(0xc0,0x0a,0,0,3))
            if self.chip!=0x1621:raise RuntimeError(f'Unvalidated chip {self.chip:04X}; only 1621 enabled')
            self.write_register(10,0x8000)
            self.write_register(13,1);time.sleep(.1)
            self.write_register(13,0);time.sleep(.1)
            for address,value in [(1,0x15),(2,0x21),(0x20,0),(0x1e,0x8040),(0x4e,0x20),
                (4,0x7ff),(3,0x5ff),(0x2b,0x60),(0x2c,0x460),(0x2d,0x60),(0x2e,0x60),
                (10,self.clock_register),(0x49,0x80),(0x22,0x11 if self.binning==2 else 0),(0x23,0x11 if self.binning==2 else 0),(8,0),(9,self.controls['exposure'].get('requested',self.controls['exposure'].get('reported',128))),(5,self.horizontal_blanking),(6,self.vertical_blanking)]:
                self.write_register(address,value)
            # Conservative analog gain avoids saturation; the vendor fast pixel clock keeps the bridge responsive.
            self.write_register(0x35,self.controls['gain'].get('requested',self.controls['gain'].get('reported',8)))
            if self.read_register(6)!=self.vertical_blanking:
                raise RuntimeError('Vertical blanking register did not retain the capture profile')
            if self.read_register(5)!=self.horizontal_blanking:
                raise RuntimeError('Horizontal blanking register did not retain the capture profile')
            for name,address in (('exposure',9),('gain',0x35)):
                applied=self.read_register(address)
                desired=self.controls[name].get('requested',self.controls[name].get('reported'))
                if applied!=desired:raise RuntimeError(f'{name} readback {applied} does not match {desired}')
                self.controls[name]['readback']=applied
                self.controls[name]['register_verified']=True
            time.sleep(.4)
            from .usb_stream import QueuedBulkReader
            self.stream_sequence_base=self.sequence
            self.reader=QueuedBulkReader(self.device,self.width*self.height)
            self.reader.start()
            self.control(0x40,1,3,15,b'')
            for name in ('exposure','gain'):
                self.controls[name]['acknowledged']=True
        except Exception:
            self.close()
            raise

    def read_interruptible(self,stop_requested):
        return self.read(stop_requested)

    def read_sensor_frame(self,stop_requested=None):
        return self.read(stop_requested,render=False)

    def read(self,stop_requested=None,recover=True,render=True):
        if self.device is None:raise RuntimeError('Camera is closed')
        if self.reader is None:raise RuntimeError('USB reader unavailable')
        exposure=self.controls['exposure'].get('requested',self.controls['exposure'].get('reported',128))
        full_restarts=0;pipe_attempts=0;pipe_deadline=None
        for attempt in range(36 if recover else 1):
            try:
                sequence,timestamp,payload=self.reader.read(timeout=max(10,exposure*.0004+3),cancelled=stop_requested)
                break
            except RuntimeError as exc:
                pipe_error=any(marker in str(exc) for marker in ('USB async transfer failed: status=1','USB async transfer failed: status=4','USB submit failed: -9'))
                transient=pipe_error or 'No complete USB frame' in str(exc)
                if not transient or not recover or full_restarts>=3:raise
                if stop_requested is not None and stop_requested():raise InterruptedError('Capture stopped')
                self.last_transport_error=str(exc)
                self.last_transport_state=self.capture_transport_state()
                if pipe_deadline is None:pipe_deadline=time.monotonic()+2
                if (pipe_error and pipe_attempts<32
                    and time.monotonic()<pipe_deadline
                    and self.last_transport_state.get('control_endpoint_responsive')
                    and self.restart_image_pipe()):
                    pipe_attempts+=1
                    continue
                pipe_attempts=32
                full_restarts+=1
                # Cancel/drain before closing the handle. A stale bridge stream can
                # stop responding on macOS; a fresh volatile initialization recovers it.
                device=self.device
                self.close()
                # Reset the owned device only after all transfers and handles close.
                # Reinitializing registers alone can leave a failed USB pipe alive.
                self.reset_transport(device)
                # High exposure/gain profiles have stalled the image pipe in this
                # setup; the control endpoint can remain responsive. Return to the measured stable profile, preserving the request
                # in metadata instead of repeatedly reopening an unstable profile.
                if exposure>128 or self.controls['gain'].get('requested',self.controls['gain'].get('reported',8))!=8:
                    self.controls['rejected_hardware_profile']=copy.deepcopy({name:self.controls[name] for name in ('exposure','gain')})
                    self.controls['exposure']={'requested':128,'unit':'sensor rows','acknowledged':False}
                    self.controls['gain']={'requested':8,'unit':'raw register 0x35','acknowledged':False}
                    exposure=128
                time.sleep(.3*full_restarts)
                self.open()
                self.recoveries+=1
        raw=np.frombuffer(payload,dtype=np.uint8).reshape(self.height,self.width).copy()
        self.sequence=self.stream_sequence_base+sequence
        controls={**copy.deepcopy(self.controls),'chip_id':f'{self.chip:04X}',
                  'pixel_clock_register':f'{self.clock_register:04X}',
                  'transport':f'{self.reader.depth} queued {self.reader.transfer_bytes}-byte transfers; short-packet frame boundary',
                  'winusb_raw_io':self.reader.winusb_raw_io,
                  'discarded_usb_frames':self.reader.assembler.discarded,
                  'dropped_preview_frames':self.reader.dropped,
                  'capture_profile':self.capture_profile,'vertical_blanking':self.vertical_blanking,'horizontal_blanking':self.horizontal_blanking,'sensor_binning':self.binning,'pixel_format':'8bit Bayer GRBG', 'bayer_pattern':'GRBG',
                  'display_mode':self.display_mode,'white_balance_rgb':self.white_balance.tolist(),
                  'display_gamma':self.display_gamma,
                  'image_options':copy.deepcopy(self.image_options),
                  'usb_recoveries':self.recoveries,'last_transport_error':self.last_transport_error,
                  'last_transport_state':copy.deepcopy(self.last_transport_state),
                  'usb_bus_resets':self.bus_resets,'last_reset_error':self.last_reset_error,
                  'usb_pipe_recoveries':self.pipe_recoveries,'last_pipe_restart_error':self.last_pipe_restart_error,
                  'bayer_validation':'MT9T001 color-gain response on connected camera'}
        if not render:return SensorFrame(raw,self.sequence,timestamp,'legacy-usb',f'{VID:04X}:{PID:04X}',controls)
        from .color import render_sensor
        rgb=render_sensor(raw,self.display_mode,'GRBG',self.white_balance,self.display_gamma,**self.image_options)
        frame=Frame(rgb,self.sequence,timestamp,'legacy-usb',f'{VID:04X}:{PID:04X}',controls,raw)
        return frame

    def capture_full_frame(self,stop_requested=None,render=True):
        if self.device is None:raise RuntimeError('Camera is closed')
        if self.capture_profile=='full':return self.read(stop_requested,render=render)
        original=self.capture_profile
        self.close()
        try:
            self.set_capture_profile('full');self.open()
            return self.read(stop_requested,recover=False,render=render)
        finally:
            self.close()
            self.set_capture_profile(original)
            if stop_requested is None or not stop_requested():self.open()

    @staticmethod
    def _validate_control(name,value):
        if not math.isfinite(value) or value!=int(value):
            raise ValueError('Legacy controls require integer register values')
        if name=='exposure' and 1<=value<=1536:return 'sensor rows'
        if name=='gain' and (8<=value<=32 or 80<=value<=96):return 'MT9T001 analog gain register code'
        raise ValueError('Live exposure: 1..1536 rows. Gain: 1..8x analog. Long integration is not supported in live mode.')

    def restart_image_pipe(self):
        """Drain failed reads, clear endpoint stall, preserve sensor and bridge."""
        if not hasattr(self.device,'clear_halt') or not hasattr(self.reader,'close'):return False
        self.reader.close()  # Never clear/release while callbacks own the buffers.
        self.reader=None
        try:
            self.device.clear_halt(0x82)
            from .usb_stream import QueuedBulkReader
            self.stream_sequence_base=self.sequence
            self.reader=QueuedBulkReader(self.device,self.width*self.height)
            self.reader.start()
        except Exception as exc:
            self.last_pipe_restart_error=str(exc)
            if self.reader is not None:
                self.reader.close()  # Cancellation failure must propagate; keep handle.
                self.reader=None
            return False
        self.pipe_recoveries+=1
        self.recoveries+=1
        self.last_pipe_restart_error=None
        return True

    def reset_transport(self,device):
        if not hasattr(device,'reset'):return
        try:
            device.reset();self.bus_resets+=1;self.last_reset_error=None
        except Exception as reset_error:self.last_reset_error=str(reset_error)
        finally:
            import usb.util
            usb.util.dispose_resources(device)

    def set_control(self,name,value):
        return self.set_controls({name:value})[name]

    def configure_controls(self,values):
        """Validate desired settings before connection; open() applies them."""
        if self.device is not None:raise RuntimeError('Configure before opening the camera')
        updates={}
        for name,value in values.items():
            unit=self._validate_control(name,value)
            updates[name]={'requested':int(value),'unit':unit,'acknowledged':False}
        self.controls.update(updates)

    def set_controls(self,values):
        if self.device is None:raise RuntimeError('Camera is closed')
        updates={}
        for name,value in values.items():
            unit=self._validate_control(name,value)
            updates[name]={'requested':int(value),'acknowledged':True,'unit':unit,
                           'optical_effect_verified':False,
                           'note':'Transfer cancelled and camera reinitialized before applying; ms calibration unverified'}
        if self.reader is not None:
            previous=copy.deepcopy(self.controls)
            if all(int(value)==self.controls[name].get('requested',self.controls[name].get('reported')) for name,value in values.items()):return self.controls.copy()
            self.close()
            self.controls.pop('rejected_hardware_profile',None)
            self.controls.update(updates)
            try:self.open()
            except Exception:
                self.controls=previous
                self.open()
                raise
        else:
            # Control-only path, also used by the isolated protocol tests.
            for name,value in values.items():
                if name=='exposure':
                    self.write_register(8,0);self.write_register(9,int(value))
                else:self.write_register(0x35,int(value))
            self.controls.update(updates)
        return updates

    def close(self):
        if self.device is None:return
        if self.reader is not None:
            # Completion callbacks must finish before releasing the device handle.
            self.reader.close()
            self.reader=None
        import usb.util
        device=self.device
        errors=[]
        try:
            for args in [(0xc0,0xb,0,7,1),(0x40,1,1,15,b''),(0x40,1,0,15,b'')]:
                try:self.control(*args)
                except Exception as exc:errors.append(str(exc))
            if self.claimed:
                try:usb.util.release_interface(device,0)
                except Exception as exc:errors.append(str(exc))
        finally:
            usb.util.dispose_resources(device)
            self.device=None;self.claimed=False
            if self.activity is not None:
                self.activity.close();self.activity=None
        if errors:raise RuntimeError(f'Camera cleanup failed: {errors}')
