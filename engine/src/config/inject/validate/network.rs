//! Validators for proxy and network address fields.
//!
//! Logic preserved verbatim from the original monolithic
//! `engine/src/config/inject.rs::validate()`.

use crate::config::components::{NetworkConfig, ProxyConfig};
use crate::config::validation::{is_safe_cidr, is_safe_network_addr};
use crate::error::{EngineError, EngineResult};

// -- Network -----------------------------------------------------------------

pub(super) fn validate_proxy(proxy: &ProxyConfig) -> EngineResult<()> {
    // Proxy URLs -- written to /etc/environment via echo
    for (field, val) in [
        ("http_proxy", &proxy.http_proxy),
        ("https_proxy", &proxy.https_proxy),
    ] {
        if let Some(url) = val {
            if url
                .chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "{field} contains shell metacharacters: {url:?}"
                )));
            }
        }
    }

    // no_proxy entries -- written to /etc/environment
    for entry in &proxy.no_proxy {
        if entry
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "no_proxy contains shell metacharacters: {entry:?}"
            )));
        }
    }

    Ok(())
}

pub(super) fn validate_network(
    network: &NetworkConfig,
    static_ip: Option<&String>,
    gateway: Option<&String>,
) -> EngineResult<()> {
    // Static IP -- CIDR notation (e.g. "192.168.1.10/24") placed in cloud-init
    // netplan YAML, Kickstart `--ip=`, and preseed `netcfg/get_ipaddress`.
    if let Some(ip) = static_ip {
        is_safe_cidr(ip, "static_ip")?;
    }

    // Gateway -- plain IP or hostname placed in cloud-init routes and Kickstart
    // `--gateway=` directive.
    if let Some(gw) = gateway {
        is_safe_network_addr(gw, "gateway")?;
    }

    // DNS servers -- may be IPv4, IPv6, or hostnames.
    for dns in &network.dns_servers {
        is_safe_network_addr(dns, "dns_server")?;
    }

    // NTP servers -- may be IPv4, IPv6, or hostnames.
    for ntp in &network.ntp_servers {
        is_safe_network_addr(ntp, "ntp_server")?;
    }

    Ok(())
}
