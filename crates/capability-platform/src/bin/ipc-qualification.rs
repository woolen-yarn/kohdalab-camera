#[cfg(not(windows))]
fn main() {
    eprintln!("Windows fake-only fixture");
}
#[cfg(windows)]
mod fixture {
    use capability_platform::*;
    use std::{
        io,
        time::{Duration, Instant},
    };
    fn deadline() -> Instant {
        Instant::now() + Duration::from_millis(700)
    }
    fn record(k: u8, s: &[u8; 32], seq: u64, c: &[u8; 32]) -> [u8; 128] {
        let mut b = [0; 128];
        b[..8].copy_from_slice(&[b'C', b'P', 2, k, 128, 0, 0, 0]);
        b[8..40].copy_from_slice(s);
        b[40..72].copy_from_slice(&[42; 32]);
        b[72..80].copy_from_slice(&seq.to_le_bytes());
        b[80..112].copy_from_slice(c);
        b
    }
    fn valid(b: &[u8; 128], k: u8, s: &[u8; 32], seq: u64, c: &[u8; 32]) -> bool {
        *b == record(k, s, seq, c)
    }
    struct Producer(Option<std::thread::JoinHandle<()>>);
    impl Drop for Producer {
        fn drop(&mut self) {
            if let Some(p) = self.0.take() {
                p.join().expect("finite producer");
            }
        }
    }
    struct Watcher(
        Option<std::thread::JoinHandle<(Writer, Option<[u8; 32]>)>>,
        Cancel,
    );
    impl Drop for Watcher {
        fn drop(&mut self) {
            self.1.cancel();
            if let Some(h) = self.0.take() {
                let _ = h.join();
            }
        }
    }
    pub fn child(mode: String, mut c: Child) -> io::Result<u32> {
        if mode == "probe" {
            // Adoption is complete. Parent must observe this barrier before
            // closing bootstrap; process creation alone does not prove entry.
            c.receipt
                .write(&[0x52; 128], deadline(), &Cancel::default())?;
            return match c.bootstrap.record(deadline(), &Cancel::default()) {
                Err(e)
                    if e.raw_os_error() == Some(109)
                        || e.kind() == io::ErrorKind::UnexpectedEof =>
                {
                    Ok(21)
                }
                Err(e) => Err(e),
                Ok(_) => Err(io::Error::other("probe requires bootstrap EOF")),
            };
        }
        let producer_gate =
            std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let watcher_gate = producer_gate.clone();
        let latch = Cancel::default();
        let stop_latch = latch.clone();
        let stop_io = Cancel::default();
        let watcher_io = stop_io.clone();
        let mut stop = c.stop;
        let mut receipt = c.receipt;
        let watcher = std::thread::spawn(move || {
            let b = stop.record(Instant::now() + Duration::from_secs(3), &watcher_io);
            stop_latch.cancel();
            let session = b.ok().and_then(|b| {
                let s = b[8..40].try_into().unwrap();
                if valid(&b, 3, &s, 1, &[0; 32]) {
                    Some(s)
                } else {
                    None
                }
            });
            if let Some(s) = session {
                let _ = receipt.write(&record(4, &s, 1, &[0; 32]), deadline(), &Cancel::default());
            }
            {
                let (lock, wake) = &*watcher_gate;
                *lock.lock().unwrap() = true;
                wake.notify_all();
            }
            drop(stop);
            (receipt, session)
        });
        let mut watcher = Watcher(Some(watcher), stop_io.clone());
        let gated = mode == "stop-producer";
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let mut producer = Producer(Some(std::thread::spawn(move || {
            if gated {
                let (lock, wake) = &*producer_gate;
                let held = lock.lock().unwrap();
                ready_tx.send(()).unwrap();
                let _held = wake
                    .wait_timeout_while(held, Duration::from_secs(3), |released| !*released)
                    .unwrap();
            } else {
                std::thread::sleep(Duration::from_millis(180));
            }
        })));
        let result = (|| -> io::Result<u32> {
            let b = c.bootstrap.record(deadline(), &latch)?;
            c.bootstrap.expect_eof(deadline(), &latch)?;
            let s = b[8..40].try_into().unwrap();
            if !valid(&b, 0, &s, 1, &[0; 32]) {
                latch.cancel();
                return Ok(20);
            }
            for seq in 1..=if mode == "replay" { 2 } else { 1 } {
                let challenge = random()?;
                c.challenge
                    .write(&record(1, &s, seq, &challenge), deadline(), &latch)?;
                let r = c.response.record(deadline(), &latch)?;
                if !valid(&r, 2, &s, seq, &challenge) {
                    latch.cancel();
                    return Ok(22);
                }
            }
            if mode == "stop-producer" {
                ready_rx
                    .recv_timeout(Duration::from_secs(1))
                    .map_err(|_| io::Error::other("producer barrier"))?;
                c.challenge
                    .write(&record(6, &s, 2, &[0; 32]), deadline(), &latch)?;
                // A distinct owner branch: collection of the held producer is
                // now the operation waiting for STOP. Do not fall through to
                // the ordinary response-reader branch under a new test name.
                producer
                    .0
                    .take()
                    .ok_or_else(|| io::Error::other("missing producer"))?
                    .join()
                    .map_err(|_| io::Error::other("producer panic"))?;
                return Ok(0);
            }
            if mode == "eof" {
                c.response.expect_eof(deadline(), &latch)?;
                return Ok(23);
            }
            if mode == "block-challenge" {
                c.challenge.write(&vec![7; 1_048_576], deadline(), &latch)?;
                return Ok(99);
            }
            // No outstanding challenge: unsolicited/replayed records cannot renew.
            // Partial extra input reaches the same finite expiry and cancellation.
            c.response.record(deadline(), &latch)?;
            latch.cancel();
            Ok(22)
        })();
        latch.cancel();
        stop_io.cancel();
        drop(producer);
        drop((c.bootstrap, c.challenge, c.response));
        let (mut receipt, session) = watcher
            .0
            .take()
            .unwrap()
            .join()
            .map_err(|_| io::Error::other("STOP watcher panic"))?;
        if let Some(s) = session {
            receipt.write(&record(5, &s, 1, &[0; 32]), deadline(), &Cancel::default())?;
        }
        drop(receipt);
        Ok(match result {
            Ok(code) => code,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => 0,
            Err(e) if e.kind() == io::ErrorKind::TimedOut => 26,
            Err(e) if e.raw_os_error() == Some(109) || e.kind() == io::ErrorKind::UnexpectedEof => {
                21
            }
            Err(_) => 20,
        })
    }
    fn write(w: &mut Writer, b: &[u8]) -> io::Result<()> {
        w.write(b, deadline(), &Cancel::default())
    }
    fn read(r: &mut Reader) -> io::Result<[u8; 128]> {
        r.record(deadline(), &Cancel::default())
    }
    fn bootstrap(p: &mut Parent, s: &[u8; 32]) -> io::Result<()> {
        write(&mut p.bootstrap, &record(0, s, 1, &[0; 32]))?;
        p.bootstrap.close();
        Ok(())
    }
    fn respond(p: &mut Parent, s: &[u8; 32]) -> io::Result<[u8; 128]> {
        let q = read(&mut p.challenge)?;
        let ch = q[80..112].try_into().unwrap();
        assert!(valid(&q, 1, s, 1, &ch));
        let r = record(2, s, 1, &ch);
        write(&mut p.response, &r)?;
        Ok(r)
    }
    fn stopped(p: &mut Parent, s: &[u8; 32]) -> io::Result<()> {
        let start = Instant::now();
        write(&mut p.stop, &record(3, s, 1, &[0; 32]))?;
        assert!(valid(&read(&mut p.receipt)?, 4, s, 1, &[0; 32]));
        println!("StopObserved_us={}", start.elapsed().as_micros());
        assert!(valid(&read(&mut p.receipt)?, 5, s, 1, &[0; 32]));
        p.receipt.expect_eof(deadline(), &Cancel::default())?;
        assert_eq!(p.wait(Duration::from_secs(3))?, 0);
        Ok(())
    }
    pub fn parent() -> io::Result<()> {
        let exe = std::env::current_exe()?;
        println!("IPC-v2 fake-only; completion observed strictly before expiry");
        platform_tests(&exe)?;
        assert!(launch(std::path::Path::new(""), "normal").is_err());
        assert!(launch(&exe, "").is_err());
        let cold = handle_count()?;
        one_round(&exe)?;
        let baseline = handle_count()?;
        println!("HANDLES cold={cold} warm={baseline}");
        for round in 0..8 {
            one_round(&exe)?;
            assert_eq!(
                handle_count()?,
                baseline,
                "handle accumulation round {round}"
            );
            println!("PASS repeat={round} handles={baseline}");
        }
        let (launched, collected) = collection_counts();
        assert_eq!(launched, collected);
        println!("COLLECTION launched={launched} collected={collected}");
        println!("NATIVE_SUITE_PASS");
        Ok(())
    }
    fn one_round(exe: &std::path::Path) -> io::Result<()> {
        for case in [
            "normal",
            "bad-bootstrap",
            "partial",
            "oversize",
            "tail",
            "missing",
            "stale",
            "replay",
            "corrupt-response",
            "partial-response",
            "oversize-response",
            "unsolicited",
            "trailing-partial",
            "eof",
            "noresponse",
            "late-response",
            "late-eof",
            "stop-bootstrap",
            "stop-eof",
            "stop-response",
            "stop-producer",
            "stop-writer",
            "drop",
            "deadline-drop",
            "panic",
            "error",
        ] {
            let mode = match case {
                "replay" => "replay",
                "stop-producer" => "stop-producer",
                "eof" => "eof",
                "stop-writer" => "block-challenge",
                _ => "normal",
            };
            let mut p = launch(exe, mode)?;
            let s = random()?;
            if ["drop", "deadline-drop", "panic", "error"].contains(&case) {
                if case == "panic" {
                    let caught =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                            let _owner = p;
                            panic!("intentional collection unwind");
                        }));
                    assert!(caught.is_err());
                } else if case == "error" {
                    fn error_path(_p: Parent) -> io::Result<()> {
                        Err(io::Error::other("injected post-launch error"))
                    }
                    assert!(error_path(p).is_err());
                } else {
                    if case == "deadline-drop" {
                        assert_eq!(
                            p.wait(Duration::ZERO).unwrap_err().kind(),
                            io::ErrorKind::TimedOut
                        );
                    }
                    drop(p);
                }
                println!("PASS {case} collected-by-guard");
                continue;
            }
            if case == "stop-bootstrap" {
                stopped(&mut p, &s)?;
            } else if case == "stop-eof" {
                write(&mut p.bootstrap, &record(0, &s, 1, &[0; 32]))?;
                stopped(&mut p, &s)?;
            } else if [
                "bad-bootstrap",
                "partial",
                "oversize",
                "tail",
                "missing",
                "late-eof",
            ]
            .contains(&case)
            {
                let mut b = record(0, &s, 1, &[0; 32]);
                match case {
                    "bad-bootstrap" => {
                        b[2] = 9;
                        write(&mut p.bootstrap, &b)?;
                    }
                    "partial" => write(&mut p.bootstrap, &b[..17])?,
                    "oversize" => write(&mut p.bootstrap, &[0; 129])?,
                    "tail" => {
                        write(&mut p.bootstrap, &b)?;
                        std::thread::sleep(Duration::from_millis(20));
                        write(&mut p.bootstrap, &[0])?;
                    }
                    "late-eof" => {
                        write(&mut p.bootstrap, &b)?;
                        std::thread::sleep(Duration::from_millis(850));
                    }
                    _ => (),
                }
                p.bootstrap.close();
                let code = p.wait(Duration::from_secs(3))?;
                assert_eq!(
                    code,
                    match case {
                        "partial" | "missing" => 21,
                        "late-eof" => 26,
                        _ => 20,
                    },
                    "{case}"
                );
            } else {
                bootstrap(&mut p, &s)?;
                if case == "stop-response" {
                    let _ = read(&mut p.challenge)?;
                    stopped(&mut p, &s)?;
                } else if case == "noresponse" || case == "late-response" {
                    let q = read(&mut p.challenge)?;
                    if case == "late-response" {
                        std::thread::sleep(Duration::from_millis(850));
                        let ch = q[80..112].try_into().unwrap();
                        assert!(write(&mut p.response, &record(2, &s, 1, &ch)).is_err());
                    }
                    assert_eq!(p.wait(Duration::from_secs(3))?, 26);
                } else if case == "stale" {
                    let q = read(&mut p.challenge)?;
                    let ch = q[80..112].try_into().unwrap();
                    let mut bad = record(2, &s, 1, &ch);
                    bad[8] ^= 1;
                    write(&mut p.response, &bad)?;
                    assert_eq!(p.wait(Duration::from_secs(3))?, 22);
                } else if ["corrupt-response", "partial-response", "oversize-response"]
                    .contains(&case)
                {
                    let q = read(&mut p.challenge)?;
                    let ch = q[80..112].try_into().unwrap();
                    let mut b = record(2, &s, 1, &ch);
                    if case == "partial-response" {
                        write(&mut p.response, &b[..17])?;
                        p.response.close();
                    } else if case == "corrupt-response" {
                        b[2] = 9;
                        write(&mut p.response, &b)?;
                    } else {
                        b[4] = 129;
                        let mut bytes = b.to_vec();
                        bytes.push(0);
                        write(&mut p.response, &bytes)?;
                    }
                    assert_eq!(
                        p.wait(Duration::from_secs(3))?,
                        if case == "partial-response" { 21 } else { 22 }
                    );
                } else {
                    let r = respond(&mut p, &s)?;
                    if case == "replay" {
                        let _ = read(&mut p.challenge)?;
                        write(&mut p.response, &r)?;
                        assert_eq!(p.wait(Duration::from_secs(3))?, 22);
                    } else if case == "unsolicited" {
                        write(&mut p.response, &r)?;
                        assert_eq!(p.wait(Duration::from_secs(3))?, 22);
                    } else if case == "trailing-partial" {
                        write(&mut p.response, &[0])?;
                        assert_eq!(p.wait(Duration::from_secs(3))?, 26);
                    } else if case == "eof" {
                        p.response.close();
                        assert_eq!(p.wait(Duration::from_secs(3))?, 23);
                    } else {
                        if case == "stop-producer" {
                            assert!(valid(&read(&mut p.challenge)?, 6, &s, 2, &[0; 32]));
                        }
                        if case == "stop-writer" {
                            std::thread::sleep(Duration::from_millis(30));
                        }
                        stopped(&mut p, &s)?;
                    }
                }
            }
            drop(p);
            println!("PASS {case}");
        }
        Ok(())
    }
}
#[cfg(windows)]
fn main() {
    // First application action: no Rust HANDLE owners, other application threads,
    // or concurrent handle-table mutation. Argv is untrusted and fully validated.
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--exit-probe")) {
        std::process::exit(37);
    }
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--fake")) {
        let adopted = unsafe { capability_platform::child_at_process_entry() };
        let (mode, c) = match adopted {
            Ok(v) => v,
            Err(_) => std::process::exit(90),
        };
        let result = fixture::child(mode, c);
        std::process::exit(match result {
            Ok(n) => n as i32,
            Err(_) => 90,
        });
    }
    if let Err(e) = fixture::parent() {
        eprintln!("fixture error: {e}");
        std::process::exit(1);
    }
}
