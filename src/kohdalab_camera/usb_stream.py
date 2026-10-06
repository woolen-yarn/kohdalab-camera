"""Queued bulk I/O through PyUSB's libusb backend (private adapter boundary).

Transfers are cancelled and completed before their buffers or device are released.
Startup is synchronized at a short transfer; only exact-sized payloads become frames.
"""
from collections import deque
import ctypes as C
from threading import Condition, Event, Thread
import time
import sys
from .backends import now


class FrameAssembler:
    def __init__(self, frame_bytes, transfer_bytes):
        self.frame_bytes = frame_bytes
        self.transfer_bytes = transfer_bytes
        self.synced = False
        self.payload = bytearray()
        self.oversized = False
        self.discarded = 0

    def feed(self, data):
        boundary = len(data) < self.transfer_bytes
        frame = None
        if self.synced and not self.oversized:
            if len(self.payload) + len(data) > self.frame_bytes:
                self.oversized = True
                self.payload.clear()
            else:
                self.payload.extend(data)
                if len(self.payload) == self.frame_bytes:
                    frame = bytes(self.payload)
                    self.payload.clear()
        if boundary:
            if self.synced:
                if not self.oversized and len(self.payload) == self.frame_bytes:
                    frame = bytes(self.payload)
                elif self.payload or self.oversized:
                    self.discarded += 1
            self.synced = True
            self.oversized = False
            self.payload.clear()
        return frame


class _Timeval(C.Structure):
    _fields_ = [('tv_sec', C.c_long), ('tv_usec', C.c_long)]


