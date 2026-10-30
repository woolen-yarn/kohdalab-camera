//! Opt-in, loopback-only GUI control transport for Camera.
//! The server only queues requests; the eframe App owns every Worker operation.
use kohdalab_control_protocol::{descriptor, is_common, paths::AppPaths, response as common_response};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const LIMIT: usize = 64 * 1024;
const PEERS: usize = 4;
const QUEUE: usize = 16;
const TIMEOUT: Duration = Duration::from_secs(2);
pub struct Request {
    pub body: Value,
    pub deadline: Instant,
    pub reply: mpsc::SyncSender<Value>,
}
pub struct Server {
    pub requests: mpsc::Receiver<Request>,
    pub descriptor: PathBuf,
    stopped: Arc<AtomicBool>,
}
impl Server {
    pub fn start(directory: Option<PathBuf>) -> Result<Self, String> {
        Self::start_with_wake(directory, || {})
    }
    pub fn start_with_wake(directory: Option<PathBuf>, wake: impl Fn() + Send + Sync + 'static) -> Result<Self, String> {
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let dir = directory
            .or_else(|| {
                AppPaths::for_app("camera").ok().map(|paths| paths.run_dir())
            })
            .ok_or("Cannot locate user profile")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let token = uuid::Uuid::new_v4().to_string();
        let instance = uuid::Uuid::new_v4().to_string();
        let path = dir.join(format!("camera-{}-{instance}.json", std::process::id()));
        let mut tmp = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
        let d = descriptor(
            json!({"protocol":1,"app":"kohdalab-camera","address":format!("127.0.0.1:{port}"),"token":token,"instance_id":instance,"pid":std::process::id()}),
            "kohdalab-camera",
            1,
        );
        serde_json::to_writer(&mut tmp, &d).map_err(|e| e.to_string())?;
        tmp.flush().map_err(|e| e.to_string())?;
        tmp.persist_noclobber(&path).map_err(|e| e.to_string())?;
        let (s, r) = mpsc::sync_channel(QUEUE);
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let peers = Arc::new(AtomicUsize::new(0));
        thread::Builder::new()
            .name("camera-control-listener".into())
            .spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, peer)) if peer.ip().is_loopback() => {
                            let mut n = peers.load(Ordering::Acquire);
                            while n < PEERS {
                                match peers.compare_exchange(
                                    n,
                                    n + 1,
                                    Ordering::AcqRel,
                                    Ordering::Acquire,
                                ) {
                                    Ok(_) => {
                                        let s = s.clone();
                                        let t = token.clone();
                                        let p = peers.clone();
                                        let wake = wake.clone();
                                        thread::spawn(move || {
                                            let _guard = Permit(p);
                                            let _ = serve(stream, &t, &s, wake.as_ref());
                                        });
                                        break;
                                    }
                                    Err(v) => n = v,
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10))
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            requests: r,
            descriptor: path,
            stopped,
        })
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = std::fs::remove_file(&self.descriptor);
    }
}
struct Permit(Arc<AtomicUsize>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
fn error(code: &str, msg: &str) -> Value {
    json!({"ok":false,"error_code":code,"error":msg})
}
fn serve(
    mut stream: TcpStream,
    token: &str,
    sender: &mpsc::SyncSender<Request>,
    wake: &(dyn Fn() + Send + Sync),
) -> Result<(), String> {
    let deadline = Instant::now() + TIMEOUT;
    let body = match read(&mut stream, deadline).and_then(|b| {
        serde_json::from_slice::<Value>(&b).map_err(|_| "Request must be JSON".into())
    }) {
        Ok(v) => v,
        Err(e) => {
            serde_json::to_writer(&mut stream, &error("invalid_request", &e))
                .map_err(|x| x.to_string())?;
            return Ok(());
        }
    };
    if !body.is_object() {
        serde_json::to_writer(
            &mut stream,
            &error("invalid_request", "Request must be a JSON object"),
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }
    if body.get("token").and_then(Value::as_str) != Some(token) {
        let native = error("authentication", "GUI API authentication failed");
        let value = if is_common(&body) {
            common_response(&body, native)
        } else {
            native
        };
        serde_json::to_writer(&mut stream, &value).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let (reply, rx) = mpsc::sync_channel(1);
    let value = match sender.try_send(Request {
        body: body.clone(),
        deadline,
        reply,
    }) {
        Ok(()) => {
            wake();
            rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|_| error("timeout", "GUI request timed out"))
        },
        Err(mpsc::TrySendError::Full(_)) => error("busy", "GUI request queue is full"),
        Err(mpsc::TrySendError::Disconnected(_)) => error("unavailable", "GUI API is stopping"),
    };
    serde_json::to_writer(&mut stream, &value).map_err(|e| e.to_string())?;
    stream.write_all(b"\n").map_err(|e| e.to_string())
}
fn read(stream: &mut TcpStream, deadline: Instant) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        stream
            .set_read_timeout(Some(deadline.saturating_duration_since(Instant::now())))
            .map_err(|e| e.to_string())?;
        let mut b = [0; 4096];
        let n = stream.read(&mut b).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("Incomplete JSON request".into());
        }
        if let Some(i) = b[..n].iter().position(|c| *c == b'\n') {
            out.extend_from_slice(&b[..i]);
            return if out.len() <= LIMIT {
                Ok(out)
            } else {
                Err("Request exceeds 64 KiB".into())
            };
        }
        out.extend_from_slice(&b[..n]);
        if out.len() > LIMIT {
            return Err("Request exceeds 64 KiB".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    #[test]
    fn authenticated_request_wakes_an_idle_consumer() {
        let directory = tempfile::tempdir().unwrap();
        let (wake, woken) = mpsc::sync_channel(1);
        let server = Server::start_with_wake(Some(directory.path().into()), move || {
            let _ = wake.try_send(());
        }).unwrap();
        let descriptor: Value = serde_json::from_slice(&std::fs::read(&server.descriptor).unwrap()).unwrap();
        let peer = thread::spawn(move || {
            let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, descriptor["port"].as_u64().unwrap() as u16)).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            writeln!(stream, "{}", json!({"control_protocol":1,"method":"control.describe","token":descriptor["token"]})).unwrap();
            let mut reply = String::new();
            BufReader::new(stream).read_line(&mut reply).unwrap();
            assert!(!reply.is_empty());
        });
        woken.recv_timeout(Duration::from_secs(2)).unwrap();
        let request = server.requests.recv_timeout(Duration::from_secs(1)).unwrap();
        request.reply.send(json!({"ok":true,"result":{}})).unwrap();
        peer.join().unwrap();
    }
    #[test]
    fn descriptor_is_canonical_and_bad_token_never_reaches_gui() {
        let dir = tempfile::tempdir().unwrap();
        let server = Server::start(Some(dir.path().into())).unwrap();
        let descriptor: Value =
            serde_json::from_slice(&std::fs::read(&server.descriptor).unwrap()).unwrap();
        assert_eq!(descriptor["control_protocol"], 1);
        assert_eq!(descriptor["application"], "kohdalab-camera");
        let port = descriptor["port"].as_u64().unwrap() as u16;
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        writeln!(
            stream,
            "{{\"control_protocol\":1,\"method\":\"control.status\",\"token\":\"wrong\"}}"
        )
        .unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        let result: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(result["error"]["application_code"], "authentication");
        assert!(server.requests.try_recv().is_err());
    }
}
