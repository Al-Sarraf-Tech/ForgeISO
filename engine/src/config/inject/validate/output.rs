//! Validators for output filename, ISO label, expected hash.
//!
//! Logic preserved verbatim from the original monolithic
//! `engine/src/config/inject.rs::validate()`.

use crate::error::{EngineError, EngineResult};

// -- Output / labels / hashes ------------------------------------------------

pub(super) fn validate_output_label(output_label: Option<&String>) -> EngineResult<()> {
    // output_label -- used as the ISO volume label (written to xorriso -V).
    // Must follow the same rules as BuildConfig: non-empty, <= 32 ASCII chars.
    if let Some(label) = output_label {
        let label = label.trim();
        if label.is_empty() {
            return Err(EngineError::InvalidConfig(
                "output_label must not be blank".to_string(),
            ));
        }
        if label.len() > 32 {
            return Err(EngineError::InvalidConfig(format!(
                "output_label is too long ({} chars, max 32)",
                label.len()
            )));
        }
        if !label.is_ascii() {
            return Err(EngineError::InvalidConfig(
                "output_label must contain only ASCII characters".to_string(),
            ));
        }
        if label.chars().any(|c| c.is_ascii_control()) {
            return Err(EngineError::InvalidConfig(
                "output_label must not contain control characters".to_string(),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_out_name(out_name: &str) -> EngineResult<()> {
    // out_name -- used as a filename component joined with the output directory.
    // Block path separators (/ and \) to prevent writing outside the workspace.
    if !out_name.trim().is_empty() {
        let name = out_name.trim();
        if name.contains('/') || name.contains('\\') {
            return Err(EngineError::InvalidConfig(format!(
                "out_name must be a plain filename, not a path: {name:?}"
            )));
        }
        // Also block shell metacharacters in case the name is passed to xorriso.
        if name
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "out_name contains shell metacharacters: {name:?}"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_sha256(expected_sha256: Option<&String>) -> EngineResult<()> {
    // expected_sha256 -- must be exactly 64 lowercase hex characters if provided.
    // A non-hex value would cause a confusing "SHA-256 mismatch" error at
    // download time rather than a clear "invalid format" error at config time.
    if let Some(sha) = expected_sha256 {
        let sha = sha.trim().to_ascii_lowercase();
        if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(EngineError::InvalidConfig(format!(
                "expected_sha256 must be a 64-character hex string, got {:?} ({} chars)",
                sha,
                sha.len()
            )));
        }
    }
    Ok(())
}
