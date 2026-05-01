//! Network-related setters: SSH, network DNS/NTP, proxy, IP/gateway,
//! firewall.

use super::InjectConfigBuilder;
use crate::config::components::{FirewallConfig, NetworkConfig, ProxyConfig, SshConfig};

impl InjectConfigBuilder {
    #[must_use]
    pub fn ssh(mut self, val: SshConfig) -> Self {
        self.ssh = Some(val);
        self
    }

    #[must_use]
    pub fn network(mut self, val: NetworkConfig) -> Self {
        self.network = Some(val);
        self
    }

    #[must_use]
    pub fn proxy(mut self, val: ProxyConfig) -> Self {
        self.proxy = Some(val);
        self
    }

    #[must_use]
    pub fn static_ip(mut self, val: impl Into<String>) -> Self {
        self.static_ip = Some(val.into());
        self
    }

    #[must_use]
    pub fn gateway(mut self, val: impl Into<String>) -> Self {
        self.gateway = Some(val.into());
        self
    }

    #[must_use]
    pub fn firewall(mut self, val: FirewallConfig) -> Self {
        self.firewall = Some(val);
        self
    }
}
