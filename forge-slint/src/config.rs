use std::path::PathBuf;

use forgeiso_engine::{
    find_preset_by_str, resolve_url, AcquisitionStrategy, ContainerConfig, Distro, FirewallConfig,
    GrubConfig, InjectConfig, IsoSource, NetworkConfig, ProxyConfig, SshConfig, SwapConfig,
    UserConfig,
};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::state::{lines, opt, tokens, InjectState};
use crate::{clear_build_results, AppState, AppWindow, FormState, PresetCard};

// ── Preset cards shown on Step 1 ─────────────────────────────────────────────

/// Returns (row1, row2) preset card models for the two-row distro grid.
pub fn make_preset_cards() -> (ModelRc<PresetCard>, ModelRc<PresetCard>) {
    let row1: Vec<PresetCard> = vec![
        PresetCard {
            id: "ubuntu-server-lts".into(),
            emoji: "\u{1F427}".into(), // 🐧
            name: "Ubuntu Server".into(),
            desc: "LTS Server".into(),
        },
        PresetCard {
            id: "fedora-server".into(),
            emoji: "\u{1F3A9}".into(), // 🎩
            name: "Fedora Server".into(),
            desc: "Latest stable".into(),
        },
        PresetCard {
            id: "linux-mint-cinnamon".into(),
            emoji: "\u{1F33F}".into(), // 🌿
            name: "Linux Mint".into(),
            desc: "Cinnamon".into(),
        },
        PresetCard {
            id: "arch-linux".into(),
            emoji: "\u{2699}\u{FE0F}".into(), // ⚙️
            name: "Arch Linux".into(),
            desc: "Rolling release".into(),
        },
    ];
    let row2: Vec<PresetCard> = vec![
        PresetCard {
            id: "rocky-linux".into(),
            emoji: "\u{1FAA8}".into(), // 🪨
            name: "Rocky Linux".into(),
            desc: "RHEL compatible".into(),
        },
        PresetCard {
            id: "almalinux".into(),
            emoji: "\u{1F9AC}".into(), // 🦬
            name: "AlmaLinux".into(),
            desc: "RHEL compatible".into(),
        },
        PresetCard {
            id: "centos-stream".into(),
            emoji: "\u{1F534}".into(), // 🔴
            name: "CentOS Stream".into(),
            desc: "RHEL upstream".into(),
        },
        PresetCard {
            id: "ubuntu-server-jammy".into(),
            emoji: "\u{1F427}".into(), // 🐧
            name: "Ubuntu 22.04".into(),
            desc: "Server Jammy LTS".into(),
        },
    ];
    (
        ModelRc::new(VecModel::from(row1)),
        ModelRc::new(VecModel::from(row2)),
    )
}

pub fn preset_display_name(id: &str) -> Option<&'static str> {
    find_preset_by_str(id).map(|preset| preset.name)
}

// ── Preset selection handler ──────────────────────────────────────────────────

pub fn handle_preset_clicked(w: &AppWindow, id: &str, app: &mut crate::app::ForgeApp) {
    if let Some(p) = find_preset_by_str(id) {
        let fs = w.global::<FormState>();
        fs.set_selected_preset(p.id.as_str().into());
        fs.set_selected_preset_name(p.name.into());
        fs.set_distro(p.distro.into());
        // Clear stale build state
        let gs = w.global::<AppState>();
        gs.set_step2_done(false);
        clear_build_results(w);

        if p.strategy == AcquisitionStrategy::DirectUrl {
            if let Ok(Some(url)) = resolve_url(p) {
                fs.set_source_path(url.into());
                // Trigger ISO detection on URL-resolved presets
                app.spawn_detect_iso(fs.get_source_path().into());
            }
        }

        // Apply distro-aware defaults for this preset.
        app.apply_distro_defaults(w);
    }
}

// ── InjectConfig builder ──────────────────────────────────────────────────────

