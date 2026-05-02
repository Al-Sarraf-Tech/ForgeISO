//! [`ProfileKind::Kiosk`](super::ProfileKind::Kiosk) preset.
//!
//! Single-application appliance: auto-login, autostart browser/app,
//! screen blanking disabled, minimal package set. The autostart and
//! display-power command lines are emitted via `run_commands` so they
//! remain editable after install.

use crate::config::{FirewallConfig, InjectConfig, SshConfig, UserConfig};

use super::DistroFamily;

/// Build a [`Kiosk`](super::ProfileKind::Kiosk) config on top of `base`.
pub(super) fn populate(distro: &str, mut cfg: InjectConfig) -> InjectConfig {
    let family = DistroFamily::classify(distro);

    cfg.timezone.get_or_insert_with(|| "UTC".to_string());
    cfg.locale.get_or_insert_with(|| "en_US.UTF-8".to_string());
    cfg.keyboard_layout.get_or_insert_with(|| "us".to_string());

    // Kiosks need to be reachable for remote management but typically
    // only via key auth. Leave password auth alone if the user already
    // set it; default to off.
    cfg.ssh = SshConfig {
        authorized_keys: cfg.ssh.authorized_keys,
        allow_password_auth: cfg.ssh.allow_password_auth.or(Some(false)),
        install_server: Some(true),
    };

    // Firewall on, only SSH allowed (the kiosk talks out, not in).
    cfg.firewall = FirewallConfig {
        enabled: true,
        default_policy: Some("deny".to_string()),
        allow_ports: vec!["22".to_string()],
        deny_ports: cfg.firewall.deny_ports,
    };

    // Sudo group only — no extra perms for the kiosk operator.
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

    // Minimal kiosk packages: a browser plus the X stack to drive it.
    for pkg in kiosk_packages(family) {
        if !cfg.extra_packages.iter().any(|p| p == pkg) {
            cfg.extra_packages.push((*pkg).to_string());
        }
    }

    // Auto-login + autostart Chromium in kiosk mode + disable DPMS so the
    // screen never blanks. These are emitted as run_commands so they
    // execute on first boot of the installed system.
    let username = cfg.username.clone().unwrap_or_else(|| "kiosk".to_string());

    let kiosk_cmds = [
        // Disable screen blank / DPMS.
        "systemctl set-default graphical.target".to_string(),
        // Stop power-management blanking.
        "mkdir -p /etc/X11/xorg.conf.d".to_string(),
        "printf 'Section \"ServerFlags\"\\n  Option \"BlankTime\" \"0\"\\n  Option \"StandbyTime\" \"0\"\\n  Option \"SuspendTime\" \"0\"\\n  Option \"OffTime\" \"0\"\\nEndSection\\n' > /etc/X11/xorg.conf.d/10-kiosk-noblank.conf".to_string(),
        // Enable getty auto-login for the kiosk user.
        "mkdir -p /etc/systemd/system/getty@tty1.service.d".to_string(),
        format!(
            "printf '[Service]\\nExecStart=\\nExecStart=-/sbin/agetty --autologin {username} --noclear %%I $TERM\\n' > /etc/systemd/system/getty@tty1.service.d/override.conf"
        ),
        // Bash profile launches X with chromium kiosk.
        format!(
            "printf 'if [ -z \"$DISPLAY\" ] && [ \"$(tty)\" = \"/dev/tty1\" ]; then exec startx /usr/bin/chromium --kiosk --noerrdialogs --disable-infobars https://example.com; fi\\n' >> /home/{username}/.bash_profile"
        ),
    ];
    for cmd in kiosk_cmds {
        if !cfg.run_commands.iter().any(|c| c == &cmd) {
            cfg.run_commands.push(cmd);
        }
    }

    // Enable ssh + chrony + the graphical target.
    let svc_names: &[&str] = match family {
        DistroFamily::Apt => &["ssh", "chrony"],
        _ => &["sshd", "chronyd"],
    };
    for svc in svc_names {
        if !cfg.enable_services.iter().any(|s| s == *svc) {
            cfg.enable_services.push((*svc).to_string());
        }
    }

    cfg
}

fn kiosk_packages(family: DistroFamily) -> &'static [&'static str] {
    match family {
        DistroFamily::Apt => &[
            "xserver-xorg",
            "xinit",
            "openbox",
            "chromium-browser",
            "ca-certificates",
            "chrony",
        ],
        DistroFamily::Dnf | DistroFamily::DnfWithEpel => &[
            "xorg-x11-server-Xorg",
            "xorg-x11-xinit",
            "openbox",
            "chromium",
            "chrony",
        ],
        DistroFamily::Pacman => &["xorg-server", "xorg-xinit", "openbox", "chromium", "chrony"],
        DistroFamily::Zypper => &["xorg-x11-server", "xinit", "openbox", "chromium", "chrony"],
    }
}
