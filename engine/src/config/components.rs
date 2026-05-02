use serde::{Deserialize, Serialize};

/// SSH server installation and authorized-key configuration injected into the target system.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SshConfig {
    /// Public SSH keys written to the installed user's `~/.ssh/authorized_keys`.
    #[serde(default)]
    pub authorized_keys: Vec<String>,
    /// Whether to allow SSH password authentication. None = engine decides (false if keys present, true otherwise)
    #[serde(default)]
    pub allow_password_auth: Option<bool>,
    /// Whether to install `openssh-server` on the target. None = defaults to true (install openssh-server)
    #[serde(default)]
    pub install_server: Option<bool>,
}

/// Network-layer settings applied during installation: DNS resolvers and NTP servers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkConfig {
    /// DNS nameserver addresses injected into the Netplan or preseed network configuration.
    #[serde(default)]
    pub dns_servers: Vec<String>,
    /// NTP server hostnames written to `/etc/systemd/timesyncd.conf` via a late-command.
    #[serde(default)]
    pub ntp_servers: Vec<String>,
}

/// Additional attributes applied to the primary user account created during installation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserConfig {
    /// Supplementary groups the user is added to (e.g. `["docker", "libvirt"]`).
    #[serde(default)]
    pub groups: Vec<String>,
    /// Login shell path (e.g. `/bin/zsh`). Defaults to the distro's standard shell if `None`.
    #[serde(default)]
    pub shell: Option<String>,
    /// Grant full passwordless sudo (`NOPASSWD:ALL`) when `true`.
    #[serde(default)]
    pub sudo_nopasswd: bool,
    /// Specific command paths granted passwordless sudo (ignored when `sudo_nopasswd` is `true`).
    #[serde(default)]
    pub sudo_commands: Vec<String>,
}

/// UFW (Ubuntu/Mint) or firewalld (Fedora) configuration applied via late-command.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FirewallConfig {
    /// Enable and start the firewall service during first boot.
    #[serde(default)]
    pub enabled: bool,
    /// Default incoming traffic policy (`"deny"`, `"allow"`, `"reject"`).
    #[serde(default)]
    pub default_policy: Option<String>,
    /// Ports or services to permit (e.g. `"22"`, `"80/tcp"`, `"443:8443/tcp"`).
    #[serde(default)]
    pub allow_ports: Vec<String>,
    /// Ports or services to block explicitly.
    #[serde(default)]
    pub deny_ports: Vec<String>,
}

/// HTTP/HTTPS proxy and no-proxy settings written to `/etc/environment` and apt config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyConfig {
    /// URL of the HTTP proxy (e.g. `"http://proxy.corp:3128"`).
    #[serde(default)]
    pub http_proxy: Option<String>,
    /// URL of the HTTPS proxy. Often the same as `http_proxy`.
    #[serde(default)]
    pub https_proxy: Option<String>,
    /// Comma-separated list of hosts that bypass the proxy (e.g. `["localhost", "192.168.0.0/16"]`).
    #[serde(default)]
    pub no_proxy: Vec<String>,
}

/// Swap file parameters written to the target system via late-commands.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SwapConfig {
    /// Size of the swap file in mebibytes (MiB).
    pub size_mb: u32,
    /// Path of the swap file inside the installed system. Defaults to `/swapfile`.
    #[serde(default)]
    pub filename: Option<String>,
    /// Value written to `vm.swappiness` in `/etc/sysctl.d/99-swap.conf`. Omitted if `None`.
    #[serde(default)]
    pub swappiness: Option<u8>,
}

/// Container runtime selection: Docker CE and/or Podman, plus optional Docker group membership.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerConfig {
    /// Install Docker CE from the upstream Docker Inc. repository during installation.
    #[serde(default)]
    pub docker: bool,
    /// Install Podman from the distribution's standard package repository.
    #[serde(default)]
    pub podman: bool,
    /// User accounts added to the `docker` group so they can run containers without sudo.
    #[serde(default)]
    pub docker_users: Vec<String>,
}

/// GRUB bootloader tweaks applied to the installed system via `sed` and `update-grub`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GrubConfig {
    /// Seconds GRUB waits before booting the default entry. Replaces `GRUB_TIMEOUT` in `/etc/default/grub`.
    #[serde(default)]
    pub timeout: Option<u32>,
    /// Additional kernel parameters appended to `GRUB_CMDLINE_LINUX_DEFAULT`.
    #[serde(default)]
    pub cmdline_extra: Vec<String>,
    /// Override the `GRUB_DEFAULT` entry (e.g. `"0"` for the first entry, or a saved-entry token).
    #[serde(default)]
    pub default_entry: Option<String>,
}
