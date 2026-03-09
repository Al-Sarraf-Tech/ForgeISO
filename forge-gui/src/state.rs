use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Stage ordering ─────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Stage {
    Inject,
    Verify,
    Diff,
    Build,
    Completion,
}

#[allow(dead_code)]
impl Stage {
    pub const ALL: &'static [Stage] = &[
        Stage::Inject,
        Stage::Verify,
        Stage::Diff,
        Stage::Build,
        Stage::Completion,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Stage::Inject => "Inject",
            Stage::Verify => "Verify",
            Stage::Diff => "Diff",
            Stage::Build => "Build",
            Stage::Completion => "Complete",
        }
    }

    pub fn sublabel(&self) -> &'static str {
        match self {
            Stage::Inject => "Autoinstall config",
            Stage::Verify => "SHA-256 integrity",
            Stage::Diff => "Compare ISOs",
            Stage::Build => "Fetch & package",
            Stage::Completion => "Artifacts ready",
        }
    }

    pub fn step_num(&self) -> usize {
        match self {
            Stage::Inject => 1,
            Stage::Verify => 2,
            Stage::Diff => 3,
            Stage::Build => 4,
            Stage::Completion => 5,
        }
    }
}

// ── File picker target ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickTarget {
    InjectSource,
    InjectOutputDir,
    InjectWallpaper,
    VerifySource,
    DiffBase,
    DiffTarget,
    BuildSource,
    BuildOutputDir,
    BuildOverlay,
}

// ── Inject form state ──────────────────────────────────────────────────────────

/// All fields carry `#[serde(default)]` so that saved state from an older
/// version (missing newly-added fields) deserializes without error rather
/// than silently resetting the entire form.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct InjectState {
    // Required
    pub source: String,
    pub source_preset: String, // preset ID (e.g. "ubuntu-server-lts"), "" = none
    pub output_dir: String,
    pub out_name: String,
    pub output_label: String,
    pub distro: String,
    // Identity
    pub hostname: String,
    pub username: String,
    #[serde(skip)] // never persist passwords to disk
    pub password: String,
    #[serde(skip)]
    pub password_confirm: String,
    pub realname: String,
    // SSH
    pub ssh_keys: String, // newline-separated
    pub ssh_password_auth: bool,
    pub ssh_install_server: bool,
    // Network
    pub dns_servers: String,
    pub ntp_servers: String,
    pub static_ip: String,
    pub gateway: String,
    pub http_proxy: String,
    pub https_proxy: String,
    pub no_proxy: String, // comma-separated
    // System
    pub timezone: String,
    pub locale: String,
    pub keyboard_layout: String,
    pub storage_layout: String,
    pub apt_mirror: String,
    // Packages
    pub packages: String,
    pub apt_repos: String,  // Ubuntu/Debian: ppa: lines or deb http://... lines
    pub dnf_repos: String,  // Fedora/RHEL: repo stanza or URL, one per line
    pub dnf_mirror: String, // primary DNF mirror override
    pub pacman_repos: String, // Arch: Server = https://... lines
    pub pacman_mirror: String, // primary pacman mirror override
    // Commands
    pub run_commands: String,
    pub late_commands: String,
    // Firewall
    pub firewall_enabled: bool,
    pub firewall_policy: String,
    pub allow_ports: String,
    pub deny_ports: String,
    // User / access
    pub user_groups: String,   // newline-separated
    pub user_shell: String,    // e.g. /bin/bash, /usr/bin/zsh
    pub sudo_nopasswd: bool,   // grant NOPASSWD:ALL sudoers
    pub sudo_commands: String, // newline-separated specific commands for sudoers
    // Services
    pub enable_services: String,  // newline-separated
    pub disable_services: String, // newline-separated
    // Containers
    pub docker: bool,
    pub podman: bool,
    pub docker_users: String, // newline-separated users to add to docker group
    // Swap
    pub swap_size_mb: String,
    pub swap_filename: String,   // path inside target, default /swapfile
    pub swap_swappiness: String, // 0–100, blank = system default
    // Encryption (LUKS)
    pub encrypt: bool,
    #[serde(skip)] // never persist passphrase to disk
    pub encrypt_passphrase: String,
    // Custom mounts (fstab entries, one per line)
    pub mounts: String,
    // GRUB
    pub grub_timeout: String, // seconds as string, empty = engine default
    pub grub_cmdline: String, // extra kernel command-line args
    pub grub_default: String, // default boot entry label
    // Sysctl
    pub sysctl_pairs: String, // newline-separated "key=value" entries
    // Misc
    pub no_user_interaction: bool,
    pub wallpaper_path: String,
    // Verification
    pub expected_sha256: String, // hex SHA-256; empty = skip check
}

