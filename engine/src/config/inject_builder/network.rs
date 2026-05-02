//! Network-related setters: SSH, network DNS/NTP, proxy, IP/gateway,
//! firewall.

use super::InjectConfigBuilder;
use crate::config::components::{FirewallConfig, NetworkConfig, ProxyConfig, SshConfig};

impl InjectConfigBuilder {
    /// Configure SSH server installation and authorized-key injection.
    #[must_use]
    pub fn ssh(mut self, val: SshConfig) -> Self {
        self.ssh = Some(val);
        self
    }

    /// Set DNS and NTP server addresses applied to the installed system.
    #[must_use]
    pub fn network(mut self, val: NetworkConfig) -> Self {
        self.network = Some(val);
        self
    }

    /// Set HTTP/HTTPS proxy and no-proxy settings written to `/etc/environment` and apt config.
    #[must_use]
    pub fn proxy(mut self, val: ProxyConfig) -> Self {
        self.proxy = Some(val);
        self
    }

    /// Set a static IPv4 address in CIDR notation (e.g. `"192.168.1.10/24"`) instead of DHCP.
    #[must_use]
    pub fn static_ip(mut self, val: impl Into<String>) -> Self {
        self.static_ip = Some(val.into());
        self
    }

    /// Set the default gateway IP address used when a static IP is configured.
    #[must_use]
    pub fn gateway(mut self, val: impl Into<String>) -> Self {
        self.gateway = Some(val.into());
        self
    }

    /// Configure UFW (Ubuntu/Mint) or firewalld (Fedora) rules applied via late-command.
    #[must_use]
    pub fn firewall(mut self, val: FirewallConfig) -> Self {
        self.firewall = Some(val);
        self
    }
}
