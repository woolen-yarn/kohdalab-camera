"""Bilinear Bayer conversion; the original sensor bytes are never modified."""
import numpy as np
from functools import lru_cache
from pathlib import Path
import ctypes,sys

@lru_cache(maxsize=1)
def _native_renderer():
    name='camera-render.dll' if sys.platform=='win32' else 'libcamera_render.dylib'
    candidates=[Path(__file__).parent/'_native'/name,Path(__file__).parents[2]/'build/camera-render'/name]
    for path in candidates:
        if not path.is_file():continue
        library=ctypes.CDLL(str(path))
        if library.kohda_render_api_version()!=1:raise RuntimeError('Unsupported camera renderer ABI')
        function=library.kohda_render
        function.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_int,
                           ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_void_p]
        function.restype=ctypes.c_int
        return function
    return None

@lru_cache(maxsize=64)
def _render_lut(white_balance,balance_dtype,gamma,brightness_ev):
    # Preserve the original uint8 truncation after each correction stage.
    values=np.tile(np.arange(256,dtype=np.uint8)[:,None],(1,3))
    if white_balance!=(1,1,1):
        values=np.clip(values.astype(np.float32)*np.asarray(white_balance,dtype=balance_dtype),0,255).astype(np.uint8)
    if brightness_ev:
        values=np.clip(values.astype(np.float32)*2**brightness_ev,0,255).astype(np.uint8)
    if gamma!=1:
        lut=np.round((np.arange(256)/255)**(1/gamma)*255).astype(np.uint8)
        values=lut[values]
    values.flags.writeable=False
    return values

def render_sensor(raw,mode='color',pattern='GRBG',white_balance=(1,1,1),gamma=2.2,brightness_ev=0.0,mirror=False,flip=False,rotation=0):
    if mode not in ('color','gray') or not .5<=gamma<=3:raise ValueError('Invalid display settings')
    validate_image_options(dict(brightness_ev=brightness_ev,mirror=mirror,flip=flip,rotation=rotation))
    balance=tuple(white_balance) if mode=='color' else (1,1,1)
    native=_native_renderer()
    if native is not None and raw.dtype==np.uint8 and raw.ndim==2 and min(raw.shape)>=2:
        if pattern not in ('GRBG','GBRG','RGGB','BGGR'):raise ValueError('Unsupported Bayer pattern')
        height,width=raw.shape
        shape=(width,height,3) if rotation in (90,270) else (height,width,3)
        output=np.empty(shape,dtype=np.uint8)
        sensor=np.ascontiguousarray(raw)
        lut=_render_lut(balance,np.asarray(white_balance).dtype.str,gamma,brightness_ev)
        result=native(sensor.ctypes.data,width,height,('GRBG','GBRG','RGGB','BGGR').index(pattern),
                      int(mode=='gray'),lut.ctypes.data,int(mirror),int(flip),rotation,output.ctypes.data)
        if result:raise RuntimeError('Camera renderer rejected the frame')
        return output
    rgb=demosaic(raw,pattern) if mode=='color' else np.repeat(raw[:,:,None],3,axis=2)
    if balance!=(1,1,1) or brightness_ev or gamma!=1:
        lut=_render_lut(balance,np.asarray(white_balance).dtype.str,gamma,brightness_ev)
        for channel in range(3):rgb[:,:,channel]=lut[rgb[:,:,channel],channel]
    if mirror:rgb=rgb[:,::-1]
    if flip:rgb=rgb[::-1]
    if rotation:rgb=np.rot90(rgb,-rotation//90)
    return np.ascontiguousarray(rgb)

def demosaic(raw,pattern):
    if pattern not in ('GRBG','GBRG','RGGB','BGGR'):raise ValueError('Unsupported Bayer pattern')
    if raw.ndim!=2 or min(raw.shape)<2:raise ValueError('Expected a 2D sensor image')
    data=raw.astype(np.uint16)
    p=np.pad(data,1,mode='reflect')
    horizontal=p[1:-1,:-2]+p[1:-1,2:]
    vertical=p[:-2,1:-1]+p[2:,1:-1]
    diagonal=(p[:-2,:-2]+p[:-2,2:]+p[2:,:-2]+p[2:,2:])>>2
    rgb=np.empty((*raw.shape,3),dtype=np.uint8)
    for channel,color in enumerate('RGB'):
        plane=rgb[:,:,channel]
        if color=='G':
            plane[:]=(horizontal+vertical)>>2
            for index,site in enumerate(pattern):
                if site=='G':plane[index//2::2,index%2::2]=data[index//2::2,index%2::2]
        else:
            row,col=divmod(pattern.index(color),2)
            plane[row::2,col::2]=data[row::2,col::2]
            plane[row::2,1-col::2]=horizontal[row::2,1-col::2]>>1
            plane[1-row::2,col::2]=vertical[1-row::2,col::2]>>1
            plane[1-row::2,1-col::2]=diagonal[1-row::2,1-col::2]
    return rgb


def validate_image_options(options):
    import math
    if set(options)-{'brightness_ev','mirror','flip','rotation'}:raise ValueError('Unknown image option')
    if not math.isfinite(options['brightness_ev']) or not -3<=options['brightness_ev']<=4:raise ValueError('Brightness must be -3..4 EV')
    if options['rotation'] not in (0,90,180,270):raise ValueError('Rotation must be 0, 90, 180 or 270 degrees')
    if not isinstance(options['mirror'],bool) or not isinstance(options['flip'],bool):raise ValueError('Flip options must be boolean')
    return dict(options)

def gain_to_register(factor):
    if not 1<=factor<=8:raise ValueError('Gain must be 1..8x')
    return round(factor*8) if factor<=4 else 64+round(factor*4)

def register_to_gain(code):
    return ((code>>6 & 1)+1)*(code&63)/8
