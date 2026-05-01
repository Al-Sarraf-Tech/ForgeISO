use crate::config::{Distro, InjectConfig};

/// Section 1 — NTP servers (push timesyncd config + enable service).
pub(super) fn append_ntp(cfg: &InjectConfig, cmds: &mut Vec<String>) {
    if !cfg.network.ntp_servers.is_empty() {
        let ntp_list = cfg.network.ntp_servers.join(" ");
        cmds.push(format!(
            "printf '[Time]\\nNTP={ntp_list}\\n' > /target/etc/systemd/timesyncd.conf"
        ));
        cmds.push("chroot /target systemctl enable systemd-timesyncd".to_string());
    }
}

/// Section 2 — Wallpaper (copy asset and configure dconf default).
pub(super) fn append_wallpaper(cfg: &InjectConfig, cmds: &mut Vec<String>) {
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
}

/// Section 3a — Mint-only SSH `authorized_keys` injection via late_command.
/// Ubuntu handles SSH keys in cloud-init YAML, Fedora via the `sshkey`
/// Kickstart directive, and Arch via the archinstall `!users` list.
pub(super) fn append_ssh_keys_mint(cfg: &InjectConfig, cmds: &mut Vec<String>) {
    let is_mint = matches!(cfg.distro, Some(Distro::Mint));
    if !is_mint || cfg.ssh.authorized_keys.is_empty() {
        return;
    }
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

/// Section 3b — User groups, login shell, and sudo configuration.
pub(super) fn append_user_groups_shell_sudo(cfg: &InjectConfig, cmds: &mut Vec<String>) {
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
}
