//! Distro-aware smart defaults for the GUI wizard.
//!
//! Each distro family gets a conservative, practical set of defaults that
//! populate form fields when the user hasn't manually edited them. The
//! defaults cover packages, user groups, services, and basic settings.

use std::collections::HashSet;

/// A snapshot of defaults for a given distro/preset selection.
#[derive(Clone, Debug, Default)]
pub struct DistroDefaults {
    /// Space-separated package list (matches the GUI text field format).
    pub packages: String,
    /// Newline-separated supplemental groups.
    pub user_groups: String,
    /// Login shell path.
    pub user_shell: String,
    /// Newline-separated services to enable.
    pub enable_services: String,
    /// Newline-separated services to disable.
    pub disable_services: String,
    /// Default firewall policy when firewall is enabled.
    pub firewall_policy: String,
    /// Space-separated allowed ports when firewall is enabled.
    pub allow_ports: String,
    /// Whether to add the primary user to the docker group.
    pub docker_user_auto: bool,
}

/// Distro family for default resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistroFamily {
    Ubuntu,
    Mint,
    Fedora,
    Arch,
}

impl DistroFamily {
    /// Derive from the distro string stored in the GUI state.
    pub fn from_distro_str(s: &str) -> Option<Self> {
        match s {
            "ubuntu" => Some(Self::Ubuntu),
            "mint" => Some(Self::Mint),
            "fedora" => Some(Self::Fedora),
            "arch" => Some(Self::Arch),
            _ => None,
        }
    }
}

/// Whether a preset is server-oriented (gets more packages) vs desktop.
fn is_server_preset(preset_id: &str) -> bool {
    matches!(
        preset_id,
        "ubuntu-server-lts"
            | "fedora-server"
            | "rocky-linux"
            | "almalinux"
            | "centos-stream"
            | "rhel-custom"
    )
}

/// Compute defaults for a distro family and preset.
pub fn defaults_for(distro: &str, preset_id: &str) -> DistroDefaults {
    let family = DistroFamily::from_distro_str(distro);
    let server = is_server_preset(preset_id);

    match family {
        Some(DistroFamily::Ubuntu) => ubuntu_defaults(server),
        Some(DistroFamily::Mint) => mint_defaults(),
        Some(DistroFamily::Fedora) => fedora_defaults(server),
        Some(DistroFamily::Arch) => arch_defaults(server),
        None => DistroDefaults::default(),
    }
}

fn ubuntu_defaults(server: bool) -> DistroDefaults {
    DistroDefaults {
        packages: if server {
            "curl wget git vim htop rsync net-tools".into()
        } else {
            "curl git htop".into()
        },
        user_groups: "sudo".into(),
        user_shell: "/bin/bash".into(),
        enable_services: String::new(),
        disable_services: if server {
            "snapd".into()
        } else {
            String::new()
        },
        firewall_policy: "deny".into(),
        allow_ports: "22/tcp".into(),
        docker_user_auto: true,
    }
}

fn mint_defaults() -> DistroDefaults {
    DistroDefaults {
        packages: "curl git htop".into(),
        user_groups: "sudo".into(),
        user_shell: "/bin/bash".into(),
        enable_services: String::new(),
        disable_services: String::new(),
        firewall_policy: "deny".into(),
        allow_ports: "22/tcp".into(),
        docker_user_auto: true,
    }
}

fn fedora_defaults(server: bool) -> DistroDefaults {
    DistroDefaults {
        packages: if server {
            "curl wget git vim-enhanced htop rsync bind-utils".into()
        } else {
            "curl git htop".into()
        },
        user_groups: "wheel".into(),
        user_shell: "/bin/bash".into(),
        enable_services: String::new(),
        disable_services: String::new(),
        firewall_policy: "deny".into(),
        allow_ports: "22/tcp".into(),
        docker_user_auto: true,
    }
}

fn arch_defaults(server: bool) -> DistroDefaults {
    DistroDefaults {
        packages: if server {
            "curl wget git vim htop rsync openssh".into()
        } else {
            "curl git htop".into()
        },
        user_groups: "wheel".into(),
        user_shell: "/bin/bash".into(),
        enable_services: if server { "sshd".into() } else { String::new() },
        disable_services: String::new(),
        firewall_policy: "deny".into(),
        allow_ports: "22/tcp".into(),
        docker_user_auto: true,
    }
}

/// Which fields the defaults system can populate.
/// Used to track user edits so we don't clobber them.
#[allow(dead_code)]
pub const DEFAULT_FIELDS: &[&str] = &[
    "packages",
    "user_groups",
    "user_shell",
    "enable_services",
    "disable_services",
    "firewall_policy",
    "allow_ports",
    "docker_users",
];

