"""Bounded Foundation activity for real-time capture, without changing preferences."""
import ctypes as C
import sys

class CaptureActivity:
    # NSActivityUserInitiatedAllowingIdleSystemSleep | NSActivityLatencyCritical.
    # Values from Foundation NSProcessInfo.h. Idle/display sleep remain permitted.
    options=0x00EFFFFF | 0xFF00000000

    def __init__(self):
        self.token=None
        if sys.platform!='darwin':return
        self.foundation=C.CDLL('/System/Library/Frameworks/Foundation.framework/Foundation')
        self.objc=C.CDLL('/usr/lib/libobjc.A.dylib')
        for name,args,result in (
            ('objc_getClass',[C.c_char_p],C.c_void_p),
            ('sel_registerName',[C.c_char_p],C.c_void_p),
            ('objc_retain',[C.c_void_p],C.c_void_p),
            ('objc_release',[C.c_void_p],None),
            ('objc_autoreleasePoolPush',[],C.c_void_p),
            ('objc_autoreleasePoolPop',[C.c_void_p],None)):
            f=getattr(self.objc,name);f.argtypes=args;f.restype=result
        address=C.cast(self.objc.objc_msgSend,C.c_void_p).value
        self.send=C.CFUNCTYPE(C.c_void_p,C.c_void_p,C.c_void_p)(address)
        self.send_string=C.CFUNCTYPE(C.c_void_p,C.c_void_p,C.c_void_p,C.c_char_p)(address)
        self.begin=C.CFUNCTYPE(C.c_void_p,C.c_void_p,C.c_void_p,C.c_uint64,C.c_void_p)(address)
        self.end=C.CFUNCTYPE(None,C.c_void_p,C.c_void_p,C.c_void_p)(address)
        pool=self.objc.objc_autoreleasePoolPush()
        try:
            self.process=self.send(self.objc.objc_getClass(b'NSProcessInfo'),self.sel(b'processInfo'))
            reason=self.send_string(self.objc.objc_getClass(b'NSString'),self.sel(b'stringWithUTF8String:'),b'KohdaLab real-time USB camera capture')
            token=self.begin(self.process,self.sel(b'beginActivityWithOptions:reason:'),self.options,reason)
            if not token:raise RuntimeError('macOS capture activity unavailable')
            self.token=self.objc.objc_retain(token)
        finally:self.objc.objc_autoreleasePoolPop(pool)

    def sel(self,name):return self.objc.sel_registerName(name)

    def close(self):
        if self.token is None:return
        pool=self.objc.objc_autoreleasePoolPush()
        try:
            self.end(self.process,self.sel(b'endActivity:'),self.token)
            self.objc.objc_release(self.token)
            self.token=None
        finally:self.objc.objc_autoreleasePoolPop(pool)