impl Default for InjectState {
    fn default() -> Self {
        let cache = dirs_cache();
        Self {
            source: String::new(),
            source_preset: String::new(),
            output_dir: cache,
            out_name: "forgeiso-local.iso".into(),
            output_label: String::new(),
            distro: "ubuntu".into(),
            hostname: String::new(),
            username: String::new(),
            password: String::new(),
            password_confirm: String::new(),
            realname: String::new(),
            ssh_keys: String::new(),
            ssh_password_auth: false,
            ssh_install_server: true,
            dns_servers: String::new(),
            ntp_servers: String::new(),
            static_ip: String::new(),
            gateway: String::new(),
            http_proxy: String::new(),
            https_proxy: String::new(),
            no_proxy: String::new(),
            timezone: String::new(),
            locale: String::new(),
            keyboard_layout: String::new(),
            storage_layout: String::new(),
            apt_mirror: String::new(),
            packages: String::new(),
            apt_repos: String::new(),
            dnf_repos: String::new(),
            dnf_mirror: String::new(),
            pacman_repos: String::new(),
            pacman_mirror: String::new(),
            run_commands: String::new(),
            late_commands: String::new(),
            firewall_enabled: false,
            firewall_policy: String::new(),
            allow_ports: String::new(),
            deny_ports: String::new(),
            user_groups: String::new(),
            user_shell: String::new(),
            sudo_nopasswd: false,
            sudo_commands: String::new(),
            enable_services: String::new(),
            disable_services: String::new(),
            docker: false,
            podman: false,
            docker_users: String::new(),
            swap_size_mb: String::new(),
            swap_filename: String::new(),
            swap_swappiness: String::new(),
            encrypt: false,
            encrypt_passphrase: String::new(),
            mounts: String::new(),
            grub_timeout: String::new(),
            grub_cmdline: String::new(),
            grub_default: String::new(),
            sysctl_pairs: String::new(),
            no_user_interaction: true,
            wallpaper_path: String::new(),
            expected_sha256: String::new(),
        }
    }
}

// ── Verify form state ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VerifyState {
    pub source: String,
    pub sums_url: String,
}

// ── Diff form state ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiffState {
    pub base: String,
    pub target: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DiffFilter {
    #[default]
    All,
    Added,
    Removed,
    Modified,
}

// ── Build form state ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildState {
    pub source: String,
    pub source_preset: String, // preset ID, "" = none
    pub output_dir: String,
    pub build_name: String,
    pub overlay_dir: String,
    pub output_label: String,
    pub profile: String,
    pub expected_sha256: String, // hex SHA-256; empty = skip check
}

impl Default for BuildState {
    fn default() -> Self {
        Self {
            source: String::new(),
            source_preset: String::new(),
            output_dir: "./artifacts".into(),
            build_name: "forgeiso-local".into(),
            overlay_dir: String::new(),
            output_label: String::new(),
            profile: "minimal".into(),
            expected_sha256: String::new(),
        }
    }
}

// ── Log entry ──────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub phase: String,
    pub message: String,
    pub level: LogLevel,
    pub timestamp: String,
    pub percent: Option<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

// ── Status message ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct StatusMsg {
    pub text: String,
    pub is_error: bool,
}

impl StatusMsg {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
        }
    }
    pub fn err(text: impl Into<String>) -> Self {
        // Truncate long error messages so they don't overflow the status bar.
        // Keep the first line (most errors are one line); if that exceeds 200
        // chars, truncate with an ellipsis so the UI remains readable.
        const MAX_CHARS: usize = 200;
        let raw: String = text.into();
        let first_line = raw.lines().next().unwrap_or(&raw);
        let text = if first_line.len() > MAX_CHARS {
            // Find the last char boundary at or before MAX_CHARS bytes so we
            // don't panic when slicing mid-way through a multi-byte character.
            let boundary = first_line
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= MAX_CHARS)
                .last()
                .unwrap_or(0);
            format!("{}…", &first_line[..boundary])
        } else {
            first_line.to_string()
        };
        Self {
            text,
            is_error: true,
        }
    }
}

// ── Engine type aliases ────────────────────────────────────────────────────────

pub type BuildResult = forgeiso_engine::BuildResult;
pub type DoctorReport = forgeiso_engine::DoctorReport;
pub type VerifyResult = forgeiso_engine::VerifyResult;
pub type Iso9660Compliance = forgeiso_engine::Iso9660Compliance;
pub type IsoDiff = forgeiso_engine::IsoDiff;
pub type IsoMetadata = forgeiso_engine::IsoMetadata;

// ── Helpers ────────────────────────────────────────────────────────────────────

fn dirs_cache() -> String {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".cache").join("forgeiso"))
        .unwrap_or_else(|_| PathBuf::from("/tmp/forgeiso"))
        .to_string_lossy()
        .into_owned()
}

/// Split a newline-separated textarea into non-empty trimmed strings.
pub fn lines(s: &str) -> Vec<String> {
    s.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Treat empty/whitespace-only string as None.
pub fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}
