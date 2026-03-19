use crate::config::{Distro, InjectConfig};
use crate::error::EngineResult;

/// Build all feature-specific late-commands in canonical order.
/// `pub` so that `kickstart.rs` can reuse this logic for Kickstart `%post`.
#[allow(clippy::too_many_lines)]
#[allow(clippy::missing_errors_doc)]
pub fn build_feature_late_commands(cfg: &InjectConfig) -> EngineResult<Vec<String>> {
    let mut cmds = Vec::new();

    // 1. NTP servers
    if !cfg.network.ntp_servers.is_empty() {
        let ntp_list = cfg.network.ntp_servers.join(" ");
        cmds.push(format!(
            "printf '[Time]\\nNTP={ntp_list}\\n' > /target/etc/systemd/timesyncd.conf"
        ));
        cmds.push("chroot /target systemctl enable systemd-timesyncd".to_string());
    }

    // 2. Wallpaper
    if let Some(wallpaper_path) = &cfg.wallpaper {
        if let Some(filename) = wallpaper_path.file_name() {
            if let Some(filename_str) = filename.to_str() {
                let ext = wallpaper_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("jpg");
                cmds.push(format!(
                    "cp /cdrom/wallpaper/{filename_str} /target/usr/share/backgrounds/forgeiso-wallpaper.{ext}"
                ));
                cmds.push("mkdir -p /target/etc/dconf/db/local.d".to_string());
                // Use printf '%s\n' with two separate arguments so the dconf
                // value double-quotes are literal characters inside single-quoted
                // shell arguments — avoids the \" backslash-quote artifact that
                // appears when double-quotes are escaped inside a single-quoted
                // printf format string.
                cmds.push(format!(
                    "printf '%s\\n' '[org/gnome/desktop/background]' 'picture-uri=\"file:///usr/share/backgrounds/forgeiso-wallpaper.{ext}\"' > /target/etc/dconf/db/local.d/00-forgeiso-background"
                ));
                cmds.push("chroot /target dconf update".to_string());
            }
        }
    }

    // 3a. SSH authorized_keys — injected via late_command for Mint (preseed path).
    //     Ubuntu handles this in the cloud-init YAML; Fedora uses the `sshkey`
    //     Kickstart directive; Arch handles it via the archinstall JSON !users list.
    //     Only emit for Mint so we avoid duplicating what the YAML already does.
    let is_mint = matches!(cfg.distro, Some(Distro::Mint));
    if is_mint && !cfg.ssh.authorized_keys.is_empty() {
        let uname = cfg.username.as_deref().unwrap_or("user");
        let ssh_dir = format!("/target/home/{uname}/.ssh");
        cmds.push(format!("mkdir -p {ssh_dir}"));
        for key in &cfg.ssh.authorized_keys {
            // Use printf '%s\n' with the key in SINGLE quotes so the content is
            // literal — no variable expansion ($), command substitution (`), or
            // backslash processing.  This MUST be a single-line command because
            // for Mint the late-commands are joined into a preseed/late_command
            // directive, which must be a single line.  Multi-line heredocs (the
            // alternative approach) embed literal newlines in that directive and
            // break the preseed file format.
            // The InjectConfig::validate() check ensures the key contains no
            // single quotes (which would break out of single-quoting) and no
            // FORGEISO_KEY_EOF sentinel (defense in depth from the heredoc era).
            cmds.push(format!(
                "printf '%s\\n' '{key}' >> {ssh_dir}/authorized_keys"
            ));
        }
        cmds.push(format!("chmod 700 {ssh_dir}"));
        cmds.push(format!("chmod 600 {ssh_dir}/authorized_keys"));
        cmds.push(format!("chown -R {uname}:{uname} {ssh_dir}"));
    }

    // 3b. User groups, shell, sudo
    if !cfg.user.groups.is_empty() {
        let groups = cfg.user.groups.join(",");
        let uname = cfg.username.as_deref().unwrap_or("ubuntu");
        cmds.push(format!("chroot /target usermod -aG {groups} {uname}"));
    }
    if let Some(shell) = &cfg.user.shell {
        let uname = cfg.username.as_deref().unwrap_or("ubuntu");
        cmds.push(format!("chroot /target chsh -s {shell} {uname}"));
    }
    if cfg.user.sudo_nopasswd {
        let uname = cfg.username.as_deref().unwrap_or("ubuntu");
        cmds.push(format!(
            "echo '{uname} ALL=(ALL) NOPASSWD:ALL' > /target/etc/sudoers.d/nopasswd-{uname}"
        ));
        cmds.push(format!("chmod 440 /target/etc/sudoers.d/nopasswd-{uname}"));
    } else if !cfg.user.sudo_commands.is_empty() {
        let uname = cfg.username.as_deref().unwrap_or("ubuntu");
        let cmds_str = cfg.user.sudo_commands.join(", ");
        cmds.push(format!(
            "echo '{uname} ALL=(ALL) NOPASSWD: {cmds_str}' > /target/etc/sudoers.d/cmds-{uname}"
        ));
        cmds.push(format!("chmod 440 /target/etc/sudoers.d/cmds-{uname}"));
    }

    // 4. Proxy
    // /etc/environment is distro-agnostic; APT proxy config is Ubuntu-only.
    let is_ubuntu = !matches!(cfg.distro, Some(Distro::Fedora | Distro::Arch));
    if cfg.proxy.http_proxy.is_some() || cfg.proxy.https_proxy.is_some() {
        if let Some(hp) = &cfg.proxy.http_proxy {
            cmds.push(format!(
                "echo 'http_proxy=\"{hp}\"' >> /target/etc/environment"
            ));
            if is_ubuntu {
                // Use \\n (Rust: backslash-n) so the shell command contains the
                // two-character sequence \n, which printf interprets as a newline.
                // Using \n (Rust: actual newline) would embed a literal newline
                // in the command string, breaking Mint preseed late_command lines.
                cmds.push(format!(
                    "printf 'Acquire::http::Proxy \"{hp}\";\\n' > /target/etc/apt/apt.conf.d/99proxy"
                ));
            }
        }
        if let Some(sp) = &cfg.proxy.https_proxy {
            cmds.push(format!(
                "echo 'https_proxy=\"{sp}\"' >> /target/etc/environment"
            ));
            if is_ubuntu {
                cmds.push(format!(
                    "printf 'Acquire::https::Proxy \"{sp}\";\\n' >> /target/etc/apt/apt.conf.d/99proxy"
                ));
            }
        }
    }
    // no_proxy goes to /etc/environment regardless of whether http/https proxy is set.
    if !cfg.proxy.no_proxy.is_empty() {
        let np = cfg.proxy.no_proxy.join(",");
        cmds.push(format!(
            "echo 'no_proxy=\"{np}\"' >> /target/etc/environment"
        ));
    }

    // 5. Enable/disable services
    for svc in &cfg.enable_services {
        cmds.push(format!("chroot /target systemctl enable {svc}"));
    }
    for svc in &cfg.disable_services {
        cmds.push(format!("chroot /target systemctl disable {svc}"));
    }

    // 6. sysctl
    if !cfg.sysctl.is_empty() {
        for (key, val) in &cfg.sysctl {
            cmds.push(format!(
                "echo '{key}={val}' >> /target/etc/sysctl.d/99-forgeiso.conf"
            ));
        }
        cmds.push("chroot /target sysctl -p /etc/sysctl.d/99-forgeiso.conf".to_string());
    }

    // 7. Swap
    if let Some(swap) = &cfg.swap {
        let fname = swap.filename.as_deref().unwrap_or("/swapfile");
        let mb = swap.size_mb;
        cmds.push(format!("fallocate -l {mb}M /target{fname}"));
        cmds.push(format!("chmod 600 /target{fname}"));
        cmds.push(format!("chroot /target mkswap {fname}"));
        cmds.push(format!(
            "echo '{fname} none swap defaults 0 0' >> /target/etc/fstab"
        ));
        if let Some(swappiness) = swap.swappiness {
            cmds.push(format!(
                "echo 'vm.swappiness={swappiness}' >> /target/etc/sysctl.d/99-swap.conf"
            ));
        }
    }

    // 8. Firewall.
    //    Ubuntu/Mint → UFW.
    //    Fedora → firewalld (firewall-cmd).  Commands are written with the
    //    "chroot /target" prefix so they work in the cloud-init context; the
    //    kickstart.rs %post transformer strips that prefix for Kickstart files.
    let is_fedora = matches!(cfg.distro, Some(Distro::Fedora));
    if cfg.firewall.enabled && is_ubuntu {
        if let Some(policy) = &cfg.firewall.default_policy {
            cmds.push(format!("chroot /target ufw default {policy} incoming"));
        }
        for port in &cfg.firewall.allow_ports {
            cmds.push(format!("chroot /target ufw allow {port}"));
        }
        for port in &cfg.firewall.deny_ports {
            cmds.push(format!("chroot /target ufw deny {port}"));
        }
        cmds.push("chroot /target ufw --force enable".to_string());
        cmds.push("chroot /target systemctl enable ufw".to_string());
    } else if cfg.firewall.enabled && is_fedora {
        // firewalld is already in the package list (added by kickstart.rs).
        // Set the default zone policy, then open/block individual ports.
        if let Some(policy) = &cfg.firewall.default_policy {
            // firewalld uses "ACCEPT"/"DROP"/"REJECT"; map common UFW-style words.
            let fw_policy = match policy.to_lowercase().as_str() {
                "deny" | "drop" => "DROP",
                "reject" => "REJECT",
                _ => "ACCEPT",
            };
            cmds.push(format!(
                "chroot /target firewall-cmd --permanent --set-target={fw_policy} --zone=public"
            ));
        }
        for port in &cfg.firewall.allow_ports {
            cmds.push(format!(
                "chroot /target firewall-cmd --permanent --add-port={port} --zone=public"
            ));
        }
        for port in &cfg.firewall.deny_ports {
            // firewalld has no "deny port" equivalent — remove the port from
            // the allow list (no-op if not present) as the closest approximation.
            cmds.push(format!(
                "chroot /target firewall-cmd --permanent --remove-port={port} --zone=public 2>/dev/null || true"
            ));
        }
        cmds.push("chroot /target firewall-cmd --reload 2>/dev/null || true".to_string());
        cmds.push("chroot /target systemctl enable firewalld".to_string());
    }

    // 9. APT repos — Ubuntu/Debian only.
    if is_ubuntu {
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

    // 9b. Pacman repos + mirror — Arch Linux only.
    let is_arch = matches!(cfg.distro, Some(Distro::Arch));
    if is_arch {
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

    // 10. Docker — Ubuntu apt-based install only; Fedora adds docker-ce via dnf separately.
    if cfg.containers.docker && is_ubuntu {
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

    // 11. GRUB
    let grub_changed = cfg.grub.timeout.is_some()
        || !cfg.grub.cmdline_extra.is_empty()
        || cfg.grub.default_entry.is_some();
    if grub_changed {
        if let Some(t) = cfg.grub.timeout {
            cmds.push(format!(
                r"sed -i 's|^GRUB_TIMEOUT=.*|GRUB_TIMEOUT={t}|' /target/etc/default/grub"
            ));
        }
        if let Some(entry) = &cfg.grub.default_entry {
            cmds.push(format!(
                r"sed -i 's|^GRUB_DEFAULT=.*|GRUB_DEFAULT={entry}|' /target/etc/default/grub"
            ));
        }
        for param in &cfg.grub.cmdline_extra {
            // Use | as sed delimiter so params containing / (e.g. UUID paths) are safe.
            cmds.push(format!(
                r#"sed -i 's|\(GRUB_CMDLINE_LINUX_DEFAULT=".*\)"|\1 {param}"|' /target/etc/default/grub"#
            ));
        }
        // Fedora uses grub2-mkconfig; Ubuntu/Mint use the update-grub wrapper.
        if is_fedora {
            cmds.push("chroot /target grub2-mkconfig -o /boot/grub2/grub.cfg".to_string());
        } else {
            cmds.push("chroot /target update-grub".to_string());
        }
    }

    // 12. Custom mounts (fstab entries)
    // Each entry is an fstab line: "<device> <mountpoint> <type> <options> <dump> <pass>"
    // We mkdir the mountpoint so the system doesn't fail to mount on first boot.
    // If the entry has no whitespace-separated second field we skip the mkdir but still
    // write the fstab line — the admin may be using a bind-mount or special syntax.
    for entry in &cfg.mounts {
        let parts: Vec<&str> = entry.splitn(2, ' ').collect();
        if parts.len() >= 2 {
            let mountpoint = parts[1].split_whitespace().next();
            if let Some(mp) = mountpoint {
                cmds.push(format!("mkdir -p /target{mp}"));
            }
            // If no mountpoint is present, skip mkdir (malformed fstab line);
            // still write the line so the user sees it at runtime and can diagnose.
        }
        cmds.push(format!("echo '{entry}' >> /target/etc/fstab"));
    }

    // 13. Run commands
    cmds.extend(cfg.run_commands.iter().cloned());

    // 14. Extra late commands
    cmds.extend(cfg.extra_late_commands.clone());

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
