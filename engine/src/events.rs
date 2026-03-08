use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventPhase {
    Configure,
    Doctor,
    ReleaseLookup,
    Build,
    Scan,
    Test,
    Report,
    Inspect,
    Download,
    Verify,
    Inject,
    Diff,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineEvent {
    pub ts: DateTime<Utc>,
    pub level: EventLevel,
    pub phase: EventPhase,
    pub message: String,
    /// Current operation label shown in the progress panel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub substage: Option<String>,
    /// Completion percentage 0.0–100.0 when determinable; None = indeterminate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<f32>,
    /// Bytes transferred so far (for download/hash operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_done: Option<u64>,
    /// Total bytes expected (for download/hash operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_total: Option<u64>,
}

impl EngineEvent {
    pub fn debug(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Debug,
            phase,
            message: message.into(),
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    pub fn info(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Info,
            phase,
            message: message.into(),
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    pub fn warn(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Warn,
            phase,
            message: message.into(),
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    pub fn error(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Error,
            phase,
            message: message.into(),
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    /// Attach a substage label (fluent builder).
    #[must_use]
    pub fn with_substage(mut self, substage: impl Into<String>) -> Self {
        self.substage = Some(substage.into());
        self
    }

    /// Attach a completion percent 0–100 (fluent builder).
    #[must_use]
    pub fn with_percent(mut self, percent: f32) -> Self {
        self.percent = Some(percent.clamp(0.0, 100.0));
        self
    }

    /// Attach byte transfer progress and auto-compute percent (fluent builder).
    #[must_use]
    pub fn with_bytes(mut self, done: u64, total: u64) -> Self {
        self.bytes_done = Some(done);
        self.bytes_total = Some(total);
        if total > 0 {
            // Cast via f64 to avoid precision loss on large file sizes (u64 > 16 MiB).
            self.percent = Some((done as f64 / total as f64 * 100.0).clamp(0.0, 100.0) as f32);
        }
        self
    }

    /// Convenience: structured progress event for a named substage.
    pub fn progress(
        phase: EventPhase,
        substage: impl Into<String>,
        message: impl Into<String>,
        percent: Option<f32>,
    ) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Info,
            phase,
            message: message.into(),
            substage: Some(substage.into()),
            percent,
            bytes_done: None,
            bytes_total: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_bytes_sets_percent_zero_when_total_is_zero() {
        let ev = EngineEvent::info(EventPhase::Download, "downloading").with_bytes(0, 0);
        // total==0 guard: percent must NOT be set (would be div-by-zero)
        assert!(ev.percent.is_none());
    }

    #[test]
    fn with_bytes_half_progress_is_fifty_percent() {
        let ev = EngineEvent::info(EventPhase::Download, "downloading").with_bytes(500, 1000);
        let pct = ev.percent.expect("percent should be set");
        assert!((pct - 50.0).abs() < 0.01, "expected ~50%, got {pct}");
    }

    #[test]
    fn with_bytes_complete_is_100_percent() {
        let ev = EngineEvent::info(EventPhase::Download, "downloading").with_bytes(1000, 1000);
        let pct = ev.percent.expect("percent should be set");
        assert!((pct - 100.0).abs() < 0.01, "expected 100%, got {pct}");
    }

    #[test]
    fn with_bytes_no_precision_loss_on_large_files() {
        // 10 GiB file, 5 GiB done → exactly 50%
        let ten_gib: u64 = 10 * 1024 * 1024 * 1024;
        let ev =
            EngineEvent::info(EventPhase::Download, "downloading").with_bytes(ten_gib / 2, ten_gib);
        let pct = ev.percent.expect("percent should be set");
        // With u64->f32 direct cast this would be ~49.99998% due to mantissa loss.
        // With the f64 intermediate we get a value much closer to 50.0.
        assert!(
            (pct - 50.0).abs() < 0.01,
            "precision loss: expected ~50%, got {pct}"
        );
    }

    #[test]
    fn with_bytes_clamps_above_100() {
        // done > total (e.g. download size estimate was wrong)
        let ev = EngineEvent::info(EventPhase::Download, "downloading").with_bytes(2000, 1000);
        let pct = ev.percent.expect("percent should be set");
        assert!(
            (pct - 100.0).abs() < 0.01,
            "expected clamped 100%, got {pct}"
        );
    }

    #[test]
    fn info_event_has_info_level() {
        let ev = EngineEvent::info(EventPhase::Build, "msg");
        assert_eq!(ev.level, EventLevel::Info);
    }

    #[test]
    fn progress_event_sets_substage_and_percent() {
        let ev = EngineEvent::progress(EventPhase::Inject, "step1", "doing it", Some(42.0));
        assert_eq!(ev.substage.as_deref(), Some("step1"));
        assert_eq!(ev.percent, Some(42.0));
    }
}
