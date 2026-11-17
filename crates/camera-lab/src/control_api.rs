//! Opt-in, loopback-only GUI control transport for Camera.
//! The server only queues requests; the eframe App owns every Worker operation.
use crate::{
    controls::Settings,
    session::{Snapshot, State},
};
use kohdalab_control_protocol::{descriptor, paths::AppPaths, response as common_response};
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
    pub fn start_with_wake(
        directory: Option<PathBuf>,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let dir = directory
            .or_else(|| {
                AppPaths::for_app("camera")
                    .ok()
                    .map(|paths| paths.run_dir())
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
/// The only response writer: every reply, including early refusals, is one
/// complete JSON Lines record. Serialization happens before any byte is sent,
/// so a peer never observes a partial or unterminated record.
fn write_line(stream: &mut impl Write, value: &Value) -> Result<(), String> {
    let mut line = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    line.push(b'\n');
    stream.write_all(&line).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())
}
fn serve(
    mut stream: TcpStream,
    token: &str,
    sender: &mpsc::SyncSender<Request>,
    wake: &(dyn Fn() + Send + Sync),
) -> Result<(), String> {
    // On macOS an accepted stream can inherit the listener's nonblocking mode.
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + TIMEOUT;
    let value = answer(&mut stream, token, sender, wake, deadline);
    write_line(&mut stream, &value)
}
/// Envelope policy is unchanged: unparsable or non-object requests keep the
/// legacy error shape; everything else uses `common_response`, which leaves
/// native (non-`control.*`) envelopes untouched.
fn answer(
    stream: &mut TcpStream,
    token: &str,
    sender: &mpsc::SyncSender<Request>,
    wake: &(dyn Fn() + Send + Sync),
    deadline: Instant,
) -> Value {
    let body = match read(stream, deadline).and_then(|b| {
        serde_json::from_slice::<Value>(&b).map_err(|_| "Request must be JSON".into())
    }) {
        Ok(v) => v,
        Err(e) => return error("invalid_request", &e),
    };
    if !body.is_object() {
        return error("invalid_request", "Request must be a JSON object");
    }
    let transport_error = |code, message| common_response(&body, error(code, message));
    // Authentication precedes queueing: an unauthenticated body never reaches the GUI.
    if body.get("token").and_then(Value::as_str) != Some(token) {
        return transport_error("authentication", "GUI API authentication failed");
    }
    let (reply, rx) = mpsc::sync_channel(1);
    match sender.try_send(Request {
        body: body.clone(),
        deadline,
        reply,
    }) {
        Ok(()) => {
            wake();
            rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|_| transport_error("timeout", "GUI request timed out"))
        }
        Err(mpsc::TrySendError::Full(_)) => transport_error("busy", "GUI request queue is full"),
        Err(mpsc::TrySendError::Disconnected(_)) => {
            transport_error("unavailable", "GUI API is stopping")
        }
    }
}

fn settings_json(settings: Settings) -> Value {
    json!({
        "mode": settings.mode.profile().id,
        "exposure_rows": settings.exposure_rows,
        "gain_eighths": settings.gain_eighths,
    })
}
/// `control.config` result built only from the GUI draft and the Worker
/// snapshot the GUI already holds for this tick.
///
/// Top-level `mode/exposure_rows/gain_eighths` remain the GUI draft for
/// compatibility. `applied` is non-null only while the snapshot proves that the
/// currently requested generation is streaming with checked settings; stale
/// applied values retained across a pending request, stop, disconnect, error or
/// disabled restart are never reported as current.
pub fn config(draft: Settings, snapshot: &Snapshot) -> Value {
    let failed = snapshot.restart_disabled || snapshot.state == State::Error;
    let confirmed = !failed
        && snapshot.state == State::Streaming
        && snapshot.applied_revision.is_some()
        && snapshot.applied_revision == snapshot.requested_revision;
    let applied = snapshot
        .applied
        .zip(snapshot.applied_revision)
        .filter(|_| confirmed);
    let requested = snapshot.requested.zip(snapshot.requested_revision);
    let apply_state = if failed {
        "failed"
    } else if applied.is_some() {
        "applied"
    } else {
        match snapshot.state {
            State::Connecting | State::SettingsPending => "pending",
            State::Streaming if requested.is_some() => "unconfirmed",
            State::Stopping => "stopping",
            _ => "none",
        }
    };
    let with_generation = |(settings, generation): (Settings, u64)| {
        let mut value = settings_json(settings);
        value["generation"] = json!(generation);
        value
    };
    let mut result = settings_json(draft);
    result["draft"] = settings_json(draft);
    result["requested"] = requested.map_or(Value::Null, with_generation);
    result["applied"] = applied.map_or(Value::Null, with_generation);
    result["applied_generation"] = json!(applied.map(|(_, generation)| generation));
    result["apply_state"] = json!(apply_state);
    result
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
        })
        .unwrap();
        let descriptor: Value =
            serde_json::from_slice(&std::fs::read(&server.descriptor).unwrap()).unwrap();
        let peer = thread::spawn(move || {
            let mut stream = TcpStream::connect((
                Ipv4Addr::LOCALHOST,
                descriptor["port"].as_u64().unwrap() as u16,
            ))
            .unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            writeln!(stream, "{}", json!({"control_protocol":1,"method":"control.describe","token":descriptor["token"]})).unwrap();
            let mut reply = String::new();
            BufReader::new(stream).read_line(&mut reply).unwrap();
            assert!(!reply.is_empty());
        });
        woken.recv_timeout(Duration::from_secs(2)).unwrap();
        let request = server
            .requests
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        request.reply.send(json!({"ok":true,"result":{}})).unwrap();
        peer.join().unwrap();
    }
    #[test]
    fn authenticated_request_can_arrive_in_parts() {
        let dir = tempfile::tempdir().unwrap();
        let server = Server::start(Some(dir.path().into())).unwrap();
        let descriptor: Value =
            serde_json::from_slice(&std::fs::read(&server.descriptor).unwrap()).unwrap();
        let port = descriptor["port"].as_u64().unwrap() as u16;
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        let body = json!({
            "control_protocol": 1,
            "method": "control.describe",
            "token": descriptor["token"],
        });
        serde_json::to_writer(&mut stream, &body).unwrap();

        // The server must wait for the newline even after it has read the JSON.
        stream
            .set_read_timeout(Some(Duration::from_millis(250)))
            .unwrap();
        let early_reply = stream.read(&mut [0]);
        assert!(
            matches!(&early_reply, Err(e) if matches!(e.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock)),
            "server replied or closed before the newline: {early_reply:?}"
        );
        stream.write_all(b"\n").unwrap();

        let request = server.requests.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(request.body, body);
        let expected = json!({"ok":true,"result":{}});
        request.reply.send(expected.clone()).unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap(), expected);
    }
    #[test]
    fn unprocessed_status_request_returns_canonical_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let server = Server::start(Some(dir.path().into())).unwrap();
        let descriptor: Value =
            serde_json::from_slice(&std::fs::read(&server.descriptor).unwrap()).unwrap();
        let port = descriptor["port"].as_u64().unwrap() as u16;
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        writeln!(
            stream,
            "{}",
            json!({
                "control_protocol": 1,
                "method": "control.status",
                "token": descriptor["token"],
                "request_id": "unprocessed-status",
            })
        )
        .unwrap();

        // Leave the request queued to model a GUI blocked in a modal dialog.
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["ok"], false);
        assert_eq!(reply["error"]["application_code"], "timeout");
        assert_eq!(reply["control_protocol"], 1);
        assert_eq!(reply["request_id"], "unprocessed-status");
        assert_eq!(
            server.requests.try_recv().unwrap().body["request_id"],
            "unprocessed-status"
        );
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

    /// Sends raw bytes and returns everything the server writes before closing.
    fn exchange_raw(port: u16, request: &[u8], half_close: bool) -> Vec<u8> {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(request).unwrap();
        if half_close {
            stream.shutdown(std::net::Shutdown::Write).unwrap();
        }
        let mut out = Vec::new();
        stream.read_to_end(&mut out).unwrap();
        out
    }
    #[test]
    fn every_early_refusal_is_one_newline_terminated_record_in_its_existing_shape() {
        let dir = tempfile::tempdir().unwrap();
        let server = Server::start(Some(dir.path().into())).unwrap();
        let descriptor: Value =
            serde_json::from_slice(&std::fs::read(&server.descriptor).unwrap()).unwrap();
        let port = descriptor["port"].as_u64().unwrap() as u16;
        let common_bad_token = json!({
            "control_protocol": 1,
            "method": "control.status",
            "token": "wrong",
            "request_id": "bad-token",
        });
        let native_bad_token = json!({"method": "status", "token": "wrong"});
        let cases: [(Vec<u8>, bool, &str); 5] = [
            (b"not json\n".to_vec(), false, "legacy"),
            (b"[1,2]\n".to_vec(), false, "legacy"),
            (b"{\"method\":\"control.status\"}".to_vec(), true, "legacy"),
            (
                format!("{common_bad_token}\n").into_bytes(),
                false,
                "common",
            ),
            (
                format!("{native_bad_token}\n").into_bytes(),
                false,
                "native",
            ),
        ];
        for (request, half_close, shape) in cases {
            let reply = exchange_raw(port, &request, half_close);
            assert_eq!(reply.last(), Some(&b'\n'), "unterminated reply for {shape}");
            assert_eq!(reply.iter().filter(|b| **b == b'\n').count(), 1);
            let value: Value = serde_json::from_slice(&reply[..reply.len() - 1]).unwrap();
            assert_eq!(value["ok"], false);
            match shape {
                "legacy" => {
                    assert_eq!(value["error_code"], "invalid_request");
                    assert!(value.get("control_protocol").is_none());
                }
                "common" => {
                    assert_eq!(value["control_protocol"], 1);
                    assert_eq!(value["request_id"], "bad-token");
                    assert_eq!(value["error"]["code"], "authentication");
                    assert_eq!(value["error"]["application_code"], "authentication");
                }
                _ => {
                    assert_eq!(value["error_code"], "authentication");
                    assert!(value.get("control_protocol").is_none());
                }
            }
        }
        assert!(server.requests.try_recv().is_err());
    }

    fn settings(mode: crate::controls::Mode, exposure_rows: u16, gain_eighths: u16) -> Settings {
        Settings {
            mode,
            exposure_rows,
            gain_eighths,
        }
    }
    fn assert_draft(value: &Value, draft: Settings) {
        let expected = settings_json(draft);
        assert_eq!(value["draft"], expected);
        for key in ["mode", "exposure_rows", "gain_eighths"] {
            assert_eq!(value[key], expected[key], "legacy top-level {key}");
        }
    }
    #[test]
    fn config_distinguishes_draft_pending_and_applied_generations() {
        use crate::controls::Mode;
        let live = settings(Mode::FullFieldSkip, 120, 8);
        let edited = settings(Mode::FullFieldSkip, 200, 16);

        // Before Apply: only the GUI draft exists.
        let idle = Snapshot::default();
        let value = config(edited, &idle);
        assert_draft(&value, edited);
        assert_eq!(value["applied"], Value::Null);
        assert_eq!(value["applied_generation"], Value::Null);
        assert_eq!(value["requested"], Value::Null);
        assert_eq!(value["apply_state"], "none");

        // Applied generation 1 streams; an unapplied draft edit stays a draft.
        let mut snapshot = Snapshot {
            state: State::Streaming,
            active: true,
            requested: Some(live),
            requested_revision: Some(1),
            applied: Some(live),
            applied_revision: Some(1),
            ..Snapshot::default()
        };
        let value = config(edited, &snapshot);
        assert_draft(&value, edited);
        assert_eq!(value["applied"]["exposure_rows"], 120);
        assert_eq!(value["applied"]["generation"], 1);
        assert_eq!(value["applied_generation"], 1);
        assert_eq!(value["apply_state"], "applied");

        // Apply pressed: generation 2 pending; old generation 1 is not current.
        snapshot.state = State::SettingsPending;
        snapshot.requested = Some(edited);
        snapshot.requested_revision = Some(2);
        let value = config(edited, &snapshot);
        assert_eq!(value["applied"], Value::Null);
        assert_eq!(value["applied_generation"], Value::Null);
        assert_eq!(value["requested"]["exposure_rows"], 200);
        assert_eq!(value["requested"]["generation"], 2);
        assert_eq!(value["apply_state"], "pending");

        // Streaming state alone does not prove the new generation was applied.
        snapshot.state = State::Streaming;
        let value = config(edited, &snapshot);
        assert_eq!(value["applied"], Value::Null);
        assert_eq!(value["apply_state"], "unconfirmed");

        // Generation 2 checked and applied.
        snapshot.applied = Some(edited);
        snapshot.applied_revision = Some(2);
        let value = config(edited, &snapshot);
        assert_eq!(value["applied"], {
            let mut v = settings_json(edited);
            v["generation"] = json!(2);
            v
        });
        assert_eq!(value["applied_generation"], 2);
    }
    #[test]
    fn config_reports_full_capture_as_applied_without_rewriting_the_draft() {
        use crate::controls::Mode;
        let live = settings(Mode::FullFieldSkip, 120, 8);
        let full = Settings {
            mode: Mode::Full,
            ..live
        };
        let snapshot = Snapshot {
            state: State::Streaming,
            active: true,
            requested: Some(full),
            requested_revision: Some(3),
            applied: Some(full),
            applied_revision: Some(3),
            ..Snapshot::default()
        };
        let value = config(live, &snapshot);
        assert_draft(&value, live);
        assert_eq!(value["mode"], Mode::FullFieldSkip.profile().id);
        assert_eq!(value["applied"]["mode"], Mode::Full.profile().id);
        assert_eq!(value["applied_generation"], 3);
        assert_eq!(value["apply_state"], "applied");
    }
    #[test]
    fn config_never_reports_stale_applied_settings_after_stop_disconnect_or_failure() {
        use crate::controls::Mode;
        let live = settings(Mode::FullFieldSkip, 120, 8);
        let stale = Snapshot {
            active: true,
            requested: Some(live),
            requested_revision: Some(4),
            applied: Some(live),
            applied_revision: Some(4),
            ..Snapshot::default()
        };
        let cases = [
            (
                Snapshot {
                    state: State::Stopping,
                    requested: None,
                    requested_revision: None,
                    ..stale.clone()
                },
                "stopping",
            ),
            (
                Snapshot {
                    state: State::Disconnected,
                    active: false,
                    ..stale.clone()
                },
                "none",
            ),
            (
                Snapshot {
                    state: State::Error,
                    ..stale.clone()
                },
                "failed",
            ),
            (
                Snapshot {
                    state: State::Streaming,
                    restart_disabled: true,
                    ..stale.clone()
                },
                "failed",
            ),
        ];
        for (snapshot, state) in cases {
            let value = config(live, &snapshot);
            assert_draft(&value, live);
            assert_eq!(value["applied"], Value::Null, "{state}");
            assert_eq!(value["applied_generation"], Value::Null, "{state}");
            assert_eq!(value["apply_state"], state);
        }
        // Explicit simulation streams without hardware-checked applied settings.
        let simulation = Snapshot {
            state: State::Streaming,
            applied: None,
            applied_revision: None,
            ..stale
        };
        let value = config(live, &simulation);
        assert_eq!(value["applied"], Value::Null);
        assert_eq!(value["apply_state"], "unconfirmed");
    }
}
