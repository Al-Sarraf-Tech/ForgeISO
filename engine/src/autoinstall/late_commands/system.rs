use crate::config::{Distro, InjectConfig};

/// Section 4 — HTTP/HTTPS/no_proxy environment + APT proxy config.
pub(super) fn append_proxy(cfg: &InjectConfig, cmds: &mut Vec<String>) {
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
}

/// Section 5 — `systemctl enable/disable` for explicit services.
pub(super) fn append_services(cfg: &InjectConfig, cmds: &mut Vec<String>) {
    for svc in &cfg.enable_services {
        cmds.push(format!("chroot /target systemctl enable {svc}"));
    }
    for svc in &cfg.disable_services {
        cmds.push(format!("chroot /target systemctl disable {svc}"));
    }
}

/// Section 6 — sysctl key/value pairs and `sysctl -p`.
pub(super) fn append_sysctl(cfg: &InjectConfig, cmds: &mut Vec<String>) {
    if !cfg.sysctl.is_empty() {
        for (key, val) in &cfg.sysctl {
            cmds.push(format!(
                "echo '{key}={val}' >> /target/etc/sysctl.d/99-forgeiso.conf"
            ));
        }
        cmds.push("chroot /target sysctl -p /etc/sysctl.d/99-forgeiso.conf".to_string());
    }
}

/// Section 7 — swap file allocation, mkswap, fstab line, and swappiness tuning.
pub(super) fn append_swap(cfg: &InjectConfig, cmds: &mut Vec<String>) {
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
}

/// Section 8 — Firewall (UFW for Ubuntu/Mint, firewalld for Fedora).
/// Commands are emitted with the `chroot /target` prefix so they work in the
/// cloud-init context; the kickstart `%post` transformer strips that prefix.
pub(super) fn append_firewall(cfg: &InjectConfig, cmds: &mut Vec<String>) {
    let is_ubuntu = !matches!(cfg.distro, Some(Distro::Fedora | Distro::Arch));
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
}
