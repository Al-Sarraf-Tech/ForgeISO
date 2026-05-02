//! Streaming engine event feed.
//!
//! Long-running engine operations ([`crate::ForgeIsoEngine::build`],
//! `verify_iso`, `scan_iso`, `test_iso`) publish a sequence of
//! [`EngineEvent`] records on a `tokio::sync::broadcast` channel that
//! front-ends subscribe to via
//! [`crate::ForgeIsoEngine::subscribe`].
//!
//! Each event carries:
//!
//! - an [`EventLevel`] (Debug/Info/Warn/Error) — log-level classification
//! - an [`EventPhase`] (Configure/Resolve/Verify/Build/Scan/Test/Report) —
//!   pipeline phase, used by progress bars and the GUI step rail
//! - an [`EventKind`] discriminator and message text
//!
//! Events are also rendered into the JSON log file at
//! `<FORGEISO_LOG_DIR or $XDG_STATE_HOME/forgeiso>/forgeiso.log.<date>`
//! by the observability layer, so log-tailing from `jq` produces the
//! same view the GUI sees.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Severity classification of an [`EngineEvent`]. Maps 1:1 onto the
/// `tracing` crate's level taxonomy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventLevel {
    /// Verbose internal detail — only surfaced when `RUST_LOG=debug`.
    Debug,
    /// Normal progress information; the default for in-band events.
    Info,
    /// Recoverable anomaly that did not abort the operation.
    Warn,
    /// The operation has failed or is about to fail.
    Error,
}

/// Pipeline phase an event belongs to. Drives the GUI step rail and
/// progress bar grouping. New phases may be added in a minor; consumer
/// code should use `_ => ...` arms when matching.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventPhase {
    /// Loading and validating user-supplied configuration.
    Configure,
    /// Pre-flight environment check (tool availability, write permissions).
    Doctor,
    /// Probing upstream for the latest release of a source preset.
    ReleaseLookup,
    /// Core build pipeline (extract, inject, repack).
    Build,
    /// Optional security scan running after a successful build.
    Scan,
    /// Optional boot-test running after a successful build.
    Test,
    /// Finalizing the build report and writing `report.{json,html}`.
    Report,
    /// Read-only inspection of a source ISO ([`crate::ForgeIsoEngine::inspect_source`]).
    Inspect,
    /// Source ISO download from a URL or preset.
    Download,
    /// SHA-256 / signature verification against a pinned hash.
    Verify,
    /// Writing autoinstall / kickstart / preseed files into the workspace.
    Inject,
    /// Computing the structural diff between two ISOs.
    Diff,
    /// Terminal completion event for a successful operation.
    Complete,
}

/// Semantic event kind — allows UI consumers to react to structured lifecycle
/// events without parsing message strings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum EventKind {
    /// Default: a plain log message.
    #[default]
    Log,
    /// Progress update (percent, bytes, substage already on EngineEvent).
    Progress,
    /// A phase is starting — UI can show a transition.
    PhaseStart {
        /// Short label for the phase (e.g. `extract`, `repack`).
        label: String,
    },
    /// A phase completed.
    PhaseEnd {
        /// `true` if the phase finished without raising an error.
        success: bool,
    },
    /// An artifact (ISO, report, etc.) is ready at the given path.
    ArtifactReady {
        /// Absolute path of the freshly produced artifact.
        path: PathBuf,
    },
    /// A config field passed or failed validation.
    ValidationResult {
        /// Dotted field path being validated (e.g. `network.dns_servers`).
        field: String,
        /// `Some(message)` when validation failed; `None` when passed.
        error: Option<String>,
    },
}

