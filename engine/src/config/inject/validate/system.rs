//! Validators for user, services, firewall, sysctl.
//!
//! Logic preserved verbatim from the original monolithic
//! `engine/src/config/inject.rs::validate()`.

use crate::config::components::{FirewallConfig, UserConfig};
use crate::config::validation::{is_safe_identifier, is_safe_path, is_safe_port};
use crate::error::{EngineError, EngineResult};

// -- Users / services / firewall / sysctl / sudo -----------------------------

pub(super) fn validate_user_basics(user: &UserConfig) -> EngineResult<()> {
    for g in &user.groups {
        is_safe_identifier(g, "group")?;
    }
    if let Some(shell) = &user.shell {
        is_safe_path(shell, "shell")?;
    }
    Ok(())
}

pub(super) fn validate_user_sudo(user: &UserConfig) -> EngineResult<()> {
    // Sudo commands -- these are written into sudoers, so block metacharacters
    // that could break sudoers syntax or inject shell commands.
    for cmd in &user.sudo_commands {
        if cmd
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "sudo_command contains shell metacharacters: {cmd:?}"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_services(
    enable_services: &[String],
    disable_services: &[String],
) -> EngineResult<()> {
    for svc in enable_services {
        is_safe_identifier(svc, "enable_service")?;
    }
    for svc in disable_services {
        is_safe_identifier(svc, "disable_service")?;
    }
    Ok(())
}

pub(super) fn validate_firewall(firewall: &FirewallConfig) -> EngineResult<()> {
    if let Some(policy) = &firewall.default_policy {
        is_safe_identifier(policy, "firewall_policy")?;
    }
    for port in &firewall.allow_ports {
        is_safe_port(port, "allow_port")?;
    }
    for port in &firewall.deny_ports {
        is_safe_port(port, "deny_port")?;
    }
    Ok(())
}

pub(super) fn validate_sysctl(sysctl: &[(String, String)]) -> EngineResult<()> {
    for (key, val) in sysctl {
        is_safe_identifier(key, "sysctl key")?;
        // Sysctl values can be numeric or simple strings
        if val
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "sysctl value contains shell metacharacters: {val:?}"
            )));
        }
    }
    Ok(())
}
