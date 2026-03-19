use crate::error::{EngineError, EngineResult};

pub(crate) fn is_safe_identifier(s: &str, field: &str) -> EngineResult<()> {
    if s.is_empty() {
        return Ok(());
    }
    if s.chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        Ok(())
    } else {
        Err(EngineError::InvalidConfig(format!(
            "{field} contains unsafe characters: {s:?} (only alphanumeric, dash, underscore, dot allowed)"
        )))
    }
}

pub(crate) fn is_safe_path(s: &str, field: &str) -> EngineResult<()> {
    if s.is_empty() {
        return Ok(());
    }
    if s.chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | '+'))
    {
        Ok(())
    } else {
        Err(EngineError::InvalidConfig(format!(
            "{field} contains unsafe characters: {s:?}"
        )))
    }
}

pub(crate) fn is_safe_port(s: &str, field: &str) -> EngineResult<()> {
    // Accept "22", "22/tcp", "80:443/tcp", or named services like "ssh"
    if s.is_empty() {
        return Ok(());
    }
    if !s
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '/' | ':'))
    {
        return Err(EngineError::InvalidConfig(format!(
            "{field} contains unsafe characters: {s:?}"
        )));
    }
    // Validate that any numeric-looking component is a valid port (1-65535).
    // Strip trailing "/tcp" or "/udp" protocol suffix before checking.
    let base = s.split('/').next().unwrap_or(s);
    for part in base.split(':') {
        if let Ok(n) = part.parse::<u32>() {
            if n == 0 || n > 65535 {
                return Err(EngineError::InvalidConfig(format!(
                    "{field} port number {n} is out of range (1\u{2013}65535): {s:?}"
                )));
            }
        }
    }
    Ok(())
}

// Allows IPv4 (e.g. "8.8.8.8") and IPv6 (e.g. "2001:4860:4860::8888")
// in addition to hostnames.  Allows alphanumeric, dash, dot, colon,
// and bracket characters used in IPv6 literals like "[::1]".
pub(crate) fn is_safe_network_addr(s: &str, field: &str) -> EngineResult<()> {
    if s.is_empty() {
        return Ok(());
    }
    if s.chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '[' | ']'))
    {
        Ok(())
    } else {
        Err(EngineError::InvalidConfig(format!(
            "{field} contains unsafe characters: {s:?} (only alphanumeric, dash, \
             underscore, dot, colon, brackets allowed)"
        )))
    }
}

// Like is_safe_network_addr but also allows '/' for CIDR prefix notation
// (e.g. "192.168.1.10/24" or "2001:db8::1/64").
pub(crate) fn is_safe_cidr(s: &str, field: &str) -> EngineResult<()> {
    if s.is_empty() {
        return Ok(());
    }
    if s.chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '[' | ']' | '/'))
    {
        Ok(())
    } else {
        Err(EngineError::InvalidConfig(format!(
            "{field} contains unsafe characters: {s:?} (only alphanumeric, dash, \
             underscore, dot, colon, brackets, slash allowed)"
        )))
    }
}
