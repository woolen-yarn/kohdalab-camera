//! MT9T001 Rev D pp15,17,30–31. Public citations: UI_CONTROLS_REPORT.md.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    Preview,
    Full,
    Xga,
    Hd720,
    Hd960,
    #[default]
    FullFieldSkip,
}
/// Rev D pp15,19–20: fixed-origin, even, unbinned sensor windows.
/// Closed profile set: dimensions cannot silently become an unsupported mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: &'static str,
    pub name: &'static str,
    pub width: u16,
    pub height: u16,
    pub source_width: u16,
    pub source_height: u16,
    pub clock: u16,
    pub row_start: u16,
    pub column_start: u16,
    pub exposure_max: u16,
    pub label: &'static str,
}
impl Profile {
    pub fn window(self) -> [(u16, u16); 4] {
        [
            (1, self.row_start),
            (2, self.column_start),
            (3, self.source_height - 1),
            (4, self.source_width - 1),
        ]
    }
}
impl Mode {
    pub const ALL: [Self; 2] = [Self::FullFieldSkip, Self::Full];
    pub fn parse(value: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|m| m.profile().id == value)
            .ok_or_else(|| "Unknown acquisition profile".into())
    }
    pub fn from_dimensions(_width: u16, _height: u16) -> Result<Self, String> {
        Err("Dimensions do not identify source geometry; explicit versioned profile required".into())
    }
    pub fn profile(self) -> Profile {
        let (id, name, width, height, clock, label) = match self {
            Self::Preview => (
                "small",
                "VGA",
                640,
                480,
                0x8000,
                "VGA 640 × 480 · crop · historical ~82.2 fps",
            ),
            Self::Xga => (
                "xga",
                "XGA",
                1024,
                768,
                0x8000,
                "XGA 1024 × 768 · crop · experimental / unmeasured",
            ),
            Self::Hd720 => (
                "hd720",
                "HD 16:9",
                1280,
                720,
                0x8000,
                "HD 1280 × 720 · crop · experimental / unmeasured",
            ),
            Self::Hd960 => (
                "hd960",
                "HD 4:3",
                1280,
                960,
                0x8000,
                "HD 1280 × 960 (4:3) · crop · experimental / unmeasured",
            ),
            Self::Full => (
                "full/v1",
                "Full",
                2048,
                1536,
                0x8000,
                "Full 2048 × 1536 · unskipped · physical qualification pending",
            ),
            Self::FullFieldSkip => (
                "xga_full_field_skip/v1", "Full-field pair-skip", 1024, 768, 0x8000,
                "1024 × 768 Full-field SENSOR PAIR-SKIP · physically unverified",
            ),
        };
        Profile {
            id,
            name,
            width,
            height,
            source_width: if self == Self::FullFieldSkip {2048} else {width},
            source_height: if self == Self::FullFieldSkip {1536} else {height},
            clock,
            row_start: 20,
            column_start: 32,
            exposure_max: height,
            label,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub mode: Mode,
    pub exposure_rows: u16,
    /// Recommended 1–4x analog branch; digital gain stays unity.
    pub gain_eighths: u16,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::FullFieldSkip,
            exposure_rows: 120,
            gain_eighths: 8,
        }
    }
}
impl Settings {
    pub fn dimensions(self) -> (u16, u16) {
        let p = self.mode.profile();
        (p.width, p.height)
    }
    pub fn validate(self) -> Result<Self, String> {
        if !Mode::ALL.contains(&self.mode) {
            return Err("Legacy crop mode is private compatibility/test data, not current acquisition".into());
        }
        // Narrower than the sensor field, to bound exposure within a frame.
        if self.exposure_rows == 0 || self.exposure_rows > self.mode.profile().exposure_max {
            return Err(
                "Exposure must be 1..mode height rows; long exposures are unsupported".into(),
            );
        }
        if !(8..=32).contains(&self.gain_eighths) {
            return Err(
                "Gain must be 8..32 eighths (1–4x); other encodings are unsupported".into(),
            );
        }
        Ok(self)
    }
    pub fn clock(self) -> u16 {
        self.mode.profile().clock
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_geometry_cli_and_labels() {
        assert_eq!(Mode::ALL, [Mode::FullFieldSkip, Mode::Full]);
        for mode in Mode::ALL {
            let p=mode.profile(); assert_eq!(Mode::parse(p.id).unwrap(), mode);
            assert_eq!(p.window(),[(1,20),(2,32),(3,0x05ff),(4,0x07ff)]);
            assert!(Mode::from_dimensions(p.width,p.height).is_err());
        }
        for mode in [Mode::Preview,Mode::Xga,Mode::Hd720,Mode::Hd960] { assert!(Settings {mode,..Default::default()}.validate().is_err()); }
    }
    #[test]
    fn documented_encoding_and_restricted_bounds() {
        for mode in Mode::ALL {
            let s = Settings {
                mode,
                ..Default::default()
            };
            for exposure_rows in [1, s.dimensions().1] {
                for gain_eighths in [8, 16, 32] {
                    assert!(
                        Settings {
                            exposure_rows,
                            gain_eighths,
                            ..s
                        }
                        .validate()
                        .is_ok()
                    );
                }
            }
            for exposure_rows in [0, s.dimensions().1 + 1, u16::MAX] {
                assert!(Settings { exposure_rows, ..s }.validate().is_err());
            }
            for gain_eighths in [0, 7, 33, 0x51, 0x160, u16::MAX] {
                assert!(Settings { gain_eighths, ..s }.validate().is_err());
            }
        }
        assert_eq!(Settings::default().clock(), 0x8000);
        assert_eq!(
            Settings {
                mode: Mode::Full,
                ..Default::default()
            }
            .clock(),
            0x8000
        );
    }
}
