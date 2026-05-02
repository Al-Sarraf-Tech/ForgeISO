mod validate;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::EngineResult;

use super::components::{
    ContainerConfig, FirewallConfig, GrubConfig, NetworkConfig, ProxyConfig, SshConfig, SwapConfig,
    UserConfig,
};
use super::{Distro, IsoSource};

/// Complete description of what to inject into a source ISO during the repack phase.
///
/// Every public field is serializable so configs can round-trip through YAML/JSON.
/// Build this struct via [`InjectConfigBuilder`](super::InjectConfigBuilder) for
/// compile-time field checking and automatic [`InjectConfig::validate`] on `build()`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InjectConfig {
    /// Source ISO location — a local filesystem path or an HTTP(S) URL.
    pub source: IsoSource,
    /// Optional: if None, autoinstall YAML is generated from the fields below.
    #[serde(default)]
    pub autoinstall_yaml: Option<PathBuf>,
    /// Filename of the output ISO (e.g. `"my-custom-ubuntu.iso"`).
    pub out_name: String,
    /// Volume label written to the output ISO. Must be ≤32 ASCII characters with no control chars.
    #[serde(default)]
    pub output_label: Option<String>,
    /// If set, the downloaded ISO's SHA-256 must match before injection proceeds.
    #[serde(default)]
    pub expected_sha256: Option<String>,

    // Identity
    /// Network hostname injected into the autoinstall configuration, used as `/etc/hostname`.
    #[serde(default)]
    pub hostname: Option<String>,
    /// Primary user account name created during unattended installation.
    #[serde(default)]
    pub username: Option<String>,
    /// Plaintext password hashed to `$6$` SHA-512-crypt format before writing to the installer config.
    #[serde(default)]
    pub password: Option<String>,
    /// Full real name (GECOS field) for the primary user account.
    #[serde(default)]
    pub realname: Option<String>,

    // SSH
    /// SSH server and authorized-key settings applied to the installed system.
    #[serde(default)]
    pub ssh: SshConfig,

    // Network
    /// DNS and NTP server settings applied to the installed system.
    #[serde(default)]
    pub network: NetworkConfig,

    // System
    /// IANA timezone identifier (e.g. `"America/New_York"`). Defaults to `"UTC"`.
    #[serde(default)]
    pub timezone: Option<String>,
    /// Locale string injected into the installer (e.g. `"en_US.UTF-8"`). Defaults to `"en_US.UTF-8"`.
    #[serde(default)]
    pub locale: Option<String>,
    /// XKB keyboard layout code (e.g. `"us"`, `"gb"`, `"de"`). Defaults to `"us"`.
    #[serde(default)]
    pub keyboard_layout: Option<String>,

    // Storage/Apt
    /// Disk partitioning scheme: `"lvm"` (default), `"direct"`, or `"zfs"`.
    #[serde(default)]
    pub storage_layout: Option<String>,
    /// Override the APT mirror used during installation (HTTP or HTTPS URL).
    #[serde(default)]
    pub apt_mirror: Option<String>,

    // Packages
    /// Additional packages installed via the distro's package manager during setup.
    #[serde(default)]
    pub extra_packages: Vec<String>,

    // Wallpaper
    /// Path to an image file copied to the ISO and set as the default GNOME desktop wallpaper.
    #[serde(default)]
    pub wallpaper: Option<PathBuf>,

    // Escape hatches
    /// Arbitrary shell commands appended verbatim after all engine-generated late-commands.
    #[serde(default)]
    pub extra_late_commands: Vec<String>,
    /// When `true`, suppress all interactive installer prompts for a fully unattended install.
    #[serde(default)]
    pub no_user_interaction: bool,

    // User / access management
    /// Extended user-account settings: groups, shell, sudo rules.
    #[serde(default)]
    pub user: UserConfig,

    // Firewall
    /// Firewall configuration (UFW on Ubuntu/Mint, firewalld on Fedora).
    #[serde(default)]
    pub firewall: FirewallConfig,

    // Network extras
    /// HTTP/HTTPS proxy settings written to `/etc/environment` and apt config.
    #[serde(default)]
    pub proxy: ProxyConfig,
    /// Static IPv4 address in CIDR notation (e.g. `"192.168.1.10/24"`).
    #[serde(default)]
    pub static_ip: Option<String>,
    /// Default gateway IP address used when `static_ip` is set.
    #[serde(default)]
    pub gateway: Option<String>,

    // Services
    /// Systemd unit names enabled with `systemctl enable` during installation.
    #[serde(default)]
    pub enable_services: Vec<String>,
    /// Systemd unit names disabled with `systemctl disable` during installation.
    #[serde(default)]
    pub disable_services: Vec<String>,

    // Kernel
    /// Key-value sysctl tunables written to `/etc/sysctl.d/99-forgeiso.conf`.
    #[serde(default)]
    pub sysctl: Vec<(String, String)>,

    // Swap
    /// Swap file parameters; `None` means no swap file is created.
    #[serde(default)]
    pub swap: Option<SwapConfig>,

    // APT repositories (Ubuntu/Debian)
    /// Extra APT repository entries: PPA shorthand (`ppa:user/repo`) or full `deb …` lines.
    #[serde(default)]
    pub apt_repos: Vec<String>,

    /// DNF repository entries for Fedora/RHEL: a full `[id]\nbaseurl=...` stanza
    /// or a shorthand URL string that gets wrapped into a minimal stanza.
    #[serde(default)]
    pub dnf_repos: Vec<String>,

    /// Override the primary DNF mirror base URL written to `fedora.repo` and `fedora-updates.repo`.
    #[serde(default)]
    pub dnf_mirror: Option<String>,

    /// Pacman repository mirror lines for Arch Linux; each entry is a `Server = https://...` line.
    #[serde(default)]
    pub pacman_repos: Vec<String>,

    /// Primary Pacman mirror URL written as the first `Server=` line in `/etc/pacman.d/mirrorlist`.
    #[serde(default)]
    pub pacman_mirror: Option<String>,

    // Container runtimes
    /// Container runtime installation: Docker CE and/or Podman.
    #[serde(default)]
    pub containers: ContainerConfig,

    // GRUB
    /// GRUB bootloader tweaks applied via `sed` and `update-grub` / `grub2-mkconfig`.
    #[serde(default)]
    pub grub: GrubConfig,

    // LUKS encryption
    /// Enable LUKS full-disk encryption for the storage layout (requires `encrypt_passphrase`).
    #[serde(default)]
    pub encrypt: bool,
    /// LUKS passphrase written in plaintext to the autoinstall storage layout. Treat resulting ISOs as sensitive.
    #[serde(default)]
    pub encrypt_passphrase: Option<String>,

    // Custom fstab entries
    /// Additional `/etc/fstab` lines appended verbatim; each entry creates `mkdir -p <mountpoint>` before writing.
    #[serde(default)]
    pub mounts: Vec<String>,

    // Cloud-init runcmd equivalent
    /// Shell commands run via `runcmd` (cloud-init) or `%post` (Kickstart); pass through verbatim unchanged.
    #[serde(default)]
    pub run_commands: Vec<String>,

    /// Target distro family that selects the installer format. `None` defaults to Ubuntu (existing behaviour unchanged).
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
