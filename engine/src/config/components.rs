use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SshConfig {
    #[serde(default)]
    pub authorized_keys: Vec<String>,
    /// None = engine decides (false if keys present, true otherwise)
    #[serde(default)]
    pub allow_password_auth: Option<bool>,
    /// None = defaults to true (install openssh-server)
    #[serde(default)]
    pub install_server: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkConfig {
    #[serde(default)]
    pub dns_servers: Vec<String>,
    #[serde(default)]
    pub ntp_servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserConfig {
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default)]
    pub sudo_nopasswd: bool,
    #[serde(default)]
    pub sudo_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FirewallConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub default_policy: Option<String>,
    #[serde(default)]
    pub allow_ports: Vec<String>,
    #[serde(default)]
    pub deny_ports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyConfig {
    #[serde(default)]
    pub http_proxy: Option<String>,
    #[serde(default)]
    pub https_proxy: Option<String>,
    #[serde(default)]
    pub no_proxy: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SwapConfig {
    pub size_mb: u32,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub swappiness: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerConfig {
    #[serde(default)]
    pub docker: bool,
    #[serde(default)]
    pub podman: bool,
    #[serde(default)]
    pub docker_users: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GrubConfig {
    #[serde(default)]
    pub timeout: Option<u32>,
    #[serde(default)]
    pub cmdline_extra: Vec<String>,
    #[serde(default)]
    pub default_entry: Option<String>,
}
