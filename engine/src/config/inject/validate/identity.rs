//! Validators for identity / locale fields.
//!
//! Logic preserved verbatim from the original monolithic
//! `engine/src/config/inject.rs::validate()`.

use crate::config::validation::is_safe_identifier;
use crate::error::{EngineError, EngineResult};

// -- Identity / locale -------------------------------------------------------

pub(super) fn validate_identity(
    hostname: Option<&String>,
    username: Option<&String>,
    realname: Option<&String>,
    timezone: Option<&String>,
    locale: Option<&String>,
    keyboard_layout: Option<&String>,
) -> EngineResult<()> {
    if let Some(h) = hostname {
        is_safe_identifier(h, "hostname")?;
    }
    if let Some(u) = username {
        is_safe_identifier(u, "username")?;
    }

    // Timezone -- written as a bare string into cloud-init YAML, Kickstart
    // `timezone` directive, and preseed `time/zone`.  Only IANA-style chars
    // are valid (e.g. "America/New_York", "UTC", "Etc/GMT+5").  Block
    // everything that is not alphanumeric, slash, underscore, dash, or plus.
    if let Some(tz) = timezone {
        if tz.is_empty() {
            return Err(EngineError::InvalidConfig(
                "timezone must not be blank".to_string(),
            ));
        }
        if !tz
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "timezone contains unsafe characters: {tz:?} \
                 (only alphanumeric, slash, underscore, dash, plus allowed)"
            )));
        }
    }

    // Locale -- written as a bare string into cloud-init YAML and installer
    // directives.  Standard glibc locale names use alphanumeric, dash,
    // underscore, and dot (e.g. "en_US.UTF-8", "de_DE.ISO-8859-1").
    if let Some(loc) = locale {
        if loc.is_empty() {
            return Err(EngineError::InvalidConfig(
                "locale must not be blank".to_string(),
            ));
        }
        if !loc
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "locale contains unsafe characters: {loc:?} \
                 (only alphanumeric, underscore, dash, dot allowed)"
            )));
        }
    }

    // Keyboard layout -- written into cloud-init YAML keyboard.layout.
    // XKB layout identifiers are alphanumeric plus dash and underscore
    // (e.g. "us", "de", "gb", "us-intl").
    if let Some(kb) = keyboard_layout {
        if kb.is_empty() {
            return Err(EngineError::InvalidConfig(
                "keyboard_layout must not be blank".to_string(),
            ));
        }
        if !kb
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "keyboard_layout contains unsafe characters: {kb:?} \
                 (only alphanumeric, dash, underscore allowed)"
            )));
        }
    }
    if let Some(r) = realname {
        // Realname can contain spaces
        if r.chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "realname contains shell metacharacters: {r:?}"
            )));
        }
    }

    Ok(())
}
