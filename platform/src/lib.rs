//! Windows capability wrapper. No device APIs. All unsafe is confined here.
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg(windows)]
use std::{
    io,
    mem::{size_of, zeroed},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Cryptography::*, SECURITY_ATTRIBUTES},
    Storage::FileSystem::*,
    System::{Pipes::*, Threading::*, IO::CancelSynchronousIo},
};

fn check(ok: i32) -> io::Result<()> {
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn raw(h: &OwnedHandle) -> HANDLE {
    h.as_raw_handle()
}
// SAFETY invariant O: only successful APIs producing a new, non-null owned handle call this.
unsafe fn own(h: HANDLE) -> OwnedHandle {
    unsafe { OwnedHandle::from_raw_handle(h) }
}
// No safe raw adoption. Reader/Writer only hold validated synchronous directional pipes.
#[derive(Clone, Default)]
pub struct Cancel(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Release);
    }
    pub fn cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn reject<T>(&self, kind: io::ErrorKind) -> io::Result<T> {
        self.cancel();
        Err(kind.into())
    }
}
pub fn timely(cancel: &Cancel, now: Instant, deadline: Instant) -> io::Result<()> {
    if cancel.cancelled() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    if now >= deadline {
        return cancel.reject(io::ErrorKind::TimedOut);
    }
    Ok(())
}
pub struct Reader(Option<OwnedHandle>, Cancel, Option<IoWorker>);
pub struct Writer(Option<OwnedHandle>, Cancel, Option<IoWorker>);
impl Writer {
    pub fn close(&mut self) {
        self.2.take();
        self.0.take();
    }
}
impl Reader {
    pub fn close(&mut self) {
        self.2.take();
        self.0.take();
    }
}

