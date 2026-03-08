//! Generates a preseed.cfg for Linux Mint (Calamares-based live installer).
//!
//! Calamares, used by Linux Mint, supports Debian preseed files for unattended
//! installation when launched with `auto=true priority=critical preseed/file=/cdrom/preseed.cfg`.
//! This is significantly more limited than Ubuntu's cloud-init autoinstall, but
//! allows basic hands-free installation for locale, timezone, user, and disk.

use crate::config::InjectConfig;
use crate::error::EngineResult;

/// Generate a minimal preseed.cfg for Calamares-based Linux Mint unattended install.
///
/// The generated file should be placed at the ISO root so that the installer
/// can access it at `/cdrom/preseed.cfg` during boot.
pub fn generate_mint_preseed(cfg: &InjectConfig) -> EngineResult<String> {
    let locale = cfg.locale.as_deref().unwrap_or("en_US.UTF-8");
    let keyboard = cfg.keyboard_layout.as_deref().unwrap_or("us");
    let timezone = cfg.timezone.as_deref().unwrap_or("UTC");
    let hostname = cfg.hostname.as_deref().unwrap_or("mint-desktop");
    let username = cfg.username.as_deref().unwrap_or("user");

    // Hash the password — SHA-512-crypt is required for user-setup preseed.
    // Calamares reads user-setup/user-password-crypted from preseed.
    let password_hash = if let Some(pw) = &cfg.password {
        crate::autoinstall::hash_password(pw)?
    } else {
        // No password supplied — use a placeholder that locks the account.
        // The user should set a real password post-install.
        "*".to_string()
    };

    let realname = cfg.realname.as_deref().unwrap_or(username);

    // Partition method: always use guided-entire-disk for now.
    // NOTE: preseed partman is the standard Debian approach but Calamares has
    // limited preseed support. We use the simplest possible partitioning stanza.

    let mut lines: Vec<String> = Vec::new();

    // ── Locale ────────────────────────────────────────────────────────────────
    lines.push(format!("d-i debian-installer/locale string {locale}"));
    lines.push(format!(
        "d-i localechooser/supported-locales multiselect {locale}"
    ));

    // ── Keyboard ──────────────────────────────────────────────────────────────
    lines.push("d-i console-setup/ask_detect boolean false".to_string());
    lines.push(format!(
        "d-i keyboard-configuration/xkb-keymap select {keyboard}"
    ));
    lines.push(format!(
        "d-i keyboard-configuration/layoutcode string {keyboard}"
    ));

    // ── Network ───────────────────────────────────────────────────────────────
    lines.push("d-i netcfg/choose_interface select auto".to_string());
    lines.push(format!("d-i netcfg/get_hostname string {hostname}"));
    lines.push("d-i netcfg/get_domain string".to_string());
    lines.push("d-i netcfg/wireless_wep string".to_string());

    // ── Clock and timezone ────────────────────────────────────────────────────
    lines.push("d-i clock-setup/utc boolean true".to_string());
    lines.push(format!("d-i time/zone string {timezone}"));
    lines.push("d-i clock-setup/ntp boolean true".to_string());

    if let Some(ntp) = cfg.network.ntp_servers.first() {
        lines.push(format!("d-i clock-setup/ntp-server string {ntp}"));
    }

    // ── Partitioning ──────────────────────────────────────────────────────────
    // Use guided partitioning on the largest available disk (sda or nvme0n1).
    lines.push("d-i partman-auto/method string regular".to_string());
    lines.push("d-i partman-auto/choose_recipe select atomic".to_string());
    lines.push("d-i partman-lvm/device_remove_lvm boolean true".to_string());
    lines.push("d-i partman-md/device_remove_md boolean true".to_string());
    lines.push("d-i partman-lvm/confirm boolean true".to_string());
    lines.push("d-i partman-lvm/confirm_nooverwrite boolean true".to_string());
    lines.push("d-i partman/default_filesystem string ext4".to_string());
    lines.push("d-i partman/confirm_write_new_label boolean true".to_string());
    lines.push("d-i partman/choose_partition select finish".to_string());
    lines.push("d-i partman/confirm boolean true".to_string());
    lines.push("d-i partman/confirm_nooverwrite boolean true".to_string());

    // ── User account ──────────────────────────────────────────────────────────
    lines.push(format!("d-i passwd/user-fullname string {realname}"));
    lines.push(format!("d-i passwd/username string {username}"));
    lines.push(format!(
        "d-i passwd/user-password-crypted password {password_hash}"
    ));
    lines.push(
        "d-i passwd/user-default-groups string audio cdrom dip floppy plugdev sudo users video"
            .to_string(),
    );

    // Add extra groups if specified
    if !cfg.user.groups.is_empty() {
        let groups = cfg.user.groups.join(" ");
        lines.push(format!(
            "d-i passwd/user-default-groups string {groups} audio cdrom sudo users"
        ));
    }

    // Root account disabled — use sudo instead
    lines.push("d-i passwd/root-login boolean false".to_string());

    // ── Packages ──────────────────────────────────────────────────────────────
    lines.push("d-i pkgsel/update-policy select none".to_string());
    lines.push("d-i pkgsel/upgrade select safe-upgrade".to_string());

    if !cfg.extra_packages.is_empty() {
        let pkgs = cfg.extra_packages.join(" ");
        lines.push(format!("d-i pkgsel/include string {pkgs}"));
    }

    // ── APT mirror ────────────────────────────────────────────────────────────
    if let Some(mirror) = &cfg.apt_mirror {
        // Extract host and directory from the mirror URL
        // Expected format: http://mirror.example.com/ubuntu
        if let Some(stripped) = mirror.strip_prefix("http://") {
            let parts: Vec<&str> = stripped.splitn(2, '/').collect();
            if parts.len() == 2 {
                lines.push(format!("d-i mirror/http/hostname string {}", parts[0]));
                lines.push(format!("d-i mirror/http/directory string /{}", parts[1]));
            }
        }
    } else {
        lines.push("d-i mirror/country string manual".to_string());
        lines.push("d-i mirror/http/hostname string archive.ubuntu.com".to_string());
        lines.push("d-i mirror/http/directory string /ubuntu".to_string());
    }
    lines.push("d-i mirror/http/proxy string".to_string());

    // ── Boot loader ───────────────────────────────────────────────────────────
    lines.push("d-i grub-installer/only_debian boolean true".to_string());
    lines.push("d-i grub-installer/bootdev string default".to_string());

    // ── Finish ────────────────────────────────────────────────────────────────
    lines.push("d-i finish-install/reboot_in_progress note".to_string());

    Ok(lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InjectConfig;

    #[test]
    fn generates_preseed_with_defaults() {
        let cfg = InjectConfig {
            hostname: Some("testbox".into()),
            username: Some("tester".into()),
            password: Some("secret".into()),
            timezone: Some("America/Chicago".into()),
            locale: Some("en_US.UTF-8".into()),
            keyboard_layout: Some("us".into()),
            ..Default::default()
        };
        let preseed = generate_mint_preseed(&cfg).unwrap();
        assert!(preseed.contains("testbox"), "hostname missing");
        assert!(preseed.contains("tester"), "username missing");
        assert!(preseed.contains("America/Chicago"), "timezone missing");
        assert!(preseed.contains("en_US.UTF-8"), "locale missing");
        assert!(preseed.contains("$6$"), "password not hashed");
    }

    #[test]
    fn generates_preseed_with_packages() {
        let cfg = InjectConfig {
            extra_packages: vec!["curl".into(), "git".into()],
            ..Default::default()
        };
        let preseed = generate_mint_preseed(&cfg).unwrap();
        assert!(
            preseed.contains("curl git") || preseed.contains("curl") && preseed.contains("git")
        );
    }

    #[test]
    fn generates_preseed_with_extra_groups() {
        let cfg = InjectConfig {
            user: crate::config::UserConfig {
                groups: vec!["docker".into(), "libvirt".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let preseed = generate_mint_preseed(&cfg).unwrap();
        assert!(preseed.contains("docker"));
        assert!(preseed.contains("libvirt"));
    }
}
