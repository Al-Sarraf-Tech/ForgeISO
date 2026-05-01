//! Validators for GRUB bootloader fields.
//!
//! Logic preserved verbatim from the original monolithic
//! `engine/src/config/inject.rs::validate()`.

use crate::config::components::GrubConfig;
use crate::error::{EngineError, EngineResult};

// -- GRUB --------------------------------------------------------------------

pub(super) fn validate_grub(grub: &GrubConfig) -> EngineResult<()> {
    // GRUB -- default_entry and cmdline_extra are interpolated into sed s|...|...|
    // patterns (| delimiter).  Block shell metacharacters and | itself, but
    // allow / so users can specify UUID paths (e.g. rd.luks.uuid=/dev/sda2).
    if let Some(entry) = &grub.default_entry {
        if entry
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "grub_default contains shell metacharacters: {entry:?}"
            )));
        }
    }
    for param in &grub.cmdline_extra {
        if param
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "grub_cmdline contains shell metacharacters: {param:?}"
            )));
        }
    }
    // GRUB timeout -- written as a number into GRUB_TIMEOUT=N; unreasonably
    // large values produce unusable systems.  Cap at 3600 (1 hour).
    if let Some(t) = grub.timeout {
        if t > 3600 {
            return Err(EngineError::InvalidConfig(format!(
                "grub_timeout must be 0\u{2013}3600 seconds, got {t}"
            )));
        }
    }
    Ok(())
}
