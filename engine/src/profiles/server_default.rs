//! [`ProfileKind::ServerDefault`](super::ProfileKind::ServerDefault) preset.
//!
//! Sensible production defaults for a generic Linux server: SSH on, a
//! permissive-but-on firewall (allow 22/80/443), NTP from `pool.ntp.org`,
//! a baseline package set (vim, curl, git, htop, ...), wheel/sudo group
//! membership, and no SSH keys (user adds their own afterwards).

use crate::config::{FirewallConfig, InjectConfig, NetworkConfig, SshConfig, UserConfig};

use super::DistroFamily;

/// Build a [`ServerDefault`](super::ProfileKind::ServerDefault) config on top
/// of `base`. Distro-aware: package names and sudo group differ across
/// families.
pub(super) fn populate(distro: &str, mut cfg: InjectConfig) -> InjectConfig {
    let family = DistroFamily::classify(distro);

    // Identity: leave hostname/username/password/realname for the user,
    // but make sure timezone/locale/keyboard have safe defaults if unset.
    cfg.timezone.get_or_insert_with(|| "UTC".to_string());
    cfg.locale.get_or_insert_with(|| "en_US.UTF-8".to_string());
    cfg.keyboard_layout.get_or_insert_with(|| "us".to_string());

    // SSH on, install server, password auth left to engine default
    // (engine flips it off automatically when keys are present).
    cfg.ssh = SshConfig {
        authorized_keys: cfg.ssh.authorized_keys,
        allow_password_auth: cfg.ssh.allow_password_auth.or(Some(true)),
        install_server: Some(true),
    };

    // NTP via the public pool — overridable per-deployment.
    if cfg.network.ntp_servers.is_empty() {
        cfg.network = NetworkConfig {
            dns_servers: cfg.network.dns_servers,
            ntp_servers: vec![
                "0.pool.ntp.org".to_string(),
                "1.pool.ntp.org".to_string(),
                "2.pool.ntp.org".to_string(),
            ],
        };
    }

    // Firewall on, default deny incoming, common server ports allowed.
    cfg.firewall = FirewallConfig {
        enabled: true,
        default_policy: Some("deny".to_string()),
        allow_ports: vec!["22".to_string(), "80".to_string(), "443".to_string()],
        deny_ports: cfg.firewall.deny_ports,
    };

    // Sudo group: wheel on RPM/Arch, sudo on Debian/Ubuntu.
    let sudo_group = match family {
        DistroFamily::Dnf | DistroFamily::DnfWithEpel | DistroFamily::Pacman => "wheel",
        DistroFamily::Apt | DistroFamily::Zypper => "sudo",
    };
    let mut groups = cfg.user.groups.clone();
    if !groups.iter().any(|g| g == sudo_group) {
        groups.push(sudo_group.to_string());
    }
    cfg.user = UserConfig {
        groups,
        shell: cfg.user.shell.or_else(|| Some("/bin/bash".to_string())),
        sudo_nopasswd: cfg.user.sudo_nopasswd,
        sudo_commands: cfg.user.sudo_commands,
    };

    // Baseline packages, lightly distro-mapped where names differ.
    let baseline_pkgs = baseline_packages(family);
    for pkg in baseline_pkgs {
        if !cfg.extra_packages.iter().any(|p| p == pkg) {
            cfg.extra_packages.push((*pkg).to_string());
        }
    }

    // RHEL family wants EPEL for some of the baseline packages (htop on
    // older releases, etc.). EPEL release packages live in the base repos
    // for Rocky/Alma/CentOS-Stream so we just add the package name; the
    // dnf transaction picks it up.
    if matches!(family, DistroFamily::DnfWithEpel)
        && !cfg.extra_packages.iter().any(|p| p == "epel-release")
    {
        cfg.extra_packages.push("epel-release".to_string());
    }

    // Enable common services on first boot.
    for svc in ["sshd", "chronyd"] {
        // Use distro-correct service name: ssh on Debian/Ubuntu, sshd elsewhere.
        let real = if svc == "sshd" && matches!(family, DistroFamily::Apt) {
            "ssh"
        } else if svc == "chronyd" && matches!(family, DistroFamily::Apt) {
            "chrony"
        } else {
            svc
        };
        if !cfg.enable_services.iter().any(|s| s == real) {
            cfg.enable_services.push(real.to_string());
        }
    }

    cfg
}

/// Baseline package set, with distro-specific name mapping where needed.
fn baseline_packages(family: DistroFamily) -> &'static [&'static str] {
    match family {
        DistroFamily::Apt => &["vim", "curl", "git", "htop", "ca-certificates", "chrony"],
        DistroFamily::Dnf | DistroFamily::DnfWithEpel => {
            &["vim", "curl", "git", "htop", "ca-certificates", "chrony"]
        }
        DistroFamily::Pacman => &["vim", "curl", "git", "htop", "ca-certificates", "chrony"],
        DistroFamily::Zypper => &["vim", "curl", "git", "htop", "ca-certificates", "chrony"],
    }
}