/// Apply defaults to a mutable field map, skipping any field the user has edited.
/// Returns the set of fields that were actually changed.
pub fn apply_defaults(
    defaults: &DistroDefaults,
    edited: &HashSet<String>,
    username: &str,
    docker_enabled: bool,
) -> Vec<(&'static str, String)> {
    let mut changes = Vec::new();

    let fields: &[(&str, &str)] = &[
        ("packages", &defaults.packages),
        ("user_groups", &defaults.user_groups),
        ("user_shell", &defaults.user_shell),
        ("enable_services", &defaults.enable_services),
        ("disable_services", &defaults.disable_services),
        ("firewall_policy", &defaults.firewall_policy),
        ("allow_ports", &defaults.allow_ports),
    ];

    for (name, value) in fields {
        if !edited.contains(*name) {
            changes.push((*name, value.to_string()));
        }
    }

    // Auto-populate docker_users when Docker is enabled and field not edited.
    if docker_enabled && defaults.docker_user_auto && !edited.contains("docker_users") {
        let user = if username.is_empty() {
            String::new()
        } else {
            username.to_string()
        };
        changes.push(("docker_users", user));
    }

    changes
}

/// Build a human-readable summary of what defaults would be applied.
pub fn summary_for(defaults: &DistroDefaults) -> String {
    let mut parts = Vec::new();
    let pkg_count = defaults
        .packages
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .count();
    if pkg_count > 0 {
        parts.push(format!("{pkg_count} packages"));
    }
    let groups: Vec<&str> = defaults
        .user_groups
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if !groups.is_empty() {
        parts.push(format!("groups: {}", groups.join(", ")));
    }
    if !defaults.user_shell.is_empty() {
        parts.push(format!("shell: {}", defaults.user_shell));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ubuntu_server_gets_server_packages() {
        let d = defaults_for("ubuntu", "ubuntu-server-lts");
        assert!(d.packages.contains("rsync"));
        assert!(d.packages.contains("curl"));
        assert_eq!(d.user_groups, "sudo");
        assert_eq!(d.user_shell, "/bin/bash");
    }

    #[test]
    fn ubuntu_desktop_gets_lighter_packages() {
        let d = defaults_for("ubuntu", "ubuntu-desktop-lts");
        assert!(!d.packages.contains("rsync"));
        assert!(d.packages.contains("curl"));
    }

    #[test]
    fn fedora_uses_wheel_group() {
        let d = defaults_for("fedora", "fedora-server");
        assert_eq!(d.user_groups, "wheel");
        assert!(d.packages.contains("vim-enhanced"));
    }

    #[test]
    fn arch_uses_wheel_and_pacman_names() {
        let d = defaults_for("arch", "arch-linux");
        assert_eq!(d.user_groups, "wheel");
        // arch-linux is a desktop preset — openssh only in server defaults
        assert!(d.packages.contains("curl"));
        // Server preset gets openssh
        let ds = arch_defaults(true);
        assert!(ds.packages.contains("openssh"));
    }

    #[test]
    fn arch_server_enables_sshd() {
        // arch-linux is not in is_server_preset, so it's desktop
        let d = defaults_for("arch", "arch-linux");
        assert!(d.enable_services.is_empty());
        // but if it were a server context:
        let d2 = arch_defaults(true);
        assert_eq!(d2.enable_services, "sshd");
    }

    #[test]
    fn mint_uses_sudo_group() {
        let d = defaults_for("mint", "linux-mint-cinnamon");
        assert_eq!(d.user_groups, "sudo");
    }

    #[test]
    fn unknown_distro_returns_empty_defaults() {
        let d = defaults_for("debian", "");
        assert!(d.packages.is_empty());
        assert!(d.user_groups.is_empty());
    }

    #[test]
    fn apply_skips_edited_fields() {
        let d = defaults_for("ubuntu", "ubuntu-server-lts");
        let mut edited = HashSet::new();
        edited.insert("packages".to_string());
        let changes = apply_defaults(&d, &edited, "admin", false);
        // packages should be skipped
        assert!(changes.iter().all(|(name, _)| *name != "packages"));
        // user_groups should be present
        assert!(changes.iter().any(|(name, _)| *name == "user_groups"));
    }

    #[test]
    fn apply_adds_docker_user_when_enabled() {
        let d = defaults_for("ubuntu", "ubuntu-server-lts");
        let edited = HashSet::new();
        let changes = apply_defaults(&d, &edited, "admin", true);
        let docker = changes
            .iter()
            .find(|(name, _)| *name == "docker_users")
            .map(|(_, v)| v.as_str());
        assert_eq!(docker, Some("admin"));
    }

    #[test]
    fn apply_skips_docker_user_when_edited() {
        let d = defaults_for("ubuntu", "ubuntu-server-lts");
        let mut edited = HashSet::new();
        edited.insert("docker_users".to_string());
        let changes = apply_defaults(&d, &edited, "admin", true);
        assert!(changes.iter().all(|(name, _)| *name != "docker_users"));
    }

    #[test]
    fn summary_shows_count_and_groups() {
        let d = defaults_for("fedora", "fedora-server");
        let s = summary_for(&d);
        assert!(s.contains("packages"));
        assert!(s.contains("wheel"));
        assert!(s.contains("/bin/bash"));
    }
}