/// One event on the engine's broadcast channel.
///
/// Constructed via the [`EngineEvent::debug`] / `info` / `warn` /
/// `error` constructors plus the `with_*` fluent attachers and the
/// lifecycle helpers ([`EngineEvent::phase_start`],
/// [`EngineEvent::phase_end`], [`EngineEvent::artifact`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineEvent {
    /// UTC timestamp at which the event was created.
    pub ts: DateTime<Utc>,
    /// Severity classification.
    pub level: EventLevel,
    /// Pipeline phase the event belongs to.
    pub phase: EventPhase,
    /// Human-readable message rendered into the GUI activity log and
    /// the JSON tracing file.
    pub message: String,
    /// Semantic event kind for structured UI handling.
    #[serde(default)]
    pub kind: EventKind,
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
    /// Construct a `Debug`-level event for verbose internal detail. Surfaced
    /// only when `RUST_LOG=debug` (or the equivalent tracing filter) is set.
    pub fn debug(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Debug,
            phase,
            message: message.into(),
            kind: EventKind::Log,
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    /// Construct an `Info`-level event — the default for in-band progress
    /// reporting that should be visible to the user in the GUI activity log.
    pub fn info(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Info,
            phase,
            message: message.into(),
            kind: EventKind::Log,
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    /// Construct a `Warn`-level event for a recoverable anomaly that did not
    /// abort the operation (e.g. fallback path taken, optional tool missing).
    pub fn warn(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Warn,
            phase,
            message: message.into(),
            kind: EventKind::Log,
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    /// Construct an `Error`-level event for a failure that aborted (or is
    /// about to abort) the operation. The accompanying [`EngineError`]
    /// returned by the failing call carries the structured error variant.
    pub fn error(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Error,
            phase,
            message: message.into(),
            kind: EventKind::Log,
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
            kind: EventKind::Progress,
            substage: Some(substage.into()),
            percent,
            bytes_done: None,
            bytes_total: None,
        }
    }

    // ── Lifecycle event constructors ────────────────────────────────────

    /// Signal that a phase is starting — UI can show transitions.
    pub fn phase_start(phase: EventPhase, label: impl Into<String>) -> Self {
        let label_str = label.into();
        Self {
            ts: Utc::now(),
            level: EventLevel::Info,
            phase,
            message: format!("Starting: {label_str}"),
            kind: EventKind::PhaseStart { label: label_str },
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    /// Signal that a phase completed.
    pub fn phase_end(phase: EventPhase, success: bool) -> Self {
        Self {
            ts: Utc::now(),
            level: if success {
                EventLevel::Info
            } else {
                EventLevel::Error
            },
            phase,
            message: if success {
                "Phase complete".to_string()
            } else {
                "Phase failed".to_string()
            },
            kind: EventKind::PhaseEnd { success },
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    /// Signal that an artifact (ISO, report, etc.) is ready.
    pub fn artifact(phase: EventPhase, path: impl Into<PathBuf>) -> Self {
        let p = path.into();
        Self {
            ts: Utc::now(),
            level: EventLevel::Info,
            phase,
            message: format!("Artifact ready: {}", p.display()),
            kind: EventKind::ArtifactReady { path: p },
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    /// Attach a semantic event kind (fluent builder).
    #[must_use]
    pub fn with_kind(mut self, kind: EventKind) -> Self {
        self.kind = kind;
        self
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

    #[test]
    fn debug_event_uses_debug_level() {
        let ev = EngineEvent::debug(EventPhase::Configure, "hi");
        assert_eq!(ev.level, EventLevel::Debug);
        assert_eq!(ev.phase, EventPhase::Configure);
        assert_eq!(ev.message, "hi");
    }

    #[test]
    fn warn_event_uses_warn_level() {
        let ev = EngineEvent::warn(EventPhase::Doctor, "watch out");
        assert_eq!(ev.level, EventLevel::Warn);
    }

    #[test]
    fn error_event_uses_error_level() {
        let ev = EngineEvent::error(EventPhase::Build, "boom");
        assert_eq!(ev.level, EventLevel::Error);
    }

    #[test]
    fn with_substage_attaches_label_and_returns_self() {
        let ev = EngineEvent::info(EventPhase::Build, "x").with_substage("step-foo");
        assert_eq!(ev.substage.as_deref(), Some("step-foo"));
    }

    #[test]
    fn with_percent_clamps_to_valid_range() {
        let too_low = EngineEvent::info(EventPhase::Build, "x").with_percent(-5.0);
        assert_eq!(too_low.percent, Some(0.0));
        let too_high = EngineEvent::info(EventPhase::Build, "x").with_percent(150.0);
        assert_eq!(too_high.percent, Some(100.0));
    }

    #[test]
    fn phase_start_event_carries_phasestart_kind_with_label() {
        let ev = EngineEvent::phase_start(EventPhase::Inject, "config-ubuntu");
        assert_eq!(ev.level, EventLevel::Info);
        match &ev.kind {
            EventKind::PhaseStart { label } => assert_eq!(label, "config-ubuntu"),
            other => panic!("expected PhaseStart kind, got {other:?}"),
        }
        assert!(ev.message.contains("config-ubuntu"));
    }

    #[test]
    fn phase_end_success_sets_info_level_and_kind() {
        let ev = EngineEvent::phase_end(EventPhase::Build, true);
        assert_eq!(ev.level, EventLevel::Info);
        match &ev.kind {
            EventKind::PhaseEnd { success } => assert!(*success),
            other => panic!("expected PhaseEnd kind, got {other:?}"),
        }
    }

    #[test]
    fn phase_end_failure_sets_error_level() {
        let ev = EngineEvent::phase_end(EventPhase::Build, false);
        assert_eq!(ev.level, EventLevel::Error);
        match ev.kind {
            EventKind::PhaseEnd { success } => assert!(!success),
            _ => panic!("expected PhaseEnd kind"),
        }
    }

    #[test]
    fn artifact_event_carries_artifactready_kind_with_path() {
        let ev = EngineEvent::artifact(EventPhase::Complete, std::path::PathBuf::from("/x.iso"));
        match &ev.kind {
            EventKind::ArtifactReady { path } => {
                assert_eq!(path, &std::path::PathBuf::from("/x.iso"));
            }
            other => panic!("expected ArtifactReady, got {other:?}"),
        }
        assert!(ev.message.contains("/x.iso"));
    }

    #[test]
    fn with_kind_overrides_default_log_kind() {
        let ev =
            EngineEvent::info(EventPhase::Build, "msg").with_kind(EventKind::ValidationResult {
                field: "hostname".into(),
                error: Some("bad chars".into()),
            });
        match ev.kind {
            EventKind::ValidationResult { field, error } => {
                assert_eq!(field, "hostname");
                assert_eq!(error.as_deref(), Some("bad chars"));
            }
            _ => panic!("with_kind must replace kind"),
        }
    }

    #[test]
    fn event_round_trips_through_json_serialization() {
        let ev = EngineEvent::warn(EventPhase::Diff, "delta")
            .with_substage("compare")
            .with_percent(75.0);
        let json = serde_json::to_string(&ev).expect("serialize");
        let back: EngineEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.level, EventLevel::Warn);
        assert_eq!(back.phase, EventPhase::Diff);
        assert_eq!(back.substage.as_deref(), Some("compare"));
        assert_eq!(back.percent, Some(75.0));
    }
}
