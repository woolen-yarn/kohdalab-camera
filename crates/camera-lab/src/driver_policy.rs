//! Preserve installed Windows drivers. Never install or silently replace one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route { ExistingWinUsb, PreserveInstalledDriver, MissingDriver }

pub fn route(service: Option<&str>) -> Route {
    match service {
        Some(s) if s.eq_ignore_ascii_case("winusb") => Route::ExistingWinUsb,
        Some(s) if !s.is_empty() => Route::PreserveInstalledDriver,
        _ => Route::MissingDriver,
    }
}
impl Route {
    pub fn error(self) -> Option<&'static str> {
        match self {
            Self::ExistingWinUsb => None,
            Self::PreserveInstalledDriver => Some("Installed driver preserved. This release needs an existing WinUSB binding; a compatible vendor SDK backend is not available. No driver was changed."),
            Self::MissingDriver => Some("No supported Windows driver binding was found. No driver was installed or changed."),
        }
    }
}
pub fn connection_error(info: &nusb::DeviceInfo) -> Option<&'static str> {
    #[cfg(windows)] { route(info.driver()).error() }
    #[cfg(not(windows))] { let _=info; None }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn preserves_vendor_and_unknown_bindings_without_fallback() {
        assert_eq!(route(Some("WinUSB")),Route::ExistingWinUsb);
        for name in ["ManufacturerCamera","usbccgp","libusbK","unknown"] {
            assert_eq!(route(Some(name)),Route::PreserveInstalledDriver);
            assert!(route(Some(name)).error().is_some());
        }
        assert_eq!(route(None),Route::MissingDriver);
        assert_eq!(route(Some("")),Route::MissingDriver);
        assert!(route(Some("winusb")).error().is_none());
    }
}
