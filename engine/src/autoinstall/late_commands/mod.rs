use crate::config::InjectConfig;
use crate::error::EngineResult;

mod finalize;
mod packages;
mod system;
mod time_users;

/// Build all feature-specific late-commands in canonical order.
/// `pub` so that `kickstart.rs` can reuse this logic for Kickstart `%post`.
///
/// ORDER IS LOAD-BEARING — `kickstart.rs` relies on user-supplied entries
/// (`run_commands`, `extra_late_commands`) being the last `user_cmd_count`
/// elements so the Kickstart `%post` transformer can leave them unmodified.
#[allow(clippy::missing_errors_doc)]
pub fn build_feature_late_commands(cfg: &InjectConfig) -> EngineResult<Vec<String>> {
    let mut cmds = Vec::new();

    // 1. NTP servers
    time_users::append_ntp(cfg, &mut cmds);
    // 2. Wallpaper
    time_users::append_wallpaper(cfg, &mut cmds);
    // 3a. Mint-only SSH authorized_keys
    time_users::append_ssh_keys_mint(cfg, &mut cmds);
    // 3b. User groups, shell, sudo
    time_users::append_user_groups_shell_sudo(cfg, &mut cmds);

    // 4. Proxy
    system::append_proxy(cfg, &mut cmds);
    // 5. Enable/disable services
    system::append_services(cfg, &mut cmds);
    // 6. sysctl
    system::append_sysctl(cfg, &mut cmds);
    // 7. Swap
    system::append_swap(cfg, &mut cmds);
    // 8. Firewall
    system::append_firewall(cfg, &mut cmds);

    // 9. APT repos
    packages::append_apt_repos(cfg, &mut cmds);
    // 9b. Pacman repos + mirror
    packages::append_pacman_repos(cfg, &mut cmds);
    // 10. Docker
    packages::append_docker(cfg, &mut cmds);

    // 11. GRUB
    finalize::append_grub(cfg, &mut cmds);
    // 12. Custom mounts (fstab entries)
    finalize::append_mounts(cfg, &mut cmds);
    // 13 + 14. User run_commands and extra_late_commands (must be last)
    finalize::append_user_commands(cfg, &mut cmds);

    Ok(cmds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{IsoSource, UserConfig};

    #[test]
    fn test_wallpaper_dconf_has_no_spurious_backslash_before_quote() {
        // Regression: the dconf printf command used the format
        //   printf '...picture-uri="...jpg\"...' > file
        // where \" inside single-quoted shell argument is a literal backslash
        // followed by a double-quote. The dconf file therefore contained
        // `picture-uri="...jpg\"` — a malformed GVariant string value.
        // Fix: use printf '%s\n' with two separate arguments; double quotes
        // inside single-quoted shell args are literal and produce no backslash.
        let cmds = build_feature_late_commands(&InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".into(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: Default::default(),
            network: Default::default(),
            static_ip: None,
            gateway: None,
            distro: None,
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: Some(std::path::PathBuf::from("/tmp/bg.png")),
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            proxy: Default::default(),
            firewall: Default::default(),
            swap: None,
            encrypt: false,
            encrypt_passphrase: None,
            grub: Default::default(),
            mounts: vec![],
            run_commands: vec![],
            sysctl: vec![],
            apt_repos: vec![],
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
            enable_services: vec![],
            disable_services: vec![],
            containers: Default::default(),
        })
        .unwrap();
        let dconf_cmd = cmds
            .iter()
            .find(|c| c.contains("00-forgeiso-background"))
            .expect("dconf write command not found");
        // The command must not contain \" (backslash before closing quote)
        assert!(
            !dconf_cmd.contains(r#"\""#),
            "dconf command contains spurious backslash before quote: {dconf_cmd}"
        );
        // The command must contain picture-uri with a proper closing double-quote
        assert!(
            dconf_cmd.contains(r#"picture-uri=""#),
            "dconf command missing picture-uri key: {dconf_cmd}"
        );
    }

    #[test]
    fn docker_repo_entry_does_not_hardcode_amd64() {
        // Regression: arch=amd64 was hardcoded in the Docker apt repo entry.
        // On arm64 (AWS Graviton, Apple Silicon, RPi) Docker would fail to install.
        // The entry must use `$(dpkg --print-architecture)` and run inside the
        // chroot so it resolves against the TARGET system's architecture.
        let cfg = InjectConfig {
            containers: crate::config::ContainerConfig {
                docker: true,
                podman: false,
                docker_users: vec![],
            },
            ..Default::default()
        };
        let cmds = build_feature_late_commands(&cfg).unwrap();
        let docker_list_cmd = cmds
            .iter()
            .find(|c| c.contains("docker.list"))
            .expect("docker.list entry must be generated");
        assert!(
            !docker_list_cmd.contains("arch=amd64"),
            "Docker repo entry must not hardcode arch=amd64 (breaks arm64): {docker_list_cmd}"
        );
        assert!(
            docker_list_cmd.contains("dpkg --print-architecture"),
            "Docker repo entry must use dpkg --print-architecture: {docker_list_cmd}"
        );
        assert!(
            docker_list_cmd.starts_with("chroot /target bash -c"),
            "Docker repo entry must run inside chroot: {docker_list_cmd}"
        );
    }

    #[test]
    fn late_commands_omit_apt_and_ufw_for_fedora() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/fedora.iso"),
            out_name: "out.iso".to_string(),
            distro: Some(crate::config::Distro::Fedora),
            apt_repos: vec!["ppa:user/ppa".to_string()],
            containers: crate::config::ContainerConfig {
                docker: true,
                podman: false,
                docker_users: vec![],
            },
            firewall: crate::config::FirewallConfig {
                enabled: true,
                default_policy: Some("deny".to_string()),
                allow_ports: vec!["22/tcp".to_string()],
                deny_ports: vec![],
            },
            proxy: crate::config::ProxyConfig {
                http_proxy: Some("http://proxy.corp:3128".to_string()),
                https_proxy: None,
                no_proxy: vec![],
            },
            expected_sha256: None,
            ..Default::default()
        };
        let cmds = build_feature_late_commands(&cfg).unwrap();
        let all = cmds.join("\n");
        assert!(
            !all.contains("apt"),
            "apt commands must not appear for Fedora"
        );
        assert!(
            !all.contains("ufw"),
            "ufw commands must not appear for Fedora"
        );
        assert!(
            all.contains("http_proxy"),
            "/etc/environment proxy should still be set"
        );
        assert!(
            !all.contains("apt.conf.d"),
            "APT proxy config must not appear for Fedora"
        );
    }

    #[test]
    fn test_ntp_servers_appear_in_late_commands() {
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "test.iso".to_string(),
            network: crate::config::NetworkConfig {
                ntp_servers: vec![
                    "ntp1.example.com".to_string(),
                    "ntp2.example.com".to_string(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let cmds = build_feature_late_commands(&cfg).unwrap();
        let all = cmds.join("\n");
        assert!(all.contains("ntp1.example.com"), "NTP server 1 expected");
        assert!(all.contains("ntp2.example.com"), "NTP server 2 expected");
        assert!(all.contains("timesyncd"), "timesyncd config expected");
    }

    #[test]
    fn test_sudo_commands_in_late_commands() {
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "test.iso".to_string(),
            username: Some("admin".to_string()),
            user: crate::config::UserConfig {
                sudo_commands: vec!["/usr/bin/apt".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        let cmds = build_feature_late_commands(&cfg).unwrap();
        let all = cmds.join("\n");
        assert!(
            all.contains("/usr/bin/apt"),
            "sudo command should appear in late-commands"
        );
    }

    #[test]
    fn test_proxy_env_in_late_commands() {
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "test.iso".to_string(),
            proxy: crate::config::ProxyConfig {
                http_proxy: Some("http://proxy:3128".to_string()),
                https_proxy: Some("http://proxy:3128".to_string()),
                no_proxy: vec!["localhost".to_string(), "127.0.0.1".to_string()],
            },
            ..Default::default()
        };
        let cmds = build_feature_late_commands(&cfg).unwrap();
        let all = cmds.join("\n");
        assert!(all.contains("http_proxy"), "http_proxy env expected");
        assert!(all.contains("https_proxy"), "https_proxy env expected");
        assert!(all.contains("no_proxy"), "no_proxy env expected");
    }

    #[test]
    fn test_mount_entries_in_late_commands() {
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "test.iso".to_string(),
            mounts: vec!["/dev/sda2 /data ext4 defaults 0 2".to_string()],
            ..Default::default()
        };
        let cmds = build_feature_late_commands(&cfg).unwrap();
        let all = cmds.join("\n");
        assert!(all.contains("fstab"), "fstab entry expected");
        assert!(all.contains("/dev/sda2"), "mount device expected");
        assert!(all.contains("mkdir"), "mountpoint mkdir expected");
    }

    #[test]
    fn test_apt_repos_in_late_commands() {
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "test.iso".to_string(),
            apt_repos: vec!["deb http://archive.ubuntu.com/ubuntu noble main".to_string()],
            ..Default::default()
        };
        let cmds = build_feature_late_commands(&cfg).unwrap();
        let all = cmds.join("\n");
        assert!(
            all.contains("archive.ubuntu.com"),
            "APT repo URL expected in late commands"
        );
    }

    #[test]
    fn mint_ssh_keys_use_single_quoted_printf() {
        // Regression history:
        //  v1: printf '%s\n' {key:?}  — Rust Debug quoting wraps key in double
        //      quotes; $() and ` are expanded in shell.
        //  v2: single-quoted heredoc  — no shell expansion, but produces multi-
        //      line commands.  Multi-line commands in preseed/late_command break
        //      the preseed file format (late_command is a single-line directive).
        //  v3 (current): printf '%s\n' 'key' — single-quoted arg prevents all
        //      shell expansion; produces a single-line command compatible with
        //      the preseed format.  Single quotes in the key are blocked by
        //      InjectConfig::validate().
        use crate::config::{Distro, SshConfig};
        // $(id) inside single quotes is literal — no expansion occurs.
        let key_with_dollar = "ssh-ed25519 AAAAC3Nz... $(id)@host";
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "test.iso".to_string(),
            distro: Some(Distro::Mint),
            username: Some("tester".to_string()),
            ssh: SshConfig {
                authorized_keys: vec![key_with_dollar.to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        let cmds = build_feature_late_commands(&cfg).unwrap();
        let all = cmds.join("\n");
        // Must use printf with single-quoted key content
        assert!(
            all.contains("printf '%s\\n' '"),
            "Mint SSH key must use printf with single-quoted content to prevent expansion: {all}"
        );
        // Key content must appear verbatim (inside single quotes)
        assert!(
            all.contains(key_with_dollar),
            "SSH key content must appear verbatim inside single quotes: {all}"
        );
        // Must NOT embed a heredoc (multi-line commands break preseed/late_command)
        assert!(
            !all.contains("FORGEISO_KEY_EOF"),
            "heredoc sentinel must not appear — multi-line commands break preseed format: {all}"
        );
        // Verify the command is single-line (no embedded newlines in the key command)
        let key_cmd = cmds
            .iter()
            .find(|c| c.contains("authorized_keys"))
            .expect("authorized_keys command not found");
        assert!(
            !key_cmd.contains('\n'),
            "authorized_keys command must be single-line for preseed compatibility: {key_cmd:?}"
        );
    }

    // ── mount entry without mountpoint ────────────────────────────────────────

    #[test]
    fn mount_entry_with_mountpoint_generates_mkdir() {
        // A well-formed fstab entry should generate a `mkdir -p /target<mountpoint>` command.
        let cfg = crate::config::InjectConfig {
            mounts: vec!["/dev/sdb1 /data ext4 defaults 0 2".to_string()],
            ..Default::default()
        };
        let cmds = build_feature_late_commands(&cfg).unwrap();
        assert!(
            cmds.iter().any(|c| c.contains("mkdir -p /target/data")),
            "mount with valid mountpoint must generate mkdir: {cmds:?}"
        );
        assert!(
            cmds.iter()
                .any(|c| c.contains("/data ext4") && c.contains("fstab")),
            "fstab entry must still be written: {cmds:?}"
        );
    }

    #[test]
    fn mount_entry_without_mountpoint_skips_mkdir_but_writes_fstab() {
        // An fstab entry with no second whitespace field must NOT silently mkdir /mnt.
        // It must still write the line to fstab.
        let cfg = crate::config::InjectConfig {
            mounts: vec!["/dev/sdb1".to_string()],
            ..Default::default()
        };
        let cmds = build_feature_late_commands(&cfg).unwrap();
        assert!(
            !cmds.iter().any(|c| c.contains("mkdir -p /target/mnt")),
            "malformed mount entry must not mkdir /mnt silently: {cmds:?}"
        );
        assert!(
            cmds.iter()
                .any(|c| c.contains("/dev/sdb1") && c.contains("fstab")),
            "fstab entry must still be written for malformed mount: {cmds:?}"
        );
    }
}
