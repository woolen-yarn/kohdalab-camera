//! Production parent/helper continuous-session transport. The Windows entry shim
//! supplies validated OS capabilities; the GUI need not own native IO or wait.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
#[cfg(not(windows))]
pub fn unsupported() -> Result<(), String> {
    Err("Continuous sessions require Windows capabilities".into())
}

#[cfg(all(test, windows))]
pub(crate) use windows::assert_dual_failure_mapping;
