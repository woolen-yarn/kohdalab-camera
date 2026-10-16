#![forbid(unsafe_code)]
//! Finite no-USB executable exercising the actual parent and packaged entry.
#[cfg(windows)]
fn main() {
    use camera_lab::continuous_session::{ContinuousSession, Fixture, Terminal};
    use std::{
        thread,
        time::{Duration, Instant},
    };
    if std::env::args().nth(1).as_deref() == Some("--exit-probe") {
        std::process::exit(37);
    }
    let root = std::env::temp_dir().join(format!(
        "continuous-orchestration-fixture-{}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    // Windows lazily retains localization resources on the first OS error.
    // Warm that process cache before establishing the HANDLE baseline.
    let _ = std::io::Error::from_raw_os_error(5).to_string();
    let initial = capability_platform::handle_count().unwrap();
    let cases = [
        Fixture::Normal,
        Fixture::FullFrames,
        Fixture::XgaFrames,
        Fixture::ParentThreadFail,
        Fixture::UnlockFail,
        Fixture::WaitTimeout,
        Fixture::SettingsDigest,
        Fixture::BootstrapStop,
        Fixture::BootstrapStall,
        Fixture::ResponseStop,
        Fixture::LeaseStop,
        Fixture::PrepStop,
        Fixture::BadBootstrap,
        Fixture::StaleSettings,
        Fixture::Replay,
        Fixture::LeaseLoss,
        Fixture::WorkerStall,
        Fixture::Crash,
        Fixture::InitStall,
        Fixture::NoTransfer,
        Fixture::NoFrame,
        Fixture::ProducerStall,
        Fixture::DrainFail,
        Fixture::CleanupFail,
        Fixture::MissingReceipt,
        Fixture::BadReceipt,
        Fixture::NonzeroExit,
        Fixture::ReapFail,
        Fixture::ReleaseStall,
        Fixture::Normal,
    ];
    let mut warm_handles = None;
    for (index, case) in cases.into_iter().enumerate() {
        let case_root = root.join(index.to_string());
        std::fs::create_dir(&case_root).unwrap();
        let session = ContinuousSession::start_fixture(&case_root, case).unwrap();
        if matches!(case, Fixture::BootstrapStop | Fixture::BootstrapStall) {
            session.stop();
        }
        let start = Instant::now();
        let mut stopped = false;
        let mut stalled = false;
        let mut saw_unproved_owned = false;
        let mut delivered = 0u64;
        let mut bytes = 0u64;
        let mut last_sequence = 0;
        let mut first_delivery = None;
        let mut held = None;
        loop {
            let snapshot = session.poll();
            if matches!(case, Fixture::FullFrames | Fixture::XgaFrames) {
                if let Some(frame) = &snapshot.frame {
                    let dims = if case == Fixture::FullFrames {
                        (2048, 1536)
                    } else {
                        (1024, 768)
                    };
                    assert_eq!((frame.metadata.width, frame.metadata.height), dims);
                    assert_eq!(frame.raw.len(), (dims.0 * dims.1) as usize);
                    assert!(
                        frame
                            .raw
                            .iter()
                            .enumerate()
                            .all(|(i, b)| *b == (i as u64 + frame.metadata.sequence) as u8)
                    );
                    if frame.metadata.sequence > last_sequence {
                        last_sequence = frame.metadata.sequence;
                        delivered += 1;
                        bytes += frame.raw.len() as u64;
                        first_delivery.get_or_insert(Instant::now());
                        if held.is_none() {
                            held = Some(frame.clone());
                        }
                    }
                }
            }
            saw_unproved_owned |=
                snapshot.terminal == Some(Terminal::Unproved) && !snapshot.collected;
            if case == Fixture::WorkerStall && !stalled && snapshot.heartbeats >= 2 {
                stalled = true;
                thread::sleep(Duration::from_secs(4));
            }
            let early = (case == Fixture::PrepStop && snapshot.heartbeats >= 2)
                || (case == Fixture::LeaseStop && snapshot.heartbeats >= 1)
                || (case == Fixture::ResponseStop && snapshot.responding);
            let requires_fault = matches!(
                case,
                Fixture::NoTransfer
                    | Fixture::NoFrame
                    | Fixture::ProducerStall
                    | Fixture::InitStall
                    | Fixture::LeaseLoss
                    | Fixture::Replay
                    | Fixture::StaleSettings
                    | Fixture::ParentThreadFail
            );
            let enough = if matches!(case, Fixture::FullFrames | Fixture::XgaFrames) {
                delivered >= 30
            } else {
                snapshot.frames >= 5
            };
            let produced = !requires_fault && enough && snapshot.heartbeats >= 3;
            if !stopped && (early || produced) {
                session.stop();
                stopped = true;
            }
            if snapshot.collected {
                let locks = std::fs::read_dir(&case_root)
                    .unwrap()
                    .filter_map(Result::ok)
                    .filter(|e| e.path().extension().is_some_and(|x| x == "lock"))
                    .count();
                if matches!(
                    case,
                    Fixture::Normal
                        | Fixture::FullFrames
                        | Fixture::XgaFrames
                        | Fixture::PrepStop
                        | Fixture::BootstrapStop
                        | Fixture::ResponseStop
                        | Fixture::LeaseStop
                ) {
                    assert_eq!(
                        snapshot.terminal,
                        Some(Terminal::CleanCancellation),
                        "{case:?}: {:?}",
                        snapshot.first_error
                    );
                    assert_eq!(locks, 0);
                    assert_eq!(snapshot.exit, Some(0));
                    if case == Fixture::Normal {
                        assert!(
                            snapshot.frames >= 5
                                && snapshot.heartbeats >= 3
                                && snapshot.stop_observed
                        );
                    }
                } else if case == Fixture::WorkerStall {
                    // Scheduler/lifecycle-owner loss is a separate qualification
                    // from helper acquisition/lease deadline attribution.
                    assert_eq!(
                        snapshot.first_error.as_deref(),
                        Some("Lifecycle owner unresponsive")
                    );
                    assert_ne!(snapshot.terminal, Some(Terminal::CleanCancellation));
                    assert_eq!(locks, 1);
                    assert!(snapshot.helper_phase >= 1);
                } else if matches!(
                    case,
                    Fixture::NoTransfer
                        | Fixture::NoFrame
                        | Fixture::ProducerStall
                        | Fixture::InitStall
                        | Fixture::LeaseLoss
                        | Fixture::Replay
                        | Fixture::StaleSettings
                ) {
                    assert_ne!(snapshot.terminal, Some(Terminal::CleanCancellation));
                    assert_eq!(locks, 1, "failed sessions must retain their lock");
                    use camera_lab::continuous::Fault;
                    let expected = match case {
                        Fixture::NoTransfer | Fixture::ProducerStall => Fault::NoTransfer,
                        Fixture::NoFrame => Fault::NoFrame,
                        Fixture::InitStall => Fault::Initialization,
                        Fixture::Replay | Fixture::StaleSettings => Fault::Protocol,
                        Fixture::LeaseLoss => Fault::Lease,
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        snapshot.helper_fault,
                        Some(expected),
                        "intended fault required for {case:?}; first={:?}",
                        snapshot.first_error
                    );
                    if matches!(
                        case,
                        Fixture::NoTransfer
                            | Fixture::NoFrame
                            | Fixture::ProducerStall
                            | Fixture::LeaseLoss
                    ) {
                        assert!(snapshot.helper_phase >= 3, "must reach Live");
                    } else if case == Fixture::InitStall {
                        assert!(snapshot.helper_phase >= 1);
                    }
                    assert_eq!(
                        snapshot.proof_flags & 7,
                        7,
                        "fake cleanup/drop must be proved"
                    );
                } else {
                    assert_eq!(
                        snapshot.terminal,
                        Some(Terminal::Unproved),
                        "{case:?}: {:?}",
                        snapshot.first_error
                    );
                    assert_eq!(locks, 1);
                }
                if case == Fixture::WaitTimeout {
                    assert!(
                        snapshot
                            .first_error
                            .as_ref()
                            .is_some_and(|e| e.contains("Some(TimedOut)"))
                    );
                }
                if matches!(case, Fixture::BootstrapStall | Fixture::ReleaseStall) {
                    assert!(
                        saw_unproved_owned,
                        "UI must observe Unproved while native collection is still owned"
                    );
                    if case == Fixture::ReleaseStall {
                        assert!(stopped && snapshot.frames >= 5 && snapshot.helper_phase >= 3);
                        assert_eq!(
                            snapshot.helper_phase, 4,
                            "delayed release barrier must be observed"
                        );
                        assert_eq!(
                            snapshot.first_error.as_deref(),
                            Some("Shutdown proof deadline")
                        );
                    }
                }
                if case == Fixture::UnlockFail {
                    assert!(
                        snapshot
                            .first_error
                            .as_ref()
                            .is_some_and(|e| e.contains("os error 5")),
                        "actual native deletion failure required: {:?}",
                        snapshot.first_error
                    );
                }
                if locks > 0 {
                    let marker = std::fs::read_dir(&case_root)
                        .unwrap()
                        .map(Result::unwrap)
                        .find(|e| e.path().extension().is_some_and(|x| x == "lock"))
                        .unwrap()
                        .path();
                    let exact_generation = std::fs::read(&marker).unwrap();
                    let marker_hash = capability_platform::digest(&exact_generation).unwrap();
                    // No replacement even after numeric child collection.
                    let blocked =
                        ContinuousSession::start_fixture(&case_root, Fixture::Normal).unwrap();
                    let at = Instant::now();
                    loop {
                        let s = blocked.poll();
                        if s.collected {
                            assert!(s.first_error.is_some());
                            assert_eq!(s.terminal, Some(Terminal::Unproved));
                            break;
                        }
                        assert!(at.elapsed() < Duration::from_secs(5));
                        thread::yield_now();
                    }
                    assert_eq!(
                        std::fs::read(&marker).unwrap(),
                        exact_generation,
                        "blocked restart must retain exact marker generation"
                    );
                    println!(
                        "retained_marker case={case:?} bytes={} sha256={}",
                        exact_generation.len(),
                        marker_hash
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>()
                    );
                }
                println!(
                    "PASS case={case:?} exit={:?} terminal={:?} frames={} heartbeats={} stop={} locks={} first={:?} helper_first={:?} phase={} proof={}",
                    snapshot.exit,
                    snapshot.terminal,
                    snapshot.frames,
                    snapshot.heartbeats,
                    snapshot.stop_observed,
                    locks,
                    snapshot.first_error,
                    snapshot.helper_fault,
                    snapshot.helper_phase,
                    snapshot.proof_flags
                );
                if let Some(at) = first_delivery {
                    println!(
                        "generated_transport case={case:?} delivered={delivered} bytes={bytes} elapsed_us={} slow_consumer_ms=75 held_stale_frames=1 native_requests={:?}",
                        at.elapsed().as_micros(),
                        capability_platform::transfer_metrics()
                    );
                    let old = held.as_ref().unwrap();
                    assert!(
                        old.raw
                            .iter()
                            .enumerate()
                            .all(|(i, b)| *b == (i as u64 + old.metadata.sequence) as u8)
                    );
                }
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(100),
                "finite fixture failed to collect {case:?}"
            );
            thread::sleep(Duration::from_millis(
                if matches!(case, Fixture::FullFrames | Fixture::XgaFrames) {
                    75
                } else {
                    20
                },
            ));
        }
        drop(session);
        thread::sleep(Duration::from_millis(150));
        let handles = capability_platform::handle_count().unwrap();
        if case == Fixture::Normal {
            if let Some(warm) = warm_handles {
                assert_eq!(handles, warm, "warm parent HANDLE count must remain stable");
            } else {
                warm_handles = Some(handles);
            }
        }
        println!(
            "handles case={case:?} count={}",
            capability_platform::handle_count().unwrap()
        );
    }
    let counts = capability_platform::collection_counts();
    assert_eq!(counts.0, counts.1);
    assert_eq!(counts.0, cases.len() as u64);
    println!(
        "PASS collections={counts:?} handles_initial={initial} handles_final={} mailbox_frames=1 control_mailbox=1 control_bytes=128 frame_chunk_bytes=65536 bootstrap_bytes=2048",
        capability_platform::handle_count().unwrap()
    );
}
#[cfg(not(windows))]
fn main() {
    std::process::exit(78);
}
