//! Discovery candidates only; USB identity never grants acquisition authority.
//!
//! A new entry also needs its own backend, driver policy and device qualification.
//! Transport-specific firmware, sensor and physical-location checks remain in
//! the existing backend rather than being weakened to a catalog lookup.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsbId {
    pub vendor: u16,
    pub product: u16,
}

impl std::fmt::Display for UsbId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:04x}:{:04x}", self.vendor, self.product)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraModel {
    /// Stable software identity, independent of a mutable USB product string.
    pub id: &'static str,
    pub manufacturer: &'static str,
    pub model: &'static str,
    pub usb: UsbId,
}

/// Known discovery candidates, not a claim of support for every matching unit.
pub const CAMERA_MODELS: &[CameraModel] = &[CameraModel {
    id: "shodensha-gr300",
    manufacturer: "SHODENSHA",
    model: "GR300",
    usb: UsbId {
        vendor: 0x0547,
        product: 0x4d33,
    },
}];

pub fn usb_candidate(vendor: u16, product: u16) -> Option<&'static CameraModel> {
    let usb = UsbId { vendor, product };
    CAMERA_MODELS.iter().find(|model| model.usb == usb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_retains_the_exact_existing_identity_scope() {
        let model = usb_candidate(0x0547, 0x4d33).unwrap();
        assert_eq!((model.manufacturer, model.model), ("SHODENSHA", "GR300"));
        assert_eq!(model.usb.to_string(), "0547:4d33");
        assert_eq!(CAMERA_MODELS.len(), 1);
        for (vendor, product) in [(0x0547, 0x4d32), (0x0546, 0x4d33), (0, 0), (0xffff, 0xffff)] {
            assert!(usb_candidate(vendor, product).is_none());
        }
    }

    #[test]
    fn catalog_keys_are_unambiguous() {
        for (index, model) in CAMERA_MODELS.iter().enumerate() {
            assert!(!model.id.is_empty());
            for other in &CAMERA_MODELS[index + 1..] {
                assert_ne!(model.id, other.id);
                assert_ne!(model.usb, other.usb);
            }
        }
    }
}
