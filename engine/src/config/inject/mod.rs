mod validate;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::EngineResult;

use super::components::{
    ContainerConfig, FirewallConfig, GrubConfig, NetworkConfig, ProxyConfig, SshConfig, SwapConfig,
    UserConfig,
};
use super::{Distro, IsoSource};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InjectConfig {
    pub source: IsoSource,
    /// Optional: if None, YAML is generated from fields below
    #[serde(default)]
    pub autoinstall_yaml: Option<PathBuf>,
    pub out_name: String,
    #[serde(default)]
    pub output_label: Option<String>,
    /// If set, the downloaded ISO's SHA-256 must match before injection proceeds.
    #[serde(default)]
    pub expected_sha256: Option<String>,

    // Identity
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    /// Plaintext; hashed to $6$ format before writing
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub realname: Option<String>,

    // SSH
    #[serde(default)]
    pub ssh: SshConfig,

    // Network
    #[serde(default)]
    pub network: NetworkConfig,

    // System
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub keyboard_layout: Option<String>,

    // Storage/Apt
    #[serde(default)]
    pub storage_layout: Option<String>, // "lvm" | "direct" | "zfs"
    #[serde(default)]
    pub apt_mirror: Option<String>,

    // Packages
    #[serde(default)]
    pub extra_packages: Vec<String>,

    // Wallpaper
    #[serde(default)]
    pub wallpaper: Option<PathBuf>,

    // Escape hatches
    #[serde(default)]
    pub extra_late_commands: Vec<String>,
    #[serde(default)]
    pub no_user_interaction: bool,

    // User / access management
    #[serde(default)]
    pub user: UserConfig,

    // Firewall
    #[serde(default)]
    pub firewall: FirewallConfig,

    // Network extras
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub static_ip: Option<String>,
    #[serde(default)]
    pub gateway: Option<String>,

    // Services
    #[serde(default)]
    pub enable_services: Vec<String>,
    #[serde(default)]
    pub disable_services: Vec<String>,

    // Kernel
    #[serde(default)]
    pub sysctl: Vec<(String, String)>,

    // Swap
    #[serde(default)]
    pub swap: Option<SwapConfig>,

    // APT repositories (Ubuntu/Debian)
    #[serde(default)]
    pub apt_repos: Vec<String>,

    // DNF repositories (Fedora/RHEL) -- each entry is a full `[id]\nbaseurl=...` stanza
    // or a shorthand URL string that gets wrapped into a minimal stanza.
    #[serde(default)]
    pub dnf_repos: Vec<String>,

    // Optional override for the primary DNF mirror base URL.
    #[serde(default)]
    pub dnf_mirror: Option<String>,

    // Pacman repositories (Arch Linux) -- each entry is a `Server = https://...` mirror line.
    #[serde(default)]
    pub pacman_repos: Vec<String>,

    // Optional primary pacman mirror URL (written as the first Server= line in mirrorlist).
    #[serde(default)]
    pub pacman_mirror: Option<String>,

    // Container runtimes
    #[serde(default)]
    pub containers: ContainerConfig,

    // GRUB
    #[serde(default)]
    pub grub: GrubConfig,

    // LUKS encryption
    #[serde(default)]
    pub encrypt: bool,
    #[serde(default)]
    pub encrypt_passphrase: Option<String>,

    // Custom fstab entries
    #[serde(default)]
    pub mounts: Vec<String>,

    // Cloud-init runcmd equivalent
    #[serde(default)]
    pub run_commands: Vec<String>,

    // Target distro -- None means Ubuntu (default, existing behaviour unchanged)
    #[serde(default)]
    pub distro: Option<Distro>,
}

impl InjectConfig {
    /// Validate structured fields to prevent shell injection in late-commands.
    /// Fields like `run_commands` and `extra_late_commands` are intentional
    /// escape hatches and are NOT validated here.
    ///
    /// # Errors
    /// Returns [`crate::error::EngineError::InvalidConfig`] if any field contains
    /// shell-unsafe characters.
    pub fn validate(&self) -> EngineResult<()> {
        validate::run(self)
    }
}

#[cfg(test)]
mod tests;
