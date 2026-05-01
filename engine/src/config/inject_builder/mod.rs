//! Typed fluent builder for [`InjectConfig`].
//!
//! The struct definition, constructor, and final `build()` (which runs
//! [`InjectConfig::validate`]) live here. Setter methods are split across
//! per-concern submodules — each adds its own `impl InjectConfigBuilder`
//! block — so this module stays focused on construction and validation:
//!
//! - [`identity`]   — autoinstall, output labels/checksums, hostname/user.
//! - [`system`]     — locale/timezone/keyboard, distro, wallpaper, interactivity.
//! - [`network`]    — SSH, network, proxy, IP/gateway, firewall.
//! - [`storage`]    — storage layout, LUKS, mounts, swap.
//! - [`packages`]   — APT/DNF/Pacman, extra packages, container runtime.
//! - [`services`]   — services, sysctl, GRUB, run/late commands, user access.

use std::path::PathBuf;

use crate::error::EngineResult;

use super::components::{
    ContainerConfig, FirewallConfig, GrubConfig, NetworkConfig, ProxyConfig, SshConfig, SwapConfig,
    UserConfig,
};
use super::{Distro, InjectConfig, IsoSource};

mod identity;
mod network;
mod packages;
mod services;
mod storage;
mod system;

#[cfg(test)]
mod tests;

/// Builder for [`InjectConfig`] -- provides a fluent API for constructing
/// injection configurations with validation on `build()`.
///
/// `source` and `out_name` are required and supplied at construction time.
/// All other fields default to their `InjectConfig::default()` values and
/// can be overridden via chained setter methods.
///
/// # Example
///
/// ```
/// # use forgeiso_engine::config::{InjectConfigBuilder, IsoSource};
/// let cfg = InjectConfigBuilder::new(
///         IsoSource::from_raw("/tmp/ubuntu.iso"),
///         "my-custom.iso",
///     )
///     .hostname("web-server")
///     .username("admin")
///     .build()
///     .expect("validation failed");
///
/// assert_eq!(cfg.hostname.as_deref(), Some("web-server"));
/// ```
pub struct InjectConfigBuilder {
    source: IsoSource,
    out_name: String,

    // Identity
    autoinstall_yaml: Option<PathBuf>,
    output_label: Option<String>,
    expected_sha256: Option<String>,
    hostname: Option<String>,
    username: Option<String>,
    password: Option<String>,
    realname: Option<String>,

    // SSH
    ssh: Option<SshConfig>,

    // Network
    network: Option<NetworkConfig>,

    // System
    timezone: Option<String>,
    locale: Option<String>,
    keyboard_layout: Option<String>,

    // Storage/Apt
    storage_layout: Option<String>,
    apt_mirror: Option<String>,

    // Packages
    extra_packages: Option<Vec<String>>,

    // Wallpaper
    wallpaper: Option<PathBuf>,

    // Escape hatches
    extra_late_commands: Option<Vec<String>>,
    no_user_interaction: Option<bool>,

    // User / access
    user: Option<UserConfig>,

    // Firewall
    firewall: Option<FirewallConfig>,

    // Network extras
    proxy: Option<ProxyConfig>,
    static_ip: Option<String>,
    gateway: Option<String>,

    // Services
    enable_services: Option<Vec<String>>,
    disable_services: Option<Vec<String>>,

    // Kernel
    sysctl: Option<Vec<(String, String)>>,

    // Swap
    swap: Option<SwapConfig>,

    // APT repositories
    apt_repos: Option<Vec<String>>,

    // DNF
    dnf_repos: Option<Vec<String>>,
    dnf_mirror: Option<String>,

    // Pacman
    pacman_repos: Option<Vec<String>>,
    pacman_mirror: Option<String>,

    // Containers
    containers: Option<ContainerConfig>,

    // GRUB
    grub: Option<GrubConfig>,

    // LUKS encryption
    encrypt: Option<bool>,
    encrypt_passphrase: Option<String>,

    // Custom fstab entries
    mounts: Option<Vec<String>>,

    // Cloud-init runcmd
    run_commands: Option<Vec<String>>,

    // Target distro
    distro: Option<Distro>,
}

impl InjectConfigBuilder {
    /// Create a new builder with the two required fields.
    #[must_use]
    pub fn new(source: IsoSource, out_name: impl Into<String>) -> Self {
        Self {
            source,
            out_name: out_name.into(),
            autoinstall_yaml: None,
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: None,
            network: None,
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: None,
            wallpaper: None,
            extra_late_commands: None,
            no_user_interaction: None,
            user: None,
            firewall: None,
            proxy: None,
            static_ip: None,
            gateway: None,
            enable_services: None,
            disable_services: None,
            sysctl: None,
            swap: None,
            apt_repos: None,
            dnf_repos: None,
            dnf_mirror: None,
            pacman_repos: None,
            pacman_mirror: None,
            containers: None,
            grub: None,
            encrypt: None,
            encrypt_passphrase: None,
            mounts: None,
            run_commands: None,
            distro: None,
        }
    }

    /// Consume the builder and produce an [`InjectConfig`], running
    /// [`InjectConfig::validate`] before returning.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::InvalidConfig`] if any field fails validation.
    pub fn build(self) -> EngineResult<InjectConfig> {
        let cfg = InjectConfig {
            source: self.source,
            out_name: self.out_name,
            autoinstall_yaml: self.autoinstall_yaml,
            output_label: self.output_label,
            expected_sha256: self.expected_sha256,
            hostname: self.hostname,
            username: self.username,
            password: self.password,
            realname: self.realname,
            ssh: self.ssh.unwrap_or_default(),
            network: self.network.unwrap_or_default(),
            timezone: self.timezone,
            locale: self.locale,
            keyboard_layout: self.keyboard_layout,
            storage_layout: self.storage_layout,
            apt_mirror: self.apt_mirror,
            extra_packages: self.extra_packages.unwrap_or_default(),
            wallpaper: self.wallpaper,
            extra_late_commands: self.extra_late_commands.unwrap_or_default(),
            no_user_interaction: self.no_user_interaction.unwrap_or_default(),
            user: self.user.unwrap_or_default(),
            firewall: self.firewall.unwrap_or_default(),
            proxy: self.proxy.unwrap_or_default(),
            static_ip: self.static_ip,
            gateway: self.gateway,
            enable_services: self.enable_services.unwrap_or_default(),
            disable_services: self.disable_services.unwrap_or_default(),
            sysctl: self.sysctl.unwrap_or_default(),
            swap: self.swap,
            apt_repos: self.apt_repos.unwrap_or_default(),
            dnf_repos: self.dnf_repos.unwrap_or_default(),
            dnf_mirror: self.dnf_mirror,
            pacman_repos: self.pacman_repos.unwrap_or_default(),
            pacman_mirror: self.pacman_mirror,
            containers: self.containers.unwrap_or_default(),
            grub: self.grub.unwrap_or_default(),
            encrypt: self.encrypt.unwrap_or_default(),
            encrypt_passphrase: self.encrypt_passphrase,
            mounts: self.mounts.unwrap_or_default(),
            run_commands: self.run_commands.unwrap_or_default(),
            distro: self.distro,
        };
        cfg.validate()?;
        Ok(cfg)
    }
}
