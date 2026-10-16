#![forbid(unsafe_code)]
//! Bounded experimental ScopeTek-family identity read. See PHYSICAL_REPORT.md.
use nusb::MaybeFuture;
use std::io::Write;
fn run() -> Result<(), String> {
    camera_lab::progress::publish(camera_lab::progress::Stage::Identity);
    let port = std::env::args()
        .nth(1)
        .ok_or("Usage: physical-probe <exact-port-chain>")?;
    if port == "--list" {
        for d in nusb::list_devices().wait().map_err(|e| e.to_string())? {
            if d.vendor_id() == 0x0547 && d.product_id() == 0x4d33 {
                println!("{d:?}");
            }
        }
        return Ok(());
    }
    // Validate before opening any device.
    if let Some(operation) = std::env::args().nth(2)
        && !matches!(
            operation.as_str(),
            "--capture" | "--identify" | "--prepare-identify"
        )
    {
        return Err("Unknown probe operation".into());
    }
    let settings = camera_lab::controls::Settings {
        mode: if std::env::args().nth(2).as_deref() == Some("--capture") {
            camera_lab::controls::Mode::parse(
                &std::env::args()
                    .nth(3)
                    .ok_or("Missing acquisition profile")?,
            )?
        } else {
            camera_lab::controls::Mode::Preview
        },
        exposure_rows: std::env::args()
            .nth(6)
            .map(|s| s.parse::<u16>())
            .transpose()
            .map_err(|_| "Invalid exposure encoding")?
            .unwrap_or(100),
        gain_eighths: std::env::args()
            .nth(7)
            .map(|s| s.parse::<u16>())
            .transpose()
            .map_err(|_| "Invalid gain encoding")?
            .unwrap_or(8),
    }
    .validate()?;
    let mut matches = nusb::list_devices()
        .wait()
        .map_err(|e| e.to_string())?
        .filter(|d| d.vendor_id() == 0x0547 && d.product_id() == 0x4d33)
        .filter(|d| std::env::args().nth(5).is_none_or(|bus| d.bus_id() == bus))
        .filter(|d| {
            d.port_chain()
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(".")
                == port
        });
    let info = matches
        .next()
        .ok_or("No device at the explicitly selected port")?;
    if matches.next().is_some() {
        return Err("Ambiguous identity".into());
    }
    println!("identity={info:?}");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let capture = std::env::args().nth(2).as_deref() == Some("--capture");
    if capture {
        let output = std::env::args()
            .nth(4)
            .ok_or("Missing new capture directory")?;
        if std::path::Path::new(&output).exists() {
            return Err("Capture directory must be new".into());
        }
    }
    let prepare = capture || std::env::args().nth(2).as_deref() == Some("--prepare-identify");
    let legacy = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("physical-camera.lock");
    if legacy.exists() {
        return Err(
            "Legacy recovery lock exists; evidence review required; no automatic migration".into(),
        );
    }
    let lock = camera_lab::coordination::acquire(
        &camera_lab::coordination::user_root()?,
        &format!("0547:4d33:{}:{}", info.bus_id(), port),
    )?;
    let device = info
        .open()
        .wait()
        .map_err(|e| format!("open failed: {e}"))?;
    println!("open=ok");
    for config in device.configurations() {
        println!("configuration_bytes={:02x?}", config.as_bytes());
    }
    let config = device.active_configuration().map_err(|e| e.to_string())?;
    const EXPECTED: &[u8] = &[
        9, 2, 32, 0, 1, 1, 0, 128, 50, 9, 4, 0, 0, 2, 255, 0, 0, 0, 7, 5, 129, 3, 64, 0, 0, 7, 5,
        130, 2, 0, 2, 0,
    ];
    if config.as_bytes() != EXPECTED
        || info.product_string() != Some("3.1M USB2.0 Camera")
        || info.device_version() != 0
        || info.serial_number().is_some()
    {
        return Err(
            "Physical specimen descriptor gate failed; no vendor operations permitted".into(),
        );
    }
    let interfaces: Vec<_> = config.interfaces().collect();
    if interfaces.len() != 1 {
        return Err("Expected one interface; no claim attempted".into());
    }
    let number = interfaces[0].interface_number();
    println!("claim_interface={number}");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let interface = device
        .claim_interface(number)
        .wait()
        .map_err(|e| format!("claim failed: {e}"))?;
    let mut preparation = camera_lab::acquisition_recipe::CallerPreparation::default();
    if prepare {
        // Family transport only: Linux touptek sd_start and public ScopeTek
        // packet 5..10. No sensor programming or stream-enable operation.
        for value in [1, 0, 1] {
            if capture {
                let root = std::path::PathBuf::from(
                    std::env::args().nth(4).ok_or("Missing capture directory")?,
                );
                if camera_lab::physical::cancellation(&root).is_err() {
                    break; // trial receives zero/partial preparation truthfully
                }
            }
            if let Err(error) = preparation.dispatch_bridge(|| bridge_state(&interface, value)) {
                if !capture { return Err(error); }
                break;
            }
        }
    }
    let capture_cancelled = capture
        && camera_lab::physical::cancellation(&std::path::PathBuf::from(
            std::env::args().nth(4).ok_or("Missing capture directory")?,
        ))
        .is_err();
    if (prepare || std::env::args().nth(2).as_deref() == Some("--identify")) && !capture_cancelled && preparation.failure.is_none() {
        use nusb::transfer::{ControlIn, ControlType, Recipient};
        // Transport hypothesis: public ScopeTek capture packet 15/16 uses
        // C0/0A, value=0, index=sensor address, length=3, big endian + 08.
        // Sensor address 00 and expected ID 1621: Micron datasheet Table 5.
        // No reset, bridge preparation, or register-write request is sent.
        println!("identity_read=C0/0A value=0000 index=0000 length=3 timeout_ms=500");
        std::io::stdout().flush().map_err(|e| e.to_string())?;
        let identity = preparation.dispatch_identity(|| {
        let reply = interface
            .control_in(
                ControlIn {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Device,
                    request: 0x0a,
                    value: 0,
                    index: 0,
                    length: 3,
                },
                std::time::Duration::from_millis(500),
            )
            .wait()
            .map_err(|_| camera_lab::diagnostic::SensorFailure::ReadTransport.text().to_owned());
        reply.and_then(|bytes| {
            if prepare && !capture {
                bridge_state(&interface, 1)?;
            }
            println!("identity_reply={bytes:02x?}");
            if bytes.len() != 3 { return Err(camera_lab::diagnostic::SensorFailure::ReadLength.text().into()); }
            if bytes[2] != 8 { return Err(camera_lab::diagnostic::SensorFailure::ReadStatus.text().into()); }
            if bytes[..2] != [0x16, 0x21] { return Err(camera_lab::diagnostic::SensorFailure::IdentityMismatch.text().into()); }
            Ok(())
        })
        });
        match identity {
            Err(error) if capture => preparation.failure = Some(error),
            Err(error) => return Err(error),
            Ok(()) => {
                if prepare && capture {
                    preparation.stop_attempted = true; // existing bounded STOP; no second STOP on failure
                    if let Err(error) = preparation.dispatch_bridge(|| bridge_state(&interface, 1)) {
                        if !capture { return Err(error); }
                    }
                }
                println!("sensor=MT9T001 identity matched; acquisition still disabled");
            }
        }
    }
    let mut session = camera_lab::acquisition_telemetry::Session::default();
    let mut telemetry_root = None;
    let mut outcome = Ok(());
    let mut allow_unlock = !capture;
    if capture {
        let root = std::env::args()
            .nth(4)
            .ok_or("Capture requires mode and new output directory")?;
        let result = camera_lab::physical::trial(
            &interface,
            settings.dimensions().0,
            settings.dimensions().1,
            std::path::Path::new(&root),
            settings,
            |v| bridge_state(&interface, v),
            &mut session,
            preparation,
        );
        allow_unlock = std::path::Path::new(&root).join("CLEANUP_OK").is_file();
        telemetry_root = Some(std::path::PathBuf::from(root));
        outcome = result;
    }
    println!("claim=ok; trial ended");
    let (outcome, _telemetry_failures) = camera_lab::acquisition_telemetry::release_and_persist(
        outcome,
        interface,
        device,
        &session,
        telemetry_root.as_deref(),
        &mut std::io::stderr(),
    );
    if allow_unlock {
        std::fs::remove_file(&lock).map_err(|e| e.to_string())?;
    }
    outcome?;
    println!("cleanup=handle dropped");
    Ok(())
}
fn bridge_state(interface: &nusb::Interface, value: u16) -> Result<(), String> {
    use nusb::transfer::{ControlOut, ControlType, Recipient};
    println!("bridge_state=40/01 value={value:04x} index=000f length=0 timeout_ms=500");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    interface
        .control_out(
            ControlOut {
                control_type: ControlType::Vendor,
                recipient: Recipient::Device,
                request: 1,
                value,
                index: 0x0f,
                data: &[],
            },
            std::time::Duration::from_millis(500),
        )
        .wait()
        .map_err(|e| {
            format!("bridge state failed: {e}; stop writes; physical reconnection required")
        })?;
    // Conservative engineering settling allowance, not a datasheet requirement.
    std::thread::sleep(std::time::Duration::from_millis(20));
    println!("bridge_state=ack");
    Ok(())
}
fn main() {
    // Fail before the legacy watchdog, runtime HANDLE owners, or enumeration.
    // No duration removal is enabled until private bootstrap, parent worker
    // supervision and terminal session proof are fully connected.
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--continuous")) {
        let _ = writeln!(
            std::io::stderr(),
            "Continuous helper bootstrap is not integrated; acquisition disabled"
        );
        std::process::exit(78);
    }
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(60));
        eprintln!("deadline exceeded; STOP WRITES; physical reconnection required");
        std::process::exit(124);
    });
    if let Err(e) = run() {
        eprintln!("STOP: {e}; no simulation fallback");
        std::process::exit(camera_lab::physical_preview::helper_error_exit(&e));
    }
}