class QueuedBulkReader:
    # Short reads keep WinUSB's packet scheduling flowing at the fast sensor
    # timing. Validated against every pixel of the camera's test pattern.
    transfer_bytes = 8192 if sys.platform=='win32' else 0x80000
    depth = 64 if sys.platform=='win32' else 32

    def __init__(self, device, frame_bytes):
        import usb.backend.libusb1 as adapter
        self.adapter = adapter
        self.device = device  # Retain the PyUSB handle if cancellation cannot finish.
        self.backend = device._ctx.backend
        self.lib = self.backend.lib
        self.handle = device._ctx.handle.handle
        self.winusb_raw_io = False
        if sys.platform == 'win32':
            self._configure_winusb_raw_io()
        self.lib.libusb_cancel_transfer.argtypes = [adapter._libusb_transfer_p]
        self.lib.libusb_cancel_transfer.restype = C.c_int
        self.lib.libusb_handle_events_timeout.argtypes = [C.c_void_p, C.POINTER(_Timeval)]
        self.lib.libusb_handle_events_timeout.restype = C.c_int
        self.assembler = FrameAssembler(frame_bytes, self.transfer_bytes)
        self.condition = Condition()
        self.ready = Event()
        self.stopping = False
        self.error = None
        self.active = set()
        self.transfers = []
        self.buffers = []
        self.tokens = {}
        self.completed = {}
        self.submitted = 0
        self.next_completion = 0
        self.sequence = 0
        self.dropped = 0
        self.frames = deque(maxlen=2)
        self.callback = adapter._libusb_transfer_cb_fn_p(self._completed)
        self.thread = Thread(target=self._run, name='camera-usb', daemon=True)

    def _configure_winusb_raw_io(self):
        """Bypass WinUSB's serialized read queue before submitting any transfers."""
        try:
            supports = self.lib.libusb_endpoint_supports_raw_io
            maximum = self.lib.libusb_get_max_raw_io_transfer_size
            enable = self.lib.libusb_endpoint_set_raw_io
        except AttributeError:
            return  # Older libusb and the fake test transport have no RAW_IO API.
        for function in (supports, maximum):
            function.argtypes = [C.c_void_p, C.c_uint8]
            function.restype = C.c_int
        enable.argtypes = [C.c_void_p, C.c_uint8, C.c_int]
        enable.restype = C.c_int
        supported = supports(self.handle, 0x82)
        if supported < 0:
            raise RuntimeError(f'WinUSB RAW_IO query failed: {supported}')
        if not supported:
            return
        limit = maximum(self.handle, 0x82)
        interface = self.device.get_active_configuration()[(0, 0)]
        packet = next(e.wMaxPacketSize & 0x7ff for e in interface if e.bEndpointAddress == 0x82)
        size = min(self.transfer_bytes, limit)
        size -= size % packet
        if size <= 0:
            raise RuntimeError(f'Invalid WinUSB RAW_IO transfer limit: {limit}')
        code = enable(self.handle, 0x82, 1)
        if code < 0:
            raise RuntimeError(f'WinUSB RAW_IO enable failed: {code}')
        self.transfer_bytes = size
        self.winusb_raw_io = True

    def _submit(self, pointer):
        key = C.addressof(pointer.contents)
        result = self.lib.libusb_submit_transfer(pointer)
        if result < 0:
            raise RuntimeError(f'USB submit failed: {result}')
        self.active.add(key)
        self.tokens[key] = self.submitted
        self.submitted += 1

    def _completed(self, pointer):
        # ctypes callbacks must never let Python exceptions escape into C.
        with self.condition:
            try:
                key = C.addressof(pointer.contents)
                self.active.discard(key)
                transfer = pointer.contents
                if self.stopping:
                    self.condition.notify_all()
                    return
                if transfer.status != 0:
                    raise RuntimeError(f'USB async transfer failed: status={transfer.status}, bytes={transfer.actual_length}')
                self.completed[self.tokens[key]] = C.string_at(transfer.buffer, transfer.actual_length)
                self._submit(pointer)
                while self.next_completion in self.completed:
                    data = self.completed.pop(self.next_completion)
                    self.next_completion += 1
                    frame = self.assembler.feed(data)
                    if frame is not None:
                        self.sequence += 1
                        if len(self.frames) == self.frames.maxlen:
                            self.dropped += 1
                        self.frames.append((self.sequence, now(), frame))
                        self.condition.notify_all()
            except Exception as exc:
                self.error = exc
                self.stopping = True
                self.condition.notify_all()

    def start(self):
        self.thread.start()
        if not self.ready.wait(3):
            self.close()
            raise RuntimeError('USB reader startup timed out')
        if self.error:
            self.close()
            raise self.error

    def _allocate(self):
        with self.condition:
            for _ in range(self.depth):
                pointer = self.lib.libusb_alloc_transfer(0)
                if not pointer:
                    raise MemoryError('libusb transfer allocation failed')
                self.transfers.append(pointer)
                buffer = C.create_string_buffer(self.transfer_bytes)
                self.buffers.append(buffer)
                transfer = pointer.contents
                transfer.dev_handle = self.handle
                transfer.endpoint = 0x82
                transfer.type = 2
                # Darwin's bulk timeout also limits periods without bus data.
                # Sensor exposure/startup can legitimately exceed that limit;
                # read() provides the deadline and close() cancels explicitly.
                transfer.timeout = 0
                transfer.length = self.transfer_bytes
                transfer.buffer = C.addressof(buffer)
                transfer.callback = self.callback
                transfer.user_data = None
                self._submit(pointer)

    def _run(self):
        cancellation_started = None
        try:
            self._allocate()
            self.ready.set()
            while True:
                with self.condition:
                    if self.stopping and cancellation_started is None:
                        cancellation_started = time.monotonic()
                        for pointer in self.transfers:
                            if C.addressof(pointer.contents) in self.active:
                                self.lib.libusb_cancel_transfer(pointer)
                    if self.stopping and not self.active:
                        break
                    if cancellation_started is not None and time.monotonic() - cancellation_started > 6:
                        raise RuntimeError('USB cancellation incomplete; retaining active buffers')
                result = self.lib.libusb_handle_events_timeout(self.backend.ctx, C.byref(_Timeval(0, 100000)))
                if result < 0 and result != -10:
                    with self.condition:
                        self.error = RuntimeError(f'USB event handling failed: {result}')
                        self.stopping = True
                        self.condition.notify_all()
        except Exception as exc:
            with self.condition:
                self.error = exc
                self.stopping = True
                # A setup failure may leave some requests pending: cancel and drain.
                for pointer in self.transfers:
                    if C.addressof(pointer.contents) in self.active:
                        self.lib.libusb_cancel_transfer(pointer)
            deadline = time.monotonic() + 6
            while self.active and time.monotonic() < deadline:
                self.lib.libusb_handle_events_timeout(self.backend.ctx, C.byref(_Timeval(0, 100000)))
        finally:
            # Never free a submitted transfer, even on cancellation failure.
            if not self.active:
                for pointer in self.transfers:
                    self.lib.libusb_free_transfer(pointer)
                self.transfers.clear()
                self.buffers.clear()
            with self.condition:
                self.ready.set()
                self.condition.notify_all()

    def read(self, timeout=10, cancelled=None):
        deadline = time.monotonic() + timeout
        with self.condition:
            while not self.frames:
                if cancelled is not None and cancelled():raise InterruptedError('Capture stopped')
                if self.error:
                    raise self.error
                if self.stopping:
                    raise RuntimeError('USB reader stopped')
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise RuntimeError('No complete USB frame before deadline')
                self.condition.wait(min(remaining,.1) if cancelled is not None else remaining)
            # Latest complete frame; preview/PNG compression never blocks USB reads.
            result = self.frames.pop()
            self.dropped += len(self.frames)
            self.frames.clear()
            return result

    def close(self):
        with self.condition:
            self.stopping = True
            self.condition.notify_all()
        self.thread.join(8)
        if self.thread.is_alive() or self.active:
            raise RuntimeError('USB transfers still active; device must remain open')
        self.device = None
