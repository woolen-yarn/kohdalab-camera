//! Startup qualification is independent of sensor format and requested settings.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Intent {
    #[serde(rename = "normal/v1")]
    Normal,
    #[serde(rename = "diagnostics/v1")]
    Diagnostics,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticStatus { NotRun, Passed, Failed, Incomplete }

#[derive(Clone, Copy)]
pub(crate) struct Binding {
    pub intent: Intent,
    pub session: [u8; 32],
    pub generation: u64,
}
impl Binding {
    pub fn acquisition(self, settings: crate::controls::Settings, sequence: u64) -> Result<crate::sensor_contract::Acquisition,String> {
        Ok(crate::sensor_contract::Acquisition::new(settings,self.session,self.generation,sequence)?.with_startup(self.intent))
    }
}

impl Intent {
    /// Only call after the complete recipe succeeds. A failed recipe publishes
    /// no frame; terminal status must use the actual completion evidence.
    pub fn completed(self) -> DiagnosticStatus {
        match self { Self::Normal => DiagnosticStatus::NotRun, Self::Diagnostics => DiagnosticStatus::Passed }
    }
    pub fn status(self, live_reached: bool, failed: bool) -> DiagnosticStatus {
        match self {
            Self::Normal => DiagnosticStatus::NotRun,
            Self::Diagnostics if failed => DiagnosticStatus::Failed,
            Self::Diagnostics if live_reached => DiagnosticStatus::Passed,
            Self::Diagnostics => DiagnosticStatus::Incomplete,
        }
    }
}
