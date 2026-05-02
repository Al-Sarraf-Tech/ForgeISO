//! [`ProfileKind::DesktopDeveloper`](super::ProfileKind::DesktopDeveloper) preset.
//!
//! Developer workstation: full toolchain (gcc, make, python3, nodejs, rust),
//! docker enabled, GUI desktop bits, codecs. Firewall stays off so local
//! dev servers (Vite, webpack-dev-server, Postgres, Redis, ...) work
//! without per-port allowlists.

use crate::config::{ContainerConfig, InjectConfig, NetworkConfig, SshConfig, UserConfig};

use super::DistroFamily;

/// Build a [`DesktopDeveloper`](super::ProfileKind::DesktopDeveloper) config
/// on top of `base`.
pub(super) fn populate(distro: &str, mut cfg: InjectConfig) -> InjectConfig {
    let family = DistroFamily::classify(distro);

    cfg.timezone.get_or_insert_with(|| "UTC".to_string());
    cfg.locale.get_or_insert_with(|| "en_US.UTF-8".to_string());
    cfg.keyboard_layout.get_or_insert_with(|| "us".to_string());

    // SSH on, password auth allowed (developers often use it for first login).
    cfg.ssh = SshConfig {
        authorized_keys: cfg.ssh.authorized_keys,
        allow_password_auth: cfg.ssh.allow_password_auth.or(Some(true)),
        install_server: Some(true),
    };

    if cfg.network.ntp_servers.is_empty() {
        cfg.network = NetworkConfig {
            dns_servers: cfg.network.dns_servers,
            ntp_servers: vec!["0.pool.ntp.org".to_string()],
        };
    }

    // Firewall OFF — devs need free ports for dev servers, containers,
    // language test runners, etc. Leaves the existing firewall block
    // intact (defaults to disabled) without explicitly setting it.

    // Sudo group (sudo on apt, wheel elsewhere) plus docker group so
    // the user can talk to the docker socket without sudo.
    let sudo_group = match family {
        DistroFamily::Dnf | DistroFamily::DnfWithEpel | DistroFamily::Pacman => "wheel",
        DistroFamily::Apt | DistroFamily::Zypper => "sudo",
    };
    let mut groups = cfg.user.groups.clone();
    for g in [sudo_group, "docker"] {
        if !groups.iter().any(|existing| existing == g) {
            groups.push(g.to_string());
        }
    }
    cfg.user = UserConfig {
        groups,
        shell: cfg.user.shell.or_else(|| Some("/bin/bash".to_string())),
        // Developers commonly want passwordless sudo on their own box.
        sudo_nopasswd: true,
        sudo_commands: cfg.user.sudo_commands,
    };

    // Developer toolchain + GUI + codecs.
    for pkg in developer_packages(family) {
        if !cfg.extra_packages.iter().any(|p| p == pkg) {
            cfg.extra_packages.push((*pkg).to_string());
        }
    }
    if matches!(family, DistroFamily::DnfWithEpel)
        && !cfg.extra_packages.iter().any(|p| p == "epel-release")
    {
        cfg.extra_packages.push("epel-release".to_string());
    }

    // Docker on (managed via late-commands by the engine), and put the
    // primary user into the docker group.
    let mut docker_users = cfg.containers.docker_users.clone();
    if let Some(user) = &cfg.username {
        if !docker_users.iter().any(|u| u == user) {
            docker_users.push(user.clone());
        }
    }
    cfg.containers = ContainerConfig {
        docker: true,
        podman: cfg.containers.podman,
        docker_users,
    };

    // Enable common dev services.
    let svc_names: &[&str] = match family {
        DistroFamily::Apt => &["ssh", "chrony", "docker"],
        _ => &["sshd", "chronyd", "docker"],
    };
    for svc in svc_names {
        if !cfg.enable_services.iter().any(|s| s == *svc) {
            cfg.enable_services.push((*svc).to_string());
        }
    }

    cfg
}

fn developer_packages(family: DistroFamily) -> &'static [&'static str] {
    match family {
        DistroFamily::Apt => &[
            "build-essential",
            "gcc",
            "make",
            "git",
            "vim",
            "curl",
            "htop",
            "ca-certificates",
            "python3",
            "python3-pip",
            "nodejs",
            "npm",
            "rustc",
            "cargo",
            "docker.io",
            "ubuntu-restricted-extras",
        ],
        DistroFamily::Dnf | DistroFamily::DnfWithEpel => &[
            "gcc",
            "gcc-c++",
            "make",
            "git",
            "vim",
            "curl",
            "htop",
            "ca-certificates",
            "python3",
            "python3-pip",
            "nodejs",
            "npm",
            "rust",
            "cargo",
            "docker",
            "gstreamer1-plugins-good",
            "gstreamer1-plugins-bad-free",
        ],
        DistroFamily::Pacman => &[
            "base-devel",
            "gcc",
            "make",
            "git",
            "vim",
            "curl",
            "htop",
            "ca-certificates",
            "python",
            "python-pip",
            "nodejs",
            "npm",
            "rust",
            "docker",
            "gst-plugins-good",
            "gst-plugins-bad",
        ],
        DistroFamily::Zypper => &[
            "patterns-devel-base-devel_basis",
            "gcc",
            "make",
            "git",
            "vim",
            "curl",
            "htop",
            "ca-certificates",
            "python3",
            "python3-pip",
            "nodejs",
            "npm",
            "rust",
            "cargo",
            "docker",
        ],
    }
}
