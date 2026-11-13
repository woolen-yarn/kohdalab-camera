//! Product transactions have no transport, lease, or filesystem authority.
use crate::{
    Frame,
    controls::{Mode, Settings},
    startup::Intent,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applied {
    pub settings: Settings,
    pub intent: Intent,
    pub revision: u64,
    pub session: [u8; 32],
    pub sequence: u64,
}
#[derive(Clone, Copy)]
enum Phase {
    Full,
    Restore,
}
struct Transaction {
    original: Applied,
    expected: Applied,
    phase: Phase,
    since: Instant,
}
#[derive(Default)]
pub struct Controller {
    pub applied: Option<Applied>,
    transaction: Option<Transaction>,
    pub held: Option<Frame>,
    pub capture_result: String,
    pub restore_result: String,
    stopped: bool,
}
impl Controller {
    pub fn busy(&self) -> bool {
        self.transaction.is_some()
    }
    pub fn start(&mut self) {
        self.stopped = false;
        self.applied = None;
    }
    pub fn stop(&mut self) {
        self.stopped = true;
        if self.transaction.take().is_some() {
            self.restore_result = "Interrupted by Stop / Close; no automatic return".into();
            if self.held.is_none() {
                self.capture_result = "Interrupted before Full frame".into();
            }
        }
        self.applied = None;
    }
    pub fn begin(&mut self, now: Instant) -> Result<Applied, String> {
        if self.busy() || self.stopped {
            return Err("Capture unavailable during another transaction or Stop".into());
        }
        let original = self
            .applied
            .ok_or("Wait for a matching applied Live frame")?;
        let mut expected = original;
        expected.settings.mode = Mode::Full;
        self.held = None;
        self.capture_result = "Waiting for a new matching Full frame".into();
        self.restore_result = "Pending".into();
        self.transaction = Some(Transaction {
            original,
            expected,
            phase: Phase::Full,
            since: now,
        });
        Ok(expected)
    }
    pub fn requested(&mut self, revision: u64, now: Instant) {
        if let Some(t) = &mut self.transaction {
            if revision != t.expected.revision {
                t.expected.revision = revision;
                t.expected.sequence = 0;
                t.expected.session = [0; 32];
            }
            t.since = now;
        }
    }
    fn identity(f: &Frame, revision: u64, settings: Settings, intent: Intent) -> Option<Applied> {
        f.validate().ok()?;
        let a = f.metadata.acquisition.as_ref()?;
        if a.request_revision != Some(revision)
            || a.settings_generation != revision
            || a.settings().ok()? != settings
            || a.startup_intent != Some(intent)
        {
            return None;
        }
        Some(Applied {
            settings,
            intent,
            revision,
            session: a.session,
            sequence: a.sequence,
        })
    }
    /// Returns a restore request only after retaining the exact new Full frame.
    pub fn observe(
        &mut self,
        f: &Frame,
        revision: u64,
        settings: Settings,
        intent: Intent,
        now: Instant,
    ) -> Option<Applied> {
        if self.stopped {
            return None;
        }
        let current = Self::identity(f, revision, settings, intent)?;
        if let Some(last) = self.applied {
            if last.revision == current.revision
                && (last.session != current.session || current.sequence <= last.sequence)
            {
                return None;
            }
            if last.revision != current.revision && last.session == current.session {
                return None;
            }
        }
        if let Some(t) = &mut self.transaction {
            let e = t.expected;
            if current.revision != e.revision
                || current.settings != e.settings
                || current.intent != e.intent
                || (e.session != [0; 32]
                    && (current.session != e.session || current.sequence <= e.sequence))
            {
                return None;
            }
            self.applied = Some(current);
            match t.phase {
                Phase::Full => {
                    self.held = Some(f.clone());
                    self.capture_result = "New matching Full frame held".into();
                    self.restore_result = "Waiting for a new matching original Live frame".into();
                    t.phase = Phase::Restore;
                    t.expected = t.original;
                    // Already Full: require another sequence, without restarting.
                    if t.original.settings == current.settings {
                        t.expected = current;
                    }
                    t.since = now;
                    return Some(t.expected);
                }
                Phase::Restore => {
                    self.restore_result =
                        "Original applied Live settings restored; new matching frame verified"
                            .into();
                    self.transaction = None;
                }
            }
        } else {
            self.applied = Some(current);
        }
        None
    }
    /// A timeout/fault never certifies Live and never queues a blind restart.
    pub fn fail(&mut self, reason: &str) {
        if let Some(t) = self.transaction.take() {
            if matches!(t.phase, Phase::Full) {
                self.capture_result = format!("Full capture failed: {reason}");
            }
            self.restore_result =
                format!("Return to Live unverified: {reason}; Stop and inspect status");
        }
        self.applied = None;
        self.stopped = true;
    }
    pub fn tick(&mut self, now: Instant) -> bool {
        if self
            .transaction
            .as_ref()
            .is_some_and(|t| now.saturating_duration_since(t.since) > Duration::from_secs(65))
        {
            self.fail("matching-frame deadline");
            return true;
        }
        false
    }
}
