//! [`ProfileKind::ServerHardened`](super::ProfileKind::ServerHardened) preset.
//!
//! Security-first server: firewall default deny with only 22/80/443
//! explicitly allowed, SSH password auth OFF and root login OFF, audit
//! tooling installed, fail2ban for brute-force protection, strict
//! sysctl, no container runtime by default.

use crate::config::{
    ContainerConfig, FirewallConfig, InjectConfig, NetworkConfig, SshConfig, UserConfig,
};

use super::DistroFamily;

/// Build a [`ServerHardened`](super::ProfileKind::ServerHardened) config on top
/// of `base`. Distro-aware for package names and audit tooling.
pub(super) fn populate(distro: &str, mut cfg: InjectConfig) -> InjectConfig {
    let family = DistroFamily::classify(distro);

    cfg.timezone.get_or_insert_with(|| "UTC".to_string());
    cfg.locale.get_or_insert_with(|| "en_US.UTF-8".to_string());
    cfg.keyboard_layout.get_or_insert_with(|| "us".to_string());

    // SSH: no password auth, no root login. install_server stays on so the
    // host is reachable; the operator MUST add an authorized_key separately.
    cfg.ssh = SshConfig {
        authorized_keys: cfg.ssh.authorized_keys,
        allow_password_auth: Some(false),
        install_server: Some(true),
    };

    if cfg.network.ntp_servers.is_empty() {
        cfg.network = NetworkConfig {
            dns_servers: cfg.network.dns_servers,
            ntp_servers: vec!["0.pool.ntp.org".to_string(), "1.pool.ntp.org".to_string()],
        };
    }

    // Firewall: default deny, only 22/80/443.
    cfg.firewall = FirewallConfig {
        enabled: true,
        default_policy: Some("deny".to_string()),
        allow_ports: vec!["22".to_string(), "80".to_string(), "443".to_string()],
        deny_ports: cfg.firewall.deny_ports,
    };

    // Sudo group plus a non-NOPASSWD policy.
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
        // Hardened: never sudo without a password.
        sudo_nopasswd: false,
        sudo_commands: cfg.user.sudo_commands,
    };

    // Hardened package set: audit, fail2ban, baseline tools.
    for pkg in hardened_packages(family) {
        if !cfg.extra_packages.iter().any(|p| p == pkg) {
            cfg.extra_packages.push((*pkg).to_string());
        }
    }
    if matches!(family, DistroFamily::DnfWithEpel)
        && !cfg.extra_packages.iter().any(|p| p == "epel-release")
    {
        cfg.extra_packages.push("epel-release".to_string());
    }

    // Hardening sysctls -- conservative, verbatim values.
    let hardening_sysctls = [
        ("net.ipv4.conf.all.rp_filter", "1"),
        ("net.ipv4.conf.default.rp_filter", "1"),
        ("net.ipv4.icmp_echo_ignore_broadcasts", "1"),
        ("net.ipv4.conf.all.accept_redirects", "0"),
        ("net.ipv4.conf.all.send_redirects", "0"),
        ("net.ipv4.tcp_syncookies", "1"),
        ("kernel.kptr_restrict", "2"),
        ("kernel.dmesg_restrict", "1"),
    ];
    for (k, v) in hardening_sysctls {
        if !cfg.sysctl.iter().any(|(existing, _)| existing == k) {
            cfg.sysctl.push((k.to_string(), v.to_string()));
        }
    }

    // No container runtime by default in the hardened profile.
    cfg.containers = ContainerConfig {
        docker: false,
        podman: false,
        docker_users: cfg.containers.docker_users,
    };

    // Enable security-relevant services.
    let svc_names: &[&str] = match family {
        DistroFamily::Apt => &["ssh", "chrony", "auditd", "fail2ban"],
        _ => &["sshd", "chronyd", "auditd", "fail2ban"],
    };
    for svc in svc_names {
        if !cfg.enable_services.iter().any(|s| s == *svc) {
            cfg.enable_services.push((*svc).to_string());
        }
    }

    cfg
}

fn hardened_packages(family: DistroFamily) -> &'static [&'static str] {
    match family {
        DistroFamily::Apt => &[
            "vim",
            "curl",
            "git",
            "htop",
            "ca-certificates",
            "chrony",
            "auditd",
            "fail2ban",
            "apparmor",
            "apparmor-utils",
            "unattended-upgrades",
        ],
        DistroFamily::Dnf | DistroFamily::DnfWithEpel => &[
            "vim",
            "curl",
            "git",
            "htop",
            "ca-certificates",
            "chrony",
            "audit",
            "fail2ban",
            "policycoreutils",
            "selinux-policy-targeted",
            "dnf-automatic",
        ],
        DistroFamily::Pacman => &[
            "vim",
            "curl",
            "git",
            "htop",
            "ca-certificates",
            "chrony",
            "audit",
            "fail2ban",
            "apparmor",
        ],
        DistroFamily::Zypper => &[
            "vim",
            "curl",
            "git",
            "htop",
            "ca-certificates",
            "chrony",
            "audit",
            "fail2ban",
            "apparmor-utils",
        ],
    }
}
