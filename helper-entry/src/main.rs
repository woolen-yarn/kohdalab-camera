//! Audited process entry: adoption is the first application action. No owners,
//! callbacks or application threads precede it; every rejection exits directly.
#[cfg(windows)]
fn main() {
    // SAFETY: this is the actual single-threaded executable entry, before any
    // application HANDLE ownership or thread creation. No retry on rejection.
    let adopted = unsafe { capability_platform::child_at_process_entry() };
    let (mode, capabilities) = match adopted {
        Ok(value) => value,
        Err(_) => std::process::exit(78),
    };
    std::process::exit(camera_lab::continuous_session::helper_dispatch(
        mode,
        capabilities,
    ));
}
#[cfg(not(windows))]
fn main() {
    std::process::exit(78);
}
