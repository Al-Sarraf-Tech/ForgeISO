//! [`ProfileKind::MinimalCloud`](super::ProfileKind::MinimalCloud) preset.
//!
//! Tiny cloud-init friendly base. No extra packages, SSH password auth ON
//! for first-boot bootstrap, no GUI. Intended for users who will layer
//! their own provisioning (Ansible, cloud-init user-data, Terraform
//! cloud-init `user_data`) on top after the box comes up.

use crate::config::{
    ContainerConfig, FirewallConfig, InjectConfig, NetworkConfig, SshConfig, UserConfig,
};

use super::DistroFamily;

/// Build a [`MinimalCloud`](super::ProfileKind::MinimalCloud) config on top
/// of `base`.
pub(super) fn populate(distro: &str, mut cfg: InjectConfig) -> InjectConfig {
    let family = DistroFamily::classify(distro);

    cfg.timezone.get_or_insert_with(|| "UTC".to_string());
    cfg.locale.get_or_insert_with(|| "en_US.UTF-8".to_string());
    cfg.keyboard_layout.get_or_insert_with(|| "us".to_string());

    // Cloud bootstrap: SSH on, password auth explicitly ON so cloud-init
    // can hand off to the operator's provisioner before key material is
    // injected.
    cfg.ssh = SshConfig {
        authorized_keys: cfg.ssh.authorized_keys,
        allow_password_auth: Some(true),
        install_server: Some(true),
    };

    // Use the cloud's default NTP if the user did not specify one. We
    // still want SOMETHING set so clock drift does not break TLS.
    if cfg.network.ntp_servers.is_empty() {
        cfg.network = NetworkConfig {
            dns_servers: cfg.network.dns_servers,
            ntp_servers: vec!["pool.ntp.org".to_string()],
        };
    }

    // No firewall (cloud network isolation handles it).
    cfg.firewall = FirewallConfig {
        enabled: false,
        default_policy: cfg.firewall.default_policy,
        allow_ports: cfg.firewall.allow_ports,
        deny_ports: cfg.firewall.deny_ports,
    };

    // No container runtime by default.
    cfg.containers = ContainerConfig {
        docker: false,
        podman: false,
        docker_users: cfg.containers.docker_users,
    };

    // Sudo group only.
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

    // No baseline packages added: this profile is intentionally minimal.
    // Only ensure cloud-init itself is present (some non-cloud images do
    // not ship it).
    let cloud_pkg = match family {
        DistroFamily::Apt => "cloud-init",
        DistroFamily::Dnf | DistroFamily::DnfWithEpel => "cloud-init",
        DistroFamily::Pacman => "cloud-init",
        DistroFamily::Zypper => "cloud-init",
    };
    if !cfg.extra_packages.iter().any(|p| p == cloud_pkg) {
        cfg.extra_packages.push(cloud_pkg.to_string());
    }

    // Enable ssh + cloud-init + ntp.
    let svc_names: &[&str] = match family {
        DistroFamily::Apt => &["ssh", "chrony", "cloud-init"],
        _ => &["sshd", "chronyd", "cloud-init"],
    };
    for svc in svc_names {
        if !cfg.enable_services.iter().any(|s| s == *svc) {
            cfg.enable_services.push((*svc).to_string());
        }
    }

    cfg
}
