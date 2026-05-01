use crate::config::{Distro, InjectConfig};

/// Section 11 — GRUB timeout, default entry, and cmdline tweaks.
pub(super) fn append_grub(cfg: &InjectConfig, cmds: &mut Vec<String>) {
    let is_fedora = matches!(cfg.distro, Some(Distro::Fedora));
    let grub_changed = cfg.grub.timeout.is_some()
        || !cfg.grub.cmdline_extra.is_empty()
        || cfg.grub.default_entry.is_some();
    if !grub_changed {
        return;
    }
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

/// Section 12 — Custom mounts (fstab entries).
///
/// Each entry is an fstab line: `<device> <mountpoint> <type> <options> <dump> <pass>`.
/// We `mkdir -p` the mountpoint so the system doesn't fail to mount on first
/// boot.  If the entry has no whitespace-separated second field we skip the
/// mkdir but still write the fstab line — the admin may be using a bind-mount
/// or special syntax.
pub(super) fn append_mounts(cfg: &InjectConfig, cmds: &mut Vec<String>) {
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
}

/// Sections 13 + 14 — User-supplied `run_commands` and `extra_late_commands`.
///
/// These pass through verbatim so callers control their own paths (including
/// any literal `/target/` substrings).  Kickstart `%post` rewriting MUST treat
/// the trailing `user_cmd_count` entries as untouchable.
pub(super) fn append_user_commands(cfg: &InjectConfig, cmds: &mut Vec<String>) {
    cmds.extend(cfg.run_commands.iter().cloned());
    cmds.extend(cfg.extra_late_commands.clone());
}