// A single dedicated worker per endpoint, created lazily. Capacity-one request
// and result queues prevent backlog. Session receipt payloads use bounded bulk
// chunks; priority STOP and challenge/response have separate endpoint workers.
// The worker retains its native handle and buffer until the synchronous call
// actually completes. Timeout initiates cancellation; it never proves release.
static QUALIFICATION_POST_READ_PAUSE_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
struct Request {
    storage: Vec<u8>,
    write: bool,
    deadline: Instant,
    cancel: Cancel,
}
struct Completion {
    result: io::Result<usize>,
    storage: Vec<u8>,
}
struct IoWorker {
    requests: Option<std::sync::mpsc::SyncSender<Request>>,
    results: std::sync::mpsc::Receiver<Completion>,
    thread: Option<std::thread::JoinHandle<()>>,
}
static WORKERS_STARTED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static WORKERS_JOINED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static REQUESTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static REQUEST_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Native requests / request-buffer allocation bytes (cumulative, parent process).
pub fn transfer_metrics() -> (u64, u64) {
    (
        REQUESTS.load(std::sync::atomic::Ordering::Acquire),
        REQUEST_BYTES.load(std::sync::atomic::Ordering::Acquire),
    )
}
impl IoWorker {
    fn new(handle: &OwnedHandle) -> io::Result<Self> {
        let handle = handle.try_clone()?;
        let (requests, input) = std::sync::mpsc::sync_channel::<Request>(1);
        let (output, results) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("continuous-pipe".into())
            .spawn(move || {
                while let Ok(mut request) = input.recv() {
                    let result = (|| {
                        timely(&request.cancel, Instant::now(), request.deadline)?;
                        let mut n = 0;
                        // SAFETY: synchronous directional pipe, initialized owned
                        // allocation and count stay live on this worker through the
                        // native return. All callers enforce a DWORD-sized bound.
                        let ok = unsafe {
                            if request.write {
                                WriteFile(
                                    raw(&handle),
                                    request.storage.as_ptr(),
                                    request.storage.len() as u32,
                                    &mut n,
                                    null_mut(),
                                )
                            } else {
                                ReadFile(
                                    raw(&handle),
                                    request.storage.as_mut_ptr(),
                                    request.storage.len() as u32,
                                    &mut n,
                                    null_mut(),
                                )
                            }
                        };
                        let result = if ok == 0 {
                            Err(io::Error::last_os_error())
                        } else {
                            Ok(n as usize)
                        };
                        if !request.write {
                            let ms = QUALIFICATION_POST_READ_PAUSE_MS
                                .swap(0, std::sync::atomic::Ordering::AcqRel);
                            if ms != 0 {
                                std::thread::sleep(Duration::from_millis(ms));
                            }
                        }
                        timely(&request.cancel, Instant::now(), request.deadline)?;
                        result
                    })();
                    if output
                        .send(Completion {
                            result,
                            storage: request.storage,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })?;
        WORKERS_STARTED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Ok(Self {
            requests: Some(requests),
            results,
            thread: Some(thread),
        })
    }
    fn interrupt(&self) -> io::Result<()> {
        if let Some(thread) = &self.thread {
            // SAFETY: owned live thread handle, retained until join.
            if unsafe { CancelSynchronousIo(thread.as_raw_handle()) } == 0 {
                let e = io::Error::last_os_error();
                if e.raw_os_error() != Some(ERROR_NOT_FOUND as i32) {
                    return Err(e);
                }
            }
        }
        Ok(())
    }
}
impl Drop for IoWorker {
    fn drop(&mut self) {
        self.requests.take(); // wake an idle worker; never detach a native call
        if let Some(thread) = self.thread.as_ref() {
            while !thread.is_finished() {
                let _ = self.interrupt();
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
            WORKERS_JOINED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }
    }
}
fn transfer(
    h: &OwnedHandle,
    slot: &mut Option<IoWorker>,
    buf: &mut [u8],
    write: bool,
    deadline: Instant,
    cancel: &Cancel,
    expected_eof: bool,
) -> io::Result<usize> {
    timely(cancel, Instant::now(), deadline)?;
    if buf.len() > 1_048_576 {
        return cancel.reject(io::ErrorKind::InvalidInput);
    }
    if slot.is_none() {
        *slot = Some(IoWorker::new(h)?);
    }
    let worker = slot.as_mut().ok_or(io::ErrorKind::Other)?;
    REQUESTS.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    REQUEST_BYTES.fetch_add(buf.len() as u64, std::sync::atomic::Ordering::AcqRel);
    worker
        .requests
        .as_ref()
        .ok_or(io::ErrorKind::BrokenPipe)?
        .try_send(Request {
            storage: buf.to_vec(),
            write,
            deadline,
            cancel: cancel.clone(),
        })
        .map_err(|_| io::Error::other("pipe request queue unavailable"))?;
    let mut failure = None;
    let completion = loop {
        match worker.results.recv_timeout(Duration::from_millis(1)) {
            Ok(c) => break c,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return cancel.reject(io::ErrorKind::BrokenPipe)
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
        }
        if failure.is_none() {
            if let Err(e) = timely(cancel, Instant::now(), deadline) {
                failure = Some(e);
            }
        }
        if failure.is_some() {
            // Repeat through the check/issue race. Preserve the first failure.
            let _ = worker.interrupt();
        }
    };
    if let Some(e) = failure {
        cancel.cancel();
        slot.take();
        return Err(e);
    }
    if let Err(e) = timely(cancel, Instant::now(), deadline) {
        slot.take();
        return Err(e);
    }
    match completion.result {
        Ok(n) if n <= buf.len() => {
            buf[..n].copy_from_slice(&completion.storage[..n]);
            Ok(n)
        }
        Ok(_) => cancel.reject(io::ErrorKind::InvalidData),
        Err(e) => {
            if !expected_eof || e.raw_os_error() != Some(ERROR_BROKEN_PIPE as i32) {
                cancel.cancel();
            }
            slot.take(); // native completion received; worker now joined
            Err(e)
        }
    }
}
impl Writer {
    pub fn write(&mut self, bytes: &[u8], deadline: Instant, cancel: &Cancel) -> io::Result<()> {
        if self.1.cancelled() {
            return cancel.reject(io::ErrorKind::Interrupted);
        }
        let result = (|| {
            if bytes.len() > 1_048_576 {
                return cancel.reject(io::ErrorKind::InvalidInput);
            }
            let mut copy = bytes.to_vec();
            let result = transfer(
                self.0.as_ref().ok_or(io::ErrorKind::BrokenPipe)?,
                &mut self.2,
                &mut copy,
                true,
                deadline,
                cancel,
                false,
            );
            match result {
                Ok(n) if n == bytes.len() => Ok(()),
                Ok(_) => cancel.reject(io::ErrorKind::WriteZero),
                Err(e) => Err(e),
            }
        })();
        if result.is_err() {
            self.1.cancel();
            cancel.cancel();
        }
        result
    }
}
impl Reader {
    /// Bounded bulk payload read, with the same retained native IO owner as
    /// records. Caller validates closed dimensions before allocating storage.
    pub fn read_exact(
        &mut self,
        bytes: &mut [u8],
        deadline: Instant,
        cancel: &Cancel,
    ) -> io::Result<()> {
        if bytes.len() > 1_048_576 || self.1.cancelled() {
            return cancel.reject(io::ErrorKind::InvalidInput);
        }
        let mut used = 0;
        while used < bytes.len() {
            let result = transfer(
                self.0.as_ref().ok_or(io::ErrorKind::BrokenPipe)?,
                &mut self.2,
                &mut bytes[used..],
                false,
                deadline,
                cancel,
                false,
            );
            let n = match result {
                Ok(n) => n,
                Err(error) => {
                    self.1.cancel();
                    return Err(error);
                }
            };
            if n == 0 {
                self.1.cancel();
                return cancel.reject(io::ErrorKind::UnexpectedEof);
            }
            used += n;
        }
        Ok(())
    }
    pub fn record(&mut self, deadline: Instant, cancel: &Cancel) -> io::Result<[u8; 128]> {
        if self.1.cancelled() {
            return cancel.reject(io::ErrorKind::Interrupted);
        }
        let result = (|| {
            let mut b = [0; 128];
            let mut used = 0;
            while used < b.len() {
                let n = transfer(
                    self.0.as_ref().ok_or(io::ErrorKind::BrokenPipe)?,
                    &mut self.2,
                    &mut b[used..],
                    false,
                    deadline,
                    cancel,
                    false,
                )?;
                if n == 0 {
                    return cancel.reject(io::ErrorKind::UnexpectedEof);
                }
                used += n;
            }
            timely(cancel, Instant::now(), deadline)?;
            Ok(b)
        })();
        if result.is_err() {
            self.1.cancel();
            cancel.cancel();
        }
        result
    }
    pub fn expect_eof(&mut self, deadline: Instant, cancel: &Cancel) -> io::Result<()> {
        if self.1.cancelled() {
            return cancel.reject(io::ErrorKind::Interrupted);
        }
        let result = (|| {
            let mut b = [0];
            match transfer(
                self.0.as_ref().ok_or(io::ErrorKind::BrokenPipe)?,
                &mut self.2,
                &mut b,
                false,
                deadline,
                cancel,
                true,
            ) {
                Err(e) if e.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) => Ok(()),
                Ok(0) => Ok(()),
                Ok(_) => cancel.reject(io::ErrorKind::InvalidData),
                Err(e) => Err(e),
            }
        })();
        self.1.cancel();
        if result.is_err() {
            cancel.cancel();
        }
        if result.is_ok() {
            self.close();
        }
        result
    }
}
fn pipe(child_reads: bool) -> io::Result<(Reader, Writer)> {
    let mut r = null_mut();
    let mut w = null_mut();
    let sa = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    // SAFETY P: initialized outputs and security attributes; ownership immediately wrapped before fallible operations.
    unsafe {
        check(CreatePipe(&mut r, &mut w, &sa, 4096))?;
        let r = own(r);
        let w = own(w);
        check(SetHandleInformation(
            if child_reads { raw(&w) } else { raw(&r) },
            HANDLE_FLAG_INHERIT,
            0,
        ))?;
        Ok((
            Reader(Some(r), Cancel::default(), None),
            Writer(Some(w), Cancel::default(), None),
        ))
    }
}
pub fn random() -> io::Result<[u8; 32]> {
    let mut b = [0; 32];
    // SAFETY R: initialized output valid for length; system-preferred provider needs no algorithm handle.
    let status = unsafe {
        BCryptGenRandom(
            null_mut(),
            b.as_mut_ptr(),
            32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status < 0 {
        return Err(io::Error::other("BCryptGenRandom failed"));
    }
    Ok(b)
}
/// SHA256 of bounded canonical session bytes; no textual secret formatting.
pub fn digest(bytes: &[u8]) -> io::Result<[u8; 32]> {
    if bytes.len() > 4096 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut algorithm = null_mut();
    let mut output = [0u8; 32];
    // SAFETY: initialized outputs, constant NUL terminated algorithm name,
    // borrowed input and output live through the synchronous calls.
    unsafe {
        if BCryptOpenAlgorithmProvider(&mut algorithm, BCRYPT_SHA256_ALGORITHM, null(), 0) < 0 {
            return Err(io::Error::other("SHA256 provider"));
        }
        let result = BCryptHash(
            algorithm,
            null(),
            0,
            bytes.as_ptr(),
            bytes.len() as u32,
            output.as_mut_ptr(),
            32,
        );
        BCryptCloseAlgorithmProvider(algorithm, 0);
        if result < 0 {
            return Err(io::Error::other("SHA256 failed"));
        }
    }
    Ok(output)
}

/// Exact-file release primitive. Drop only closes; an unreleased interlock
/// persists. This primitive does not decide whether cleanup/reap proof exists.
/// The coordinator must retain this object throughout the two-phase protocol.
pub struct RetainedInterlock(Option<std::fs::File>);
impl RetainedInterlock {
    pub fn create_new(path: &std::path::Path, generation: &[u8]) -> io::Result<Self> {
        use std::{io::Write, os::windows::fs::OpenOptionsExt};
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
            // Deny writes, rename and deletion by other opens while owned.
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        let mut owned = Self(Some(file));
        let file = owned.0.as_mut().ok_or(io::ErrorKind::Other)?;
        file.write_all(generation)?;
        file.sync_all()?;
        Ok(owned)
    }
    /// Delete the exact still-owned file. No path reopen, identity comparison
    /// followed by unlink, stale-path adoption or automatic Drop deletion.
    /// On native failure ownership and the persistent file are retained.
    pub fn release_after_proof(&mut self) -> io::Result<()> {
        let file = self.0.as_ref().ok_or(io::ErrorKind::InvalidInput)?;
        let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
        // SAFETY: owned live DELETE-capable file; generated initialized ABI;
        // synchronous metadata call borrows the input only for this call.
        unsafe {
            check(SetFileInformationByHandle(
                file.as_raw_handle(),
                FileDispositionInfo,
                (&disposition as *const FILE_DISPOSITION_INFO).cast(),
                size_of::<FILE_DISPOSITION_INFO>() as u32,
            ))?;
        }
        self.0.take();
        Ok(())
    }
}
pub struct Parent {
    pub bootstrap: Writer,
    pub challenge: Reader,
    pub response: Writer,
    pub stop: Writer,
    pub receipt: Reader,
    pub process: Collector,
}
pub struct Child {
    pub bootstrap: Reader,
    pub challenge: Writer,
    pub response: Reader,
    pub stop: Reader,
    pub receipt: Writer,
}
struct Attributes {
    storage: Vec<usize>,
}
impl Attributes {
    fn ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY A: constructed only after successful initialization; storage outlives delete.
        unsafe {
            DeleteProcThreadAttributeList(self.ptr());
        }
    }
}
pub fn launch(exe: &std::path::Path, mode: &str) -> io::Result<Parent> {
    // This standalone collector only accepts its finite fake-only self. It does
    // not promise a bounded collection time for arbitrary executable programs.
    if exe != std::env::current_exe()? {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    launch_inner(exe, mode, false)
}
/// Controlled packaged sibling only. Explicit application name and the same
/// five-endpoint inheritance allowlist as qualification; never invokes a shell.
pub fn launch_helper(exe: &std::path::Path, fake: bool) -> io::Result<Parent> {
    let expected = std::env::current_exe()?.with_file_name("continuous-helper.exe");
    if !exe.is_absolute() || exe != expected {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    launch_inner(
        exe,
        if fake { "session-fake" } else { "session-usb" },
        false,
    )
}
fn launch_inner(exe: &std::path::Path, mode: &str, fail_after_pipes: bool) -> io::Result<Parent> {
    if mode.is_empty()
        || mode.len() > 64
        || !mode.bytes().all(|x| x.is_ascii_alphanumeric() || x == b'-')
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = exe.as_os_str().encode_wide().collect();
    if path.is_empty()
        || path.len() > 16000
        || path.contains(&0)
        || path.contains(&(b'"' as u16))
        || path.last() == Some(&(b'\\' as u16))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let (br, bw) = pipe(true)?;
    let (cr, cw) = pipe(false)?;
    let (rr, rw) = pipe(true)?;
    let (sr, sw) = pipe(true)?;
    let (tr, tw) = pipe(false)?;
    if fail_after_pipes {
        return Err(io::Error::other(
            "injected pipe setup failure after allocation",
        ));
    }
    let mut handles = vec![
        raw(br.0.as_ref().unwrap()),
        raw(cw.0.as_ref().unwrap()),
        raw(rr.0.as_ref().unwrap()),
        raw(sr.0.as_ref().unwrap()),
        raw(tw.0.as_ref().unwrap()),
    ];
    let mut adversarial = None;
    if mode == "reject-type" {
        let h = unsafe { CreateEventW(null(), 1, 0, null()) };
        if h.is_null() {
            return Err(io::Error::last_os_error());
        }
        adversarial = Some(unsafe { own(h) });
        handles[0] = raw(adversarial.as_ref().unwrap());
    } else if mode == "reject-file" {
        use std::os::windows::ffi::OsStrExt;
        let path: Vec<u16> = exe
            .with_file_name("overlapped-input.tmp")
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let h = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_ALWAYS,
                FILE_FLAG_OVERLAPPED,
                null_mut(),
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        adversarial = Some(unsafe { own(h) });
        handles[0] = raw(adversarial.as_ref().unwrap());
    } else if mode == "reject-overlapped" {
        use windows_sys::Win32::Security::*;
        let name: String = random()?.iter().map(|x| format!("{x:02x}")).collect();
        let name: Vec<u16> = format!(r"\\.\pipe\ipc-v2-rejection-{name}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut acl: ACL = unsafe { zeroed() };
        let mut sd: SECURITY_DESCRIPTOR = unsafe { zeroed() };
        unsafe {
            check(InitializeAcl(
                &mut acl,
                size_of::<ACL>() as u32,
                ACL_REVISION,
            ))?;
            check(InitializeSecurityDescriptor(
                (&mut sd as *mut SECURITY_DESCRIPTOR).cast(),
                1,
            ))?;
            check(SetSecurityDescriptorDacl(
                (&mut sd as *mut SECURITY_DESCRIPTOR).cast(),
                1,
                &acl,
                0,
            ))?;
        }
        let sa = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: (&mut sd as *mut SECURITY_DESCRIPTOR).cast(),
            bInheritHandle: 1,
        };
        let h = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                4096,
                4096,
                0,
                &sa,
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        adversarial = Some(unsafe { own(h) });
        handles[0] = raw(adversarial.as_ref().unwrap());
    } else if mode == "reject-extra-alias" {
        adversarial = Some(rr.0.as_ref().unwrap().try_clone()?);
    } else if mode == "reject-direction" {
        handles.swap(0, 1);
    } else if mode == "reject-alias" {
        let h = rr.0.as_ref().unwrap().try_clone()?;
        adversarial = Some(h);
        handles[3] = raw(adversarial.as_ref().unwrap());
    }
    if let Some(h) = &adversarial {
        unsafe {
            check(SetHandleInformation(
                raw(h),
                HANDLE_FLAG_INHERIT,
                HANDLE_FLAG_INHERIT,
            ))?;
        }
    }
    if mode == "reject-extra-alias" {
        handles.push(raw(adversarial.as_ref().unwrap()));
    }
    // Protocol handles are never installed as stdio aliases.
    let mut args = format!(
        " {} {} {} {} {} {} {}",
        if mode.starts_with("session-") {
            "--capability"
        } else {
            "--fake"
        },
        mode,
        handles[0] as usize,
        handles[1] as usize,
        handles[2] as usize,
        handles[3] as usize,
        handles[4] as usize
    );
    if mode == "probe" {
        for h in [
            raw(bw.0.as_ref().unwrap()),
            raw(cr.0.as_ref().unwrap()),
            raw(rw.0.as_ref().unwrap()),
            raw(sw.0.as_ref().unwrap()),
            raw(tr.0.as_ref().unwrap()),
        ] {
            args.push_str(&format!(" {}", h as usize));
        }
    }

    let mut command = vec![b'"' as u16];
    command.extend(&path);
    command.push(b'"' as u16);
    command.extend(args.encode_utf16());
    command.push(0);
    if command.len() > 32767 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut application = path;
    application.push(0);
    let mut bytes = 0;
    // SAFETY A: first call sizes the aligned initialized backing allocation; second initializes it.
    unsafe {
        InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut bytes);
    }
    if bytes == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
    unsafe {
        check(InitializeProcThreadAttributeList(
            storage.as_mut_ptr().cast(),
            1,
            0,
            &mut bytes,
        ))?;
    }
    let mut attr = Attributes { storage };
    // SAFETY L: handle array and attribute storage live through CreateProcessW. List contains only child endpoints.
    unsafe {
        check(UpdateProcThreadAttribute(
            attr.ptr(),
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            handles.as_mut_ptr().cast(),
            handles.len() * size_of::<HANDLE>(),
            null_mut(),
            null_mut(),
        ))?;
        let mut si: STARTUPINFOEXW = zeroed();
        si.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        si.StartupInfo.hStdInput = null_mut();
        si.StartupInfo.hStdOutput = null_mut();
        si.StartupInfo.hStdError = null_mut();
        si.lpAttributeList = attr.ptr();
        let mut pi: PROCESS_INFORMATION = zeroed();
        check(CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            1,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
            null(),
            null(),
            &si.StartupInfo,
            &mut pi,
        ))?;
        let process = Collector(Some(own(pi.hProcess)));
        LAUNCHED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let thread = own(pi.hThread);
        drop(thread);
        // Close child-only copies immediately. All pre-launch errors unwind RAII locals.
        drop((br, cw, rr, sr, tw));
        Ok(Parent {
            bootstrap: bw,
            challenge: cr,
            response: rw,
            stop: sw,
            receipt: tr,
            process,
        })
    }
}
// Collection owner is installed immediately at successful CreateProcessW.
// On error, panic, deadline or ordinary drop it waits for self-termination; never kills.
// Public launch accepts only the finite fake-only self. Scheduling and OS wait
// failures can delay collection; ownership remains here until a numeric exit.
pub struct Collector(Option<OwnedHandle>);
static LAUNCHED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static COLLECTED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn collection_counts() -> (u64, u64) {
    (
        LAUNCHED.load(std::sync::atomic::Ordering::Acquire),
        COLLECTED.load(std::sync::atomic::Ordering::Acquire),
    )
}
impl Drop for Collector {
    fn drop(&mut self) {
        if let Some(h) = self.0.as_ref() {
            // Retain ownership and retry errors; closing is never used as collection.
            loop {
                match wait_handle(h, Duration::from_secs(1)) {
                    Ok(_) => {
                        COLLECTED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(1)),
                }
            }
        }
    }
}
pub fn wait_status(status: u32, error: i32) -> io::Result<()> {
    match status {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_TIMEOUT => Err(io::ErrorKind::TimedOut.into()),
        WAIT_FAILED => Err(io::Error::from_raw_os_error(error)),
        x => Err(io::Error::other(format!("unexpected wait status {x}"))),
    }
}
fn wait_handle(h: &OwnedHandle, timeout: Duration) -> io::Result<u32> {
    // SAFETY: privately owned process, initialized output, finite timeout.
    unsafe {
        let status =
            WaitForSingleObject(raw(h), timeout.as_millis().min(u32::MAX as u128 - 1) as u32);
        let error = GetLastError() as i32;
        wait_status(status, error)?;
        let mut code = 0;
        check(GetExitCodeProcess(raw(h), &mut code))?;
        Ok(code)
    }
}
impl Parent {
    pub fn wait(&mut self, timeout: Duration) -> io::Result<u32> {
        self.process.wait(timeout)
    }
}
impl Collector {
    /// Same retained collection policy as Drop, made explicit so the background
    /// owner can publish eventual numeric collection. Never a caller/UI operation.
    pub fn wait_retained(&mut self) -> u32 {
        loop {
            match self.wait(Duration::from_secs(1)) {
                Ok(code) => return code,
                Err(_) => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    }
    pub fn wait(&mut self, timeout: Duration) -> io::Result<u32> {
        let code = wait_handle(self.0.as_ref().ok_or(io::ErrorKind::InvalidInput)?, timeout)?;
        self.0.take(); // only after a successful numeric process collection
        COLLECTED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Ok(code)
    }
}

// Metadata queries use the generated WDK ABI, not guessed structures.
// QueryInformationFile is allowed to report pending. In that exceptional case its
// initialized heap output is retained for process lifetime, and adoption rejects.
// This fail-closed fallback never frees a potentially kernel-referenced buffer.
fn info(h: HANDLE, class: i32) -> io::Result<u32> {
    use windows_sys::{
        Wdk::Storage::FileSystem::NtQueryInformationFile, Win32::System::IO::IO_STATUS_BLOCK,
    };
    let mut output = Box::new((unsafe { zeroed::<IO_STATUS_BLOCK>() }, 0u32));
    let status = unsafe {
        NtQueryInformationFile(
            h,
            &mut output.0,
            (&mut output.1 as *mut u32).cast(),
            4,
            class,
        )
    };
    #[cfg(feature = "qualification")]
    let status = if INJECT_PENDING.swap(false, std::sync::atomic::Ordering::AcqRel) {
        0x103
    } else {
        status
    };
    if status == 0x103 {
        Box::leak(output);
        return Err(io::Error::other(
            "pending metadata query: storage retained, adoption rejected",
        ));
    }
    if status != 0 {
        return Err(io::Error::other(format!("metadata NTSTATUS {status:x}")));
    }
    Ok(output.1)
}
#[cfg(feature = "qualification")]
static INJECT_PENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn validate(values: &[HANDLE]) -> io::Result<()> {
    use windows_sys::Wdk::Storage::FileSystem::{
        FileAccessInformation, FileModeInformation, FILE_SYNCHRONOUS_IO_ALERT,
        FILE_SYNCHRONOUS_IO_NONALERT,
    };
    for (i, h) in values.iter().enumerate() {
        if h.is_null() || *h == INVALID_HANDLE_VALUE {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        // Type query accepts arbitrary borrowed handles; no data I/O or ownership.
        if unsafe { GetFileType(*h) } != FILE_TYPE_PIPE {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut flags = 0;
        unsafe {
            check(GetNamedPipeInfo(
                *h,
                &mut flags,
                null_mut(),
                null_mut(),
                null_mut(),
            ))?;
        }
        let access = info(*h, FileAccessInformation)?;
        let mode = info(*h, FileModeInformation)?;
        let read = [true, false, true, true, false][i];
        if (access & (FILE_READ_DATA | FILE_WRITE_DATA))
            != if read {
                FILE_READ_DATA
            } else {
                FILE_WRITE_DATA
            }
            || mode & (FILE_SYNCHRONOUS_IO_ALERT | FILE_SYNCHRONOUS_IO_NONALERT) == 0
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        for previous in &values[..i] {
            if unsafe { CompareObjectHandles(*previous, *h) } != 0 {
                return Err(io::ErrorKind::InvalidInput.into());
            }
        }
    }
    // Enforce alias exclusion outside argv as well. Parent copies can briefly
    // exist between CreateProcessW returning and their immediate RAII drop.
    let end = Instant::now() + Duration::from_millis(200);
    for h in values {
        while object_handle_count(*h)? != 1 {
            if Instant::now() >= end {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    Ok(())
}
fn object_handle_count(h: HANDLE) -> io::Result<u32> {
    use windows_sys::Win32::System::{
        LibraryLoader::*, WindowsProgramming::PUBLIC_OBJECT_BASIC_INFORMATION,
    };
    type Query =
        unsafe extern "system" fn(HANDLE, i32, *mut core::ffi::c_void, u32, *mut u32) -> i32;
    let dll: Vec<u16> = "ntdll.dll".encode_utf16().chain(Some(0)).collect();
    // Borrow the loaded system module. No FreeLibrary for GetModuleHandleW.
    let module = unsafe { GetModuleHandleW(dll.as_ptr()) };
    if module.is_null() {
        return Err(io::Error::last_os_error());
    }
    let proc = unsafe { GetProcAddress(module, c"NtQueryObject".as_ptr().cast()) }
        .ok_or_else(io::Error::last_os_error)?;
    // SAFETY: official NtQueryObject signature and generated public output ABI.
    let query: Query = unsafe { std::mem::transmute(proc) };
    let mut out: PUBLIC_OBJECT_BASIC_INFORMATION = unsafe { zeroed() };
    let mut length = 0;
    let status = unsafe {
        query(
            h,
            0,
            (&mut out as *mut PUBLIC_OBJECT_BASIC_INFORMATION).cast(),
            size_of_val(&out) as u32,
            &mut length,
        )
    };
    if status != 0 {
        return Err(io::Error::other(format!("NtQueryObject status {status:x}")));
    }
    Ok(out.HandleCount)
}
/// # Safety
/// Call only as the first application action in single-threaded helper startup:
/// no Rust owner may own an argv-selected inherited handle and no concurrent
/// handle-table mutation may occur. This is an ownership requirement, NOT a
/// promise that argv came from our launcher. All object/mode/direction/alias
/// checks below are mandatory even for a helper launched directly by strangers.
/// On ANY error the caller MUST immediately exit the process, without retry,
/// application callbacks, runtime initialization, or handle cleanup. A pending
/// metadata query retains both outputs until that process exit. This API is
/// exclusively for a separately audited executable entry shim.
/// On success inherited originals are consumed, owned, then made noninheritable
/// once; no duplicate/std-IO aliases are manufactured. No safe raw API exists.
pub unsafe fn child_at_process_entry() -> io::Result<(String, Child)> {
    static CLAIMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if CLAIMED.swap(true, std::sync::atomic::Ordering::AcqRel) {
        return Err(io::ErrorKind::AlreadyExists.into());
    }
    let a: Vec<_> = std::env::args_os().collect();
    if (a.len() != 8 && !(a.len() == 13 && a[2] == "probe"))
        || (a[1] != "--fake" && a[1] != "--capability")
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mode = a[2].to_str().ok_or(io::ErrorKind::InvalidInput)?.to_owned();
    if mode.starts_with("session-") != (a[1] == "--capability") {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    if mode.is_empty() || !mode.bytes().all(|x| x.is_ascii_alphanumeric() || x == b'-') {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut values = Vec::new();
    for v in &a[3..8] {
        values.push(
            v.to_str()
                .ok_or(io::ErrorKind::InvalidInput)?
                .parse::<usize>()
                .map_err(|_| io::ErrorKind::InvalidInput)? as HANDLE,
        );
    }
    // Qualification changes only borrowed startup handles before any ownership.
    // Pending injection exercises the retained-output rejection branch after a
    // real completed query; it does not claim a naturally pending native query.
    #[cfg(feature = "qualification")]
    if mode == "reject-protected" {
        unsafe {
            check(SetHandleInformation(
                values[0],
                HANDLE_FLAG_PROTECT_FROM_CLOSE,
                HANDLE_FLAG_PROTECT_FROM_CLOSE,
            ))?;
        }
    }
    #[cfg(feature = "qualification")]
    if mode == "reject-pending" {
        INJECT_PENDING.store(true, std::sync::atomic::Ordering::Release);
    }
    validate(&values)?; // absolutely no Reader/Writer/OwnedHandle before this
    if mode == "probe" {
        for v in &a[8..] {
            let h = v
                .to_str()
                .ok_or(io::ErrorKind::InvalidInput)?
                .parse::<usize>()
                .map_err(|_| io::ErrorKind::InvalidInput)? as HANDLE;
            let mut flags = 0;
            // Parent endpoints are pipes. Excluded numeric slots may be reused
            // for unrelated non-pipe runtime handles in the child. Actual
            // inherited endpoints necessarily remain FILE_TYPE_PIPE.
            if unsafe { GetFileType(h) } == FILE_TYPE_PIPE
                && unsafe { GetHandleInformation(h, &mut flags) } != 0
            {
                return Err(io::Error::other("parent-only handle inherited"));
            }
        }
    }
    for h in &values {
        let mut flags = 0;
        unsafe {
            check(GetHandleInformation(*h, &mut flags))?;
        }
        if flags & HANDLE_FLAG_INHERIT == 0 || flags & HANDLE_FLAG_PROTECT_FROM_CLOSE != 0 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
    }
    let mut owned = Vec::new();
    for h in values {
        // SAFETY: process-entry ownership contract plus complete pipe validation.
        owned.push(unsafe { own(h) });
    }
    for h in &owned {
        unsafe {
            check(SetHandleInformation(raw(h), HANDLE_FLAG_INHERIT, 0))?;
        }
    }
    let mut it = owned.into_iter();
    Ok((
        mode,
        Child {
            bootstrap: Reader(it.next(), Cancel::default(), None),
            challenge: Writer(it.next(), Cancel::default(), None),
            response: Reader(it.next(), Cancel::default(), None),
            stop: Reader(it.next(), Cancel::default(), None),
            receipt: Writer(it.next(), Cancel::default(), None),
        },
    ))
}
pub fn handle_count() -> io::Result<u32> {
    let mut n = 0;
    // SAFETY H: pseudo handle is borrowed, valid output pointer.
    unsafe {
        check(GetProcessHandleCount(GetCurrentProcess(), &mut n))?;
    }
    Ok(n)
}

/// Native qualification calls the same pipe constructors, transfer, validators,
/// cancellation latch, launch and collection code used above. No device APIs.
pub fn platform_tests(exe: &std::path::Path) -> io::Result<()> {
    let root = std::env::temp_dir().join(format!("v014-fake-lock-{}", std::process::id()));
    std::fs::create_dir(&root)?; // never adopt a preexisting root
    let path = root.join("fake-device.lock");
    {
        let _first = RetainedInterlock::create_new(&path, b"fake-session-one")?;
        assert!(RetainedInterlock::create_new(&path, b"fake-session-two").is_err());
        assert!(std::fs::rename(&path, root.join("replacement")).is_err());
        assert!(std::fs::write(&path, b"replacement").is_err());
    }
    assert_eq!(std::fs::read(&path)?, b"fake-session-one");
    assert!(RetainedInterlock::create_new(&path, b"stale-adoption").is_err());
    // Retained markers are never cleared, including in qualification. A
    // different new private marker independently tests exact proof deletion.
    let released_path = root.join("released-device.lock");
    let mut next = RetainedInterlock::create_new(&released_path, b"fake-session-two")?;
    next.release_after_proof()?;
    assert!(!released_path.exists());
    assert!(next.release_after_proof().is_err());
    assert_eq!(std::fs::read(&path)?, b"fake-session-one");
    println!("PASS fake-interlock retains-on-drop excludes-replacement exact-handle-delete");
    // Exercise the actual retained workers, not a transport model. Two workers
    // serve 1,024 successive fixed records and are both joined on endpoint drop.
    let started = WORKERS_STARTED.load(std::sync::atomic::Ordering::Acquire);
    let joined = WORKERS_JOINED.load(std::sync::atomic::Ordering::Acquire);
    {
        let (mut r, mut w) = pipe(true)?;
        for sequence in 0u64..1024 {
            let mut record = [0; 128];
            record[..8].copy_from_slice(&sequence.to_le_bytes());
            w.write(
                &record,
                Instant::now() + Duration::from_secs(2),
                &Cancel::default(),
            )?;
            assert_eq!(
                r.record(Instant::now() + Duration::from_secs(2), &Cancel::default())?,
                record
            );
        }
        assert_eq!(
            WORKERS_STARTED.load(std::sync::atomic::Ordering::Acquire) - started,
            2
        );
    }
    assert_eq!(
        WORKERS_JOINED.load(std::sync::atomic::Ordering::Acquire) - joined,
        2
    );
    println!("PASS retained-pipe-workers records=1024 started=2 joined=2");
    let t = Instant::now();
    let c = Cancel::default();
    assert!(timely(&c, t, t).is_err());
    assert!(timely(&c, t - Duration::from_secs(1), t).is_err()); // sticky, never resurrect
    assert!(wait_status(WAIT_OBJECT_0, 0).is_ok());
    assert_eq!(
        wait_status(WAIT_TIMEOUT, 5).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert_eq!(
        wait_status(WAIT_FAILED, 5).unwrap_err().raw_os_error(),
        Some(5)
    );
    assert_eq!(
        wait_status(WAIT_ABANDONED, 5).unwrap_err().kind(),
        io::ErrorKind::Other
    );
    // Native WAIT_FAILED: real owned event lacks SYNCHRONIZE after restricted duplication.
    let event = unsafe { CreateEventW(null(), 1, 0, null()) };
    if event.is_null() {
        return Err(io::Error::last_os_error());
    }
    let event = unsafe { own(event) };
    let mut restricted = null_mut();
    unsafe {
        check(DuplicateHandle(
            GetCurrentProcess(),
            raw(&event),
            GetCurrentProcess(),
            &mut restricted,
            EVENT_MODIFY_STATE,
            0,
            0,
        ))?;
    }
    let restricted = unsafe { own(restricted) };
    let status = unsafe { WaitForSingleObject(raw(&restricted), 0) };
    let error = unsafe { GetLastError() } as i32;
    assert_eq!(status, WAIT_FAILED);
    assert_eq!(
        wait_status(status, error).unwrap_err().raw_os_error(),
        Some(ERROR_ACCESS_DENIED as i32)
    );
    println!("PASS fake-time exact-expiry sticky-cancel; native WAIT_FAILED={error}; decoded unexpected-status");
    for mode in [
        "reject-type",
        "reject-direction",
        "reject-alias",
        "reject-extra-alias",
        "reject-file",
        "reject-overlapped",
        "reject-protected",
        "reject-pending",
    ] {
        let mut p = launch(exe, mode)?;
        assert_eq!(p.wait(Duration::from_secs(3))?, 90, "{mode}");
        println!("PASS argv-{mode} rejected-before-I/O");
    }
    // Explicit alias identity test even with distinct numeric values.
    let (r, w) = pipe(true)?;
    let alias = r.0.as_ref().unwrap().try_clone()?;
    let values = [
        raw(r.0.as_ref().unwrap()),
        raw(w.0.as_ref().unwrap()),
        raw(r.0.as_ref().unwrap()),
        raw(&alias),
        raw(w.0.as_ref().unwrap()),
    ];
    assert!(validate(&values).is_err());
    drop((r, w, alias));
    // A separate inheritable writer must not keep EOF alive in the child.
    let (mut sentinel, writer) = pipe(false)?;
    let mut p = launch(exe, "probe")?;
    let ready = match p
        .receipt
        .record(Instant::now() + Duration::from_secs(1), &Cancel::default())
    {
        Ok(ready) => ready,
        Err(error) => {
            let exit = p.wait(Duration::from_secs(3));
            return Err(io::Error::other(format!(
                "probe ready failed: {error}; numeric child={exit:?}"
            )));
        }
    };
    assert_eq!(ready, [0x52; 128], "entry adoption ready barrier");
    drop(writer);
    sentinel.expect_eof(Instant::now() + Duration::from_secs(1), &Cancel::default())?;
    assert_eq!(
        p.wait(Duration::ZERO).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    p.bootstrap.close();
    assert_eq!(p.wait(Duration::from_secs(3))?, 21);
    println!(
        "PASS unrelated-and-all-parent-only-handles-excluded; native WAIT_TIMEOUT then collected"
    );
    // Original read/write endpoints close, with no stdio or duplicate aliases.
    let (mut r, w) = pipe(true)?;
    drop(w);
    r.expect_eof(Instant::now() + Duration::from_secs(1), &Cancel::default())?;
    let (mut r, w) = pipe(true)?;
    drop(w);
    assert_eq!(
        r.expect_eof(
            Instant::now() - Duration::from_millis(1),
            &Cancel::default()
        )
        .unwrap_err()
        .kind(),
        io::ErrorKind::TimedOut
    );
    for eof in [false, true] {
        let (mut r, mut w) = pipe(true)?;
        if eof {
            w.close();
        } else {
            w.write(
                &[0; 128],
                Instant::now() + Duration::from_secs(1),
                &Cancel::default(),
            )?;
        }
        QUALIFICATION_POST_READ_PAUSE_MS.store(70, std::sync::atomic::Ordering::Release);
        let end = Instant::now() + Duration::from_millis(25);
        let c = Cancel::default();
        let e = if eof {
            r.expect_eof(end, &c).unwrap_err()
        } else {
            r.record(end, &c).unwrap_err()
        };
        assert_eq!(e.kind(), io::ErrorKind::TimedOut);
        assert!(c.cancelled());
        println!(
            "PASS native-complete-{} then-injected-observation-delay rejects-expiry",
            if eof { "EOF" } else { "record" }
        );
    }
    // Actual blocked reader and writer, both timeout and externally latched STOP.
    for write in [false, true] {
        for stop in [false, true] {
            let (mut r, mut w) = pipe(true)?;
            let latch = Cancel::default();
            let other = latch.clone();
            let interrupter = if stop {
                Some(std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(25));
                    other.cancel();
                }))
            } else {
                None
            };
            let end = Instant::now() + Duration::from_millis(80);
            let e = if write {
                w.write(&vec![0; 1_048_576], end, &latch).unwrap_err()
            } else {
                r.record(end, &latch).unwrap_err()
            };
            assert_eq!(
                e.kind(),
                if stop {
                    io::ErrorKind::Interrupted
                } else {
                    io::ErrorKind::TimedOut
                }
            );
            assert!(latch.cancelled());
            assert!(w
                .write(&[1], Instant::now() + Duration::from_secs(1), &latch)
                .is_err());
            if write {
                assert!(w
                    .write(
                        &[1],
                        Instant::now() + Duration::from_secs(1),
                        &Cancel::default()
                    )
                    .is_err());
            } else {
                assert!(r
                    .record(Instant::now() + Duration::from_secs(1), &Cancel::default())
                    .is_err());
            }
            if let Some(j) = interrupter {
                j.join().unwrap();
            }
            println!(
                "PASS blocked-{} cancel-{} completion-joined",
                if write { "writer" } else { "reader" },
                if stop { "STOP" } else { "deadline" }
            );
        }
    }
    for _ in 0..8 {
        assert!(launch_inner(&exe.with_file_name("absent-v2.exe"), "normal", false).is_err());
        assert!(launch_inner(exe, "normal", true).is_err());
    }
    println!("PASS launch-and-setup-errors RAII");
    Ok(())
}
