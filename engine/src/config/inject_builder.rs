use std::path::PathBuf;

use crate::error::EngineResult;

use super::components::{
    ContainerConfig, FirewallConfig, GrubConfig, NetworkConfig, ProxyConfig, SshConfig, SwapConfig,
    UserConfig,
};
use super::{Distro, InjectConfig, IsoSource};

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

    // -- Scalar setters --

    #[must_use]
    pub fn autoinstall_yaml(mut self, val: impl Into<PathBuf>) -> Self {
        self.autoinstall_yaml = Some(val.into());
        self
    }

    #[must_use]
    pub fn output_label(mut self, val: impl Into<String>) -> Self {
        self.output_label = Some(val.into());
        self
    }

    #[must_use]
    pub fn expected_sha256(mut self, val: impl Into<String>) -> Self {
        self.expected_sha256 = Some(val.into());
        self
    }

    #[must_use]
    pub fn hostname(mut self, val: impl Into<String>) -> Self {
        self.hostname = Some(val.into());
        self
    }

    #[must_use]
    pub fn username(mut self, val: impl Into<String>) -> Self {
        self.username = Some(val.into());
        self
    }

    #[must_use]
    pub fn password(mut self, val: impl Into<String>) -> Self {
        self.password = Some(val.into());
        self
    }

    #[must_use]
    pub fn realname(mut self, val: impl Into<String>) -> Self {
        self.realname = Some(val.into());
        self
    }

    #[must_use]
    pub fn timezone(mut self, val: impl Into<String>) -> Self {
        self.timezone = Some(val.into());
        self
    }

    #[must_use]
    pub fn locale(mut self, val: impl Into<String>) -> Self {
        self.locale = Some(val.into());
        self
    }

    #[must_use]
    pub fn keyboard_layout(mut self, val: impl Into<String>) -> Self {
        self.keyboard_layout = Some(val.into());
        self
    }

    #[must_use]
    pub fn storage_layout(mut self, val: impl Into<String>) -> Self {
        self.storage_layout = Some(val.into());
        self
    }

    #[must_use]
    pub fn apt_mirror(mut self, val: impl Into<String>) -> Self {
        self.apt_mirror = Some(val.into());
        self
    }

    #[must_use]
    pub fn extra_packages(mut self, val: Vec<String>) -> Self {
        self.extra_packages = Some(val);
        self
    }

    #[must_use]
    pub fn wallpaper(mut self, val: impl Into<PathBuf>) -> Self {
        self.wallpaper = Some(val.into());
        self
    }

    #[must_use]
    pub fn extra_late_commands(mut self, val: Vec<String>) -> Self {
        self.extra_late_commands = Some(val);
        self
    }

    #[must_use]
    pub fn no_user_interaction(mut self, val: bool) -> Self {
        self.no_user_interaction = Some(val);
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
    pub fn enable_services(mut self, val: Vec<String>) -> Self {
        self.enable_services = Some(val);
        self
    }

    #[must_use]
    pub fn disable_services(mut self, val: Vec<String>) -> Self {
        self.disable_services = Some(val);
        self
    }

    #[must_use]
    pub fn sysctl(mut self, val: Vec<(String, String)>) -> Self {
        self.sysctl = Some(val);
        self
    }

    #[must_use]
    pub fn apt_repos(mut self, val: Vec<String>) -> Self {
        self.apt_repos = Some(val);
        self
    }

    #[must_use]
    pub fn dnf_repos(mut self, val: Vec<String>) -> Self {
        self.dnf_repos = Some(val);
        self
    }

    #[must_use]
    pub fn dnf_mirror(mut self, val: impl Into<String>) -> Self {
        self.dnf_mirror = Some(val.into());
        self
    }

    #[must_use]
    pub fn pacman_repos(mut self, val: Vec<String>) -> Self {
        self.pacman_repos = Some(val);
        self
    }

    #[must_use]
    pub fn pacman_mirror(mut self, val: impl Into<String>) -> Self {
        self.pacman_mirror = Some(val.into());
        self
    }

    #[must_use]
    pub fn encrypt(mut self, val: bool) -> Self {
        self.encrypt = Some(val);
        self
    }

    #[must_use]
    pub fn encrypt_passphrase(mut self, val: impl Into<String>) -> Self {
        self.encrypt_passphrase = Some(val.into());
        self
    }

    #[must_use]
    pub fn mounts(mut self, val: Vec<String>) -> Self {
        self.mounts = Some(val);
        self
    }

    #[must_use]
    pub fn run_commands(mut self, val: Vec<String>) -> Self {
        self.run_commands = Some(val);
        self
    }

    #[must_use]
    pub fn distro(mut self, val: Distro) -> Self {
        self.distro = Some(val);
        self
    }

    // -- Sub-config setters --

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
    pub fn user(mut self, val: UserConfig) -> Self {
        self.user = Some(val);
        self
    }

    #[must_use]
    pub fn firewall(mut self, val: FirewallConfig) -> Self {
        self.firewall = Some(val);
        self
    }

    #[must_use]
    pub fn proxy(mut self, val: ProxyConfig) -> Self {
        self.proxy = Some(val);
        self
    }

    #[must_use]
    pub fn swap(mut self, val: SwapConfig) -> Self {
        self.swap = Some(val);
        self
    }

    #[must_use]
    pub fn containers(mut self, val: ContainerConfig) -> Self {
        self.containers = Some(val);
        self
    }

    #[must_use]
    pub fn grub(mut self, val: GrubConfig) -> Self {
        self.grub = Some(val);
        self
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SshConfig;

    #[test]
    fn builder_minimal_valid() {
        let cfg = InjectConfigBuilder::new(IsoSource::from_raw("/tmp/ubuntu.iso"), "my-custom.iso")
            .hostname("web-server")
            .username("admin")
            .build()
            .expect("minimal builder config must pass validation");

        assert_eq!(cfg.hostname.as_deref(), Some("web-server"));
        assert_eq!(cfg.username.as_deref(), Some("admin"));
        assert_eq!(cfg.out_name, "my-custom.iso");
        assert!(matches!(cfg.source, IsoSource::Path(_)));
    }

    #[test]
    fn builder_with_ssh() {
        let ssh = SshConfig {
            authorized_keys: vec![
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIG9vbWV0aGluZw== user@host".to_string(),
            ],
            allow_password_auth: Some(false),
            install_server: Some(true),
        };
        let cfg = InjectConfigBuilder::new(IsoSource::from_raw("/tmp/ubuntu.iso"), "ssh-test.iso")
            .hostname("ssh-host")
            .username("admin")
            .ssh(ssh)
            .build()
            .expect("builder with SSH config must pass validation");

        assert_eq!(cfg.ssh.authorized_keys.len(), 1);
        assert_eq!(cfg.ssh.allow_password_auth, Some(false));
        assert_eq!(cfg.ssh.install_server, Some(true));
    }

    #[test]
    fn builder_validation_fails_on_bad_hostname() {
        let result =
            InjectConfigBuilder::new(IsoSource::from_raw("/tmp/ubuntu.iso"), "bad-host.iso")
                .hostname("bad;host")
                .build();

        assert!(
            result.is_err(),
            "hostname with ';' must fail builder validation"
        );
    }
}
