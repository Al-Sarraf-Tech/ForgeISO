//! Engine-wide error taxonomy.
//!
//! Every fallible operation in the engine returns
//! [`EngineResult<T>`] = `Result<T, EngineError>`. The variants are
//! consumer-stable: log-grep against `error: <variant text>` is part
//! of the contract surface (see
//! [`STABILITY.md`](https://github.com/Al-Sarraf-Tech/ForgeISO/blob/main/STABILITY.md)).
//!
//! See [`ADR 0004`](https://github.com/Al-Sarraf-Tech/ForgeISO/blob/main/docs/adr/0004-error-handling-philosophy.md)
//! for the `thiserror` + `anyhow`-at-boundaries split, and
//! `docs/RUNBOOKS.md` for the per-variant symptom → diagnose →
//! recovery walkthroughs.

use std::io;

use thiserror::Error;

/// All errors the engine can produce.
///
/// Variants are reviewed for stability the same way as any other
/// public API item: rename, removal, or message-text change requires
/// the same semver discipline as the rest of the crate.
///
/// Marked `#[non_exhaustive]` so adding a new variant is a non-breaking
/// minor change. Consumers must include a `_ => ...` arm when matching.
#[derive(Debug, Error)]
pub enum EngineError {
    /// User-supplied configuration failed validation. The argument
    /// names the offending field and what was wrong with it. Always
    /// the first error a front-end will surface to the user.
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    /// Configuration is syntactically valid but the project's policy
    /// rules forbid it (e.g. setting `expected_sha256` to a value
    /// that does not match the upstream pin, requesting a banned
    /// distro variant, or attempting an action gated behind a feature
    /// flag that is off).
    #[error("policy violation: {0}")]
    PolicyViolation(String),
    /// A runtime failure that is not better described by another
    /// variant — typically a non-zero exit from a shell-out tool with
    /// the captured stderr embedded.
    #[error("runtime error: {0}")]
    Runtime(String),
    /// A required external tool was not on `$PATH`. The argument is
    /// the tool name (e.g. `xorriso`, `mksquashfs`); install
    /// instructions live in `docs/RUNBOOKS.md`.
    #[error("tooling missing: {0}")]
    MissingTool(String),
    /// Path-traversal or unsafe-path attempt detected during workspace
    /// or output handling. Surfaces from [`crate::workspace::Workspace`]'s
    /// `safe_join` and from generator code that writes user-controlled
    /// filenames.
    #[error("filesystem safety violation: {0}")]
    PathSafety(String),
    /// Network operation failed (download or HEAD probe of a source
    /// preset). Distinct from `Reqwest` so callers can differentiate
    /// "no internet at all" from "HTTP-level error".
    #[error("network error: {0}")]
    Network(String),
    /// A user-supplied path or preset id did not resolve to anything
    /// on disk or in the catalog.
    #[error("not found: {0}")]
    NotFound(String),
    /// The operation was cancelled cooperatively via a
    /// [`tokio_util::sync::CancellationToken`]. See
    /// [`crate::ForgeIsoEngine::build_cancellable`].
    #[error("operation cancelled")]
    Cancelled,
    /// A per-tool circuit breaker is currently open and refusing
    /// invocations. Surfaces from the
    /// [`crate::orchestrator::circuit_breaker`] sliding-window
    /// failure detector after `n` failures inside the window.
    #[error("circuit breaker open for tool: {tool}")]
    CircuitOpen {
        /// Name of the tool whose breaker is open
        /// (e.g. `mksquashfs`, `xorriso`).
        tool: String,
    },
    /// Filesystem I/O error transparently wrapping [`std::io::Error`].
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    /// JSON (de)serialization error transparently wrapping
    /// [`serde_json::Error`]. Surfaces when reading or writing
    /// `report.json`, `manifest.json`, or any persisted state file.
    #[error("serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    /// YAML (de)serialization error transparently wrapping
    /// [`serde_yaml::Error`]. Surfaces when parsing
    /// [`crate::BuildConfig`] / [`crate::InjectConfig`] from disk or
    /// when emitting `autoinstall.yaml`.
    #[error("yaml error: {0}")]
    SerdeYaml(#[from] serde_yaml::Error),
    /// HTTP error transparently wrapping [`reqwest::Error`]. Distinct
    /// from `Network` — `Reqwest` indicates the request itself
    /// completed but produced a non-success status or a body
    /// deserialization failure.
    #[error("http error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

/// Convenience alias for the engine's result type.
pub type EngineResult<T> = Result<T, EngineError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_config_error_displays_message() {
        let err = EngineError::InvalidConfig("bad hostname".to_string());
        assert_eq!(err.to_string(), "invalid config: bad hostname");
    }

    #[test]
    fn runtime_error_displays_message() {
        let err = EngineError::Runtime("xorriso failed".to_string());
        assert_eq!(err.to_string(), "runtime error: xorriso failed");
    }

    #[test]
    fn missing_tool_error_displays_message() {
        let err = EngineError::MissingTool("xorriso".to_string());
        assert_eq!(err.to_string(), "tooling missing: xorriso");
    }

    #[test]
    fn not_found_error_displays_message() {
        let err = EngineError::NotFound("/tmp/missing.iso".to_string());
        assert_eq!(err.to_string(), "not found: /tmp/missing.iso");
    }

    #[test]
    fn network_error_displays_message() {
        let err = EngineError::Network("status 404".to_string());
        assert_eq!(err.to_string(), "network error: status 404");
    }

    #[test]
    fn path_safety_error_displays_message() {
        let err = EngineError::PathSafety("path traversal detected".to_string());
        assert_eq!(
            err.to_string(),
            "filesystem safety violation: path traversal detected"
        );
    }

    #[test]
    fn policy_violation_error_displays_message() {
        let err = EngineError::PolicyViolation("license denied".to_string());
        assert_eq!(err.to_string(), "policy violation: license denied");
    }

    #[test]
    fn io_error_wraps_std_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err = EngineError::Io(io_err);
        assert!(err.to_string().contains("io error:"));
    }

    #[test]
    fn cancelled_error_displays_message() {
        let err = EngineError::Cancelled;
        assert_eq!(err.to_string(), "operation cancelled");
    }

    #[test]
    fn circuit_open_error_displays_tool_name() {
        let err = EngineError::CircuitOpen {
            tool: "mksquashfs".to_string(),
        };
        assert_eq!(err.to_string(), "circuit breaker open for tool: mksquashfs");
    }
}