pub fn build_inject_config(s: &InjectState) -> InjectConfig {
    let source = IsoSource::from_raw(s.source.trim());
    let shared_repo_lines = lines(&s.apt_repos);

    let distro = match s.distro.as_str() {
        "fedora" => Some(Distro::Fedora),
        "arch" => Some(Distro::Arch),
        "mint" => Some(Distro::Mint),
        _ => None,
    };

    let ssh = SshConfig {
        authorized_keys: lines(&s.ssh_keys),
        install_server: Some(s.ssh_install_server),
        allow_password_auth: Some(s.ssh_password_auth),
    };

    let network = NetworkConfig {
        dns_servers: tokens(&s.dns_servers),
        ntp_servers: tokens(&s.ntp_servers),
    };

    let proxy = ProxyConfig {
        http_proxy: opt(&s.http_proxy),
        https_proxy: opt(&s.https_proxy),
        no_proxy: tokens(&s.no_proxy),
    };

    let user = UserConfig {
        groups: lines(&s.user_groups),
        shell: opt(&s.user_shell),
        sudo_nopasswd: s.sudo_nopasswd,
        sudo_commands: lines(&s.sudo_commands),
    };

    let firewall = FirewallConfig {
        enabled: s.firewall_enabled,
        default_policy: opt(&s.firewall_policy),
        allow_ports: tokens(&s.allow_ports),
        deny_ports: tokens(&s.deny_ports),
    };

    let swap = s
        .swap_size_mb
        .parse::<u32>()
        .ok()
        .map(|size_mb| SwapConfig {
            size_mb,
            filename: opt(&s.swap_filename),
            swappiness: s.swap_swappiness.parse::<u8>().ok(),
        });

    let containers = ContainerConfig {
        docker: s.docker,
        podman: s.podman,
        docker_users: lines(&s.docker_users),
    };

    let grub = GrubConfig {
        timeout: s.grub_timeout.parse::<u32>().ok(),
        cmdline_extra: tokens(&s.grub_cmdline),
        default_entry: opt(&s.grub_default),
    };

    let sysctl: Vec<(String, String)> = s
        .sysctl_pairs
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let mut parts = l.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some(k), Some(v)) => {
                    let k = k.trim().to_string();
                    let v = v.trim().to_string();
                    if k.is_empty() || v.is_empty() {
                        None
                    } else {
                        Some((k, v))
                    }
                }
                _ => None,
            }
        })
        .collect();

    InjectConfig {
        source,
        autoinstall_yaml: None,
        out_name: s.out_name.clone(),
        output_label: opt(&s.output_label),
        expected_sha256: opt(&s.expected_sha256),
        hostname: opt(&s.hostname),
        username: opt(&s.username),
        password: opt(&s.password),
        realname: opt(&s.realname),
        ssh,
        network,
        proxy,
        user,
        timezone: opt(&s.timezone),
        locale: opt(&s.locale),
        keyboard_layout: opt(&s.keyboard_layout),
        storage_layout: opt(&s.storage_layout),
        apt_mirror: opt(&s.apt_mirror),
        extra_packages: tokens(&s.packages),
        wallpaper: opt(&s.wallpaper_path).map(PathBuf::from),
        extra_late_commands: lines(&s.late_commands),
        no_user_interaction: s.no_user_interaction,
        firewall,
        static_ip: opt(&s.static_ip),
        gateway: opt(&s.gateway),
        enable_services: lines(&s.enable_services),
        disable_services: lines(&s.disable_services),
        sysctl,
        swap,
        apt_repos: if matches!(distro, None | Some(Distro::Mint)) {
            shared_repo_lines.clone()
        } else {
            Vec::new()
        },
        dnf_repos: if matches!(distro, Some(Distro::Fedora)) {
            let repos = lines(&s.dnf_repos);
            if repos.is_empty() {
                shared_repo_lines.clone()
            } else {
                repos
            }
        } else {
            Vec::new()
        },
        dnf_mirror: opt(&s.dnf_mirror),
        pacman_repos: if matches!(distro, Some(Distro::Arch)) {
            let repos = lines(&s.pacman_repos);
            if repos.is_empty() {
                shared_repo_lines.clone()
            } else {
                repos
            }
        } else {
            Vec::new()
        },
        pacman_mirror: opt(&s.pacman_mirror),
        containers,
        grub,
        encrypt: s.encrypt,
        encrypt_passphrase: opt(&s.encrypt_passphrase),
        mounts: lines(&s.mounts),
        run_commands: lines(&s.run_commands),
        distro,
    }
}

#[cfg(test)]
mod tests {
    use super::{build_inject_config, preset_display_name};
    use crate::state::InjectState;

    #[test]
    fn preset_display_name_uses_engine_metadata() {
        assert_eq!(
            preset_display_name("ubuntu-server-lts"),
            Some("Ubuntu 24.04.4 LTS Server")
        );
        assert_eq!(preset_display_name("arch-linux"), Some("Arch Linux"));
    }

    #[test]
    fn preset_display_name_rejects_unknown_ids() {
        assert_eq!(preset_display_name("unknown-preset"), None);
    }

    #[test]
    fn build_inject_config_splits_flexible_token_fields() {
        let state = InjectState {
            dns_servers: "1.1.1.1, 8.8.8.8".into(),
            ntp_servers: "time1.example.com\ntime2.example.com".into(),
            no_proxy: "localhost,127.0.0.1 internal.example.com".into(),
            packages: "curl git\nhtop".into(),
            allow_ports: "22 80/tcp".into(),
            deny_ports: "23,25".into(),
            grub_cmdline: "quiet splash".into(),
            ..InjectState::default()
        };

        let cfg = build_inject_config(&state);
        assert_eq!(cfg.network.dns_servers, vec!["1.1.1.1", "8.8.8.8"]);
        assert_eq!(
            cfg.network.ntp_servers,
            vec!["time1.example.com", "time2.example.com"]
        );
        assert_eq!(
            cfg.proxy.no_proxy,
            vec!["localhost", "127.0.0.1", "internal.example.com"]
        );
        assert_eq!(cfg.extra_packages, vec!["curl", "git", "htop"]);
        assert_eq!(cfg.firewall.allow_ports, vec!["22", "80/tcp"]);
        assert_eq!(cfg.firewall.deny_ports, vec!["23", "25"]);
        assert_eq!(cfg.grub.cmdline_extra, vec!["quiet", "splash"]);
    }
}
