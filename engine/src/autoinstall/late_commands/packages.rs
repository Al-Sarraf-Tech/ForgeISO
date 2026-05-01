use crate::config::{Distro, InjectConfig};

/// Section 9 — APT extra repositories (Ubuntu/Debian only).
pub(super) fn append_apt_repos(cfg: &InjectConfig, cmds: &mut Vec<String>) {
    let is_ubuntu = !matches!(cfg.distro, Some(Distro::Fedora | Distro::Arch));
    if !is_ubuntu {
        return;
    }
    for repo in &cfg.apt_repos {
        if repo.starts_with("ppa:") {
            cmds.push(format!("chroot /target add-apt-repository -y '{repo}'"));
        } else {
            cmds.push(format!(
                "echo '{repo}' >> /target/etc/apt/sources.list.d/forgeiso-extra.list"
            ));
        }
    }
    if !cfg.apt_repos.is_empty() {
        cmds.push("chroot /target apt-get update".to_string());
    }
}

/// Section 9b — Pacman mirror override and extra repositories (Arch only).
pub(super) fn append_pacman_repos(cfg: &InjectConfig, cmds: &mut Vec<String>) {
    let is_arch = matches!(cfg.distro, Some(Distro::Arch));
    if !is_arch {
        return;
    }
    // Override primary mirror
    if let Some(mirror) = &cfg.pacman_mirror {
        cmds.push(format!(
            "echo 'Server = {mirror}/$repo/os/$arch' > /target/etc/pacman.d/mirrorlist"
        ));
    }
    // Append extra Server= lines to mirrorlist
    for repo in &cfg.pacman_repos {
        let line = repo.trim();
        if !line.is_empty() {
            cmds.push(format!("echo '{line}' >> /target/etc/pacman.d/mirrorlist"));
        }
    }
    // Refresh package database after mirror changes
    if cfg.pacman_mirror.is_some() || !cfg.pacman_repos.is_empty() {
        cmds.push("chroot /target pacman -Sy --noconfirm".to_string());
    }
}

/// Section 10 — Docker CE installation via apt (Ubuntu only).
/// Fedora handles `docker-ce` separately in `kickstart.rs`.
pub(super) fn append_docker(cfg: &InjectConfig, cmds: &mut Vec<String>) {
    let is_ubuntu = !matches!(cfg.distro, Some(Distro::Fedora | Distro::Arch));
    if !(cfg.containers.docker && is_ubuntu) {
        return;
    }
    cmds.push("install -m 0755 -d /target/etc/apt/keyrings".to_string());
    cmds.push(
        "curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /target/etc/apt/keyrings/docker.gpg".to_string()
    );
    cmds.push("chmod a+r /target/etc/apt/keyrings/docker.gpg".to_string());
    // Run the repo-entry command inside the chroot so both dpkg --print-architecture
    // and /etc/os-release resolve against the TARGET system, not the installer.
    // Hardcoding arch=amd64 would break Docker installation on arm64 hosts
    // (AWS Graviton, Apple Silicon, Raspberry Pi).  Using $() inside single-quoted
    // bash -c '...' is intentional: the outer shell treats the argument as a
    // literal; bash -c evaluates the $() substitutions inside the chroot.
    cmds.push(
        r#"chroot /target bash -c 'echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo $VERSION_CODENAME) stable" > /etc/apt/sources.list.d/docker.list'"#.to_string()
    );
    cmds.push("chroot /target apt-get update".to_string());
    cmds.push(
        "chroot /target apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin".to_string()
    );
    cmds.push("chroot /target systemctl enable docker".to_string());
    for user in &cfg.containers.docker_users {
        cmds.push(format!("chroot /target usermod -aG docker {user}"));
    }
}
