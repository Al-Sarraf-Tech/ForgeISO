use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Inject form state ─────────────────────────────────────────────────────────
// Mirrors forge-gui/src/state.rs — passwords carry #[serde(skip)] for security.

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct InjectState {
    pub source: String,
    pub source_preset: String,
    pub output_dir: String,
    pub out_name: String,
    pub output_label: String,
    pub distro: String,
    pub hostname: String,
    pub username: String,
    #[serde(skip)]
    pub password: String,
    #[serde(skip)]
    pub password_confirm: String,
    pub realname: String,
    pub ssh_keys: String,
    pub ssh_password_auth: bool,
    pub ssh_install_server: bool,
    pub dns_servers: String,
    pub ntp_servers: String,
    pub static_ip: String,
    pub gateway: String,
    pub http_proxy: String,
    pub https_proxy: String,
    pub no_proxy: String,
    pub timezone: String,
    pub locale: String,
    pub keyboard_layout: String,
    pub storage_layout: String,
    pub apt_mirror: String,
    pub packages: String,
    pub apt_repos: String,
    pub dnf_repos: String,
    pub dnf_mirror: String,
    pub pacman_repos: String,
    pub pacman_mirror: String,
    pub run_commands: String,
    pub late_commands: String,
    pub firewall_enabled: bool,
    pub firewall_policy: String,
    pub allow_ports: String,
    pub deny_ports: String,
    pub user_groups: String,
    pub user_shell: String,
    pub sudo_nopasswd: bool,
    pub sudo_commands: String,
    pub enable_services: String,
    pub disable_services: String,
    pub docker: bool,
    pub podman: bool,
    pub docker_users: String,
    pub swap_size_mb: String,
    pub swap_filename: String,
    pub swap_swappiness: String,
    pub encrypt: bool,
    #[serde(skip)]
    pub encrypt_passphrase: String,
    pub mounts: String,
    pub grub_timeout: String,
    pub grub_cmdline: String,
    pub grub_default: String,
    pub sysctl_pairs: String,
    pub no_user_interaction: bool,
    pub wallpaper_path: String,
    pub expected_sha256: String,
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
            no_user_interaction: false,
            wallpaper_path: String::new(),
            expected_sha256: String::new(),
        }
    }
}

// ── Verify form state ─────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VerifyState {
    pub source: String,
    pub sums_url: String,
}

// ── Full persisted state ──────────────────────────────────────────────────────

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PersistedState {
    pub inject: InjectState,
    pub verify: VerifyState,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn dirs_cache() -> String {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".cache").join("forgeiso"))
        .unwrap_or_else(|_| PathBuf::from("/tmp/forgeiso"))
        .to_string_lossy()
        .into_owned()
}

/// Split a newline-separated field into non-empty trimmed strings.
pub fn lines(s: &str) -> Vec<String> {
    s.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Split a flexible token field into non-empty trimmed strings.
/// Accepts commas, whitespace, and newlines as separators.
pub fn tokens(s: &str) -> Vec<String> {
    s.split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
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

#[cfg(test)]
mod tests {
    use super::{lines, opt, tokens};

    #[test]
    fn lines_only_split_on_newlines() {
        assert_eq!(lines("a\nb\n\n c "), vec!["a", "b", "c"]);
    }

    #[test]
    fn tokens_split_on_commas_spaces_and_newlines() {
        assert_eq!(
            tokens("curl git,\nhtop\tvim"),
            vec!["curl", "git", "htop", "vim"]
        );
    }

    #[test]
    fn opt_trims_whitespace() {
        assert_eq!(opt("  value  "), Some("value".to_string()));
        assert_eq!(opt("   "), None);
    }
}
