//! Generates a preseed.cfg for Linux Mint (Calamares-based live installer).
//!
//! Calamares, used by Linux Mint, supports Debian preseed files for unattended
//! installation when launched with `auto=true priority=critical preseed/file=/cdrom/preseed.cfg`.
//! This is significantly more limited than Ubuntu's cloud-init autoinstall, but
//! allows basic hands-free installation for locale, timezone, user, and disk.
//!
//! Features that cannot be expressed natively in a Calamares preseed (proxy,
//! services, sysctl, swap, apt repos, firewall, docker) are emitted as a
//! `preseed/late_command` shell script using the same late-command logic as the
//! Ubuntu cloud-init path.  The commands use `/target` as the chroot root,
//! which is the Debian installer convention and is honoured by Calamares when
//! it runs late commands.

use crate::autoinstall::build_feature_late_commands;
use crate::config::InjectConfig;
use crate::error::EngineResult;
use crate::kickstart::parse_cidr;

/// Generate a preseed.cfg for Calamares-based Linux Mint unattended install.
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

    // Static IP — if static_ip is provided, configure Debian-style static network.
    // The CIDR mask is converted to dotted-decimal for preseed.
    if let Some(static_ip) = &cfg.static_ip {
        let (ip, mask) = parse_cidr(static_ip);
        lines.push("d-i netcfg/disable_autoconfig boolean true".to_string());
        lines.push(format!("d-i netcfg/get_ipaddress string {ip}"));
        lines.push(format!("d-i netcfg/get_netmask string {mask}"));
        if let Some(gw) = &cfg.gateway {
            lines.push(format!("d-i netcfg/get_gateway string {gw}"));
        }
        if !cfg.network.dns_servers.is_empty() {
            let dns = cfg.network.dns_servers.join(" ");
            lines.push(format!("d-i netcfg/get_nameservers string {dns}"));
        }
        lines.push("d-i netcfg/confirm_static boolean true".to_string());
    } else if !cfg.network.dns_servers.is_empty() {
        // DHCP but with custom DNS nameservers.
        let dns = cfg.network.dns_servers.join(" ");
        lines.push(format!("d-i netcfg/get_nameservers string {dns}"));
    }

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
        // Extract host and directory from the mirror URL.
        // Expected format: http://mirror.example.com/path
        if let Some(stripped) = mirror.strip_prefix("http://") {
            let parts: Vec<&str> = stripped.splitn(2, '/').collect();
            if parts.len() == 2 {
                lines.push(format!("d-i mirror/http/hostname string {}", parts[0]));
                lines.push(format!("d-i mirror/http/directory string /{}", parts[1]));
            }
        }
    } else {
        // Default to Mint's own package mirror.
        lines.push("d-i mirror/country string manual".to_string());
        lines.push("d-i mirror/http/hostname string packages.linuxmint.com".to_string());
        lines.push("d-i mirror/http/directory string /mint".to_string());
    }
    lines.push("d-i mirror/http/proxy string".to_string());

    // ── Boot loader ───────────────────────────────────────────────────────────
    lines.push("d-i grub-installer/only_debian boolean true".to_string());
    lines.push("d-i grub-installer/bootdev string default".to_string());

    // ── Late commands (proxy, services, sysctl, swap, APT repos, firewall) ───
    // Features that cannot be expressed as preseed directives are emitted as a
    // shell script in preseed/late_command using the same /target-rooted
    // late-command logic as the Ubuntu cloud-init path.
    let late_cmds = build_feature_late_commands(cfg)?;
    if !late_cmds.is_empty() {
        // Join all commands with "; " for a single late_command string.
        // Each command already uses the /target prefix from build_feature_late_commands.
        let joined = late_cmds.join("; ");
        lines.push(format!("d-i preseed/late_command string {joined}"));
    }

    // ── Finish ────────────────────────────────────────────────────────────────
    lines.push("d-i finish-install/reboot_in_progress note".to_string());

    Ok(lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FirewallConfig, InjectConfig, NetworkConfig, ProxyConfig, UserConfig};

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
            user: UserConfig {
                groups: vec!["docker".into(), "libvirt".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let preseed = generate_mint_preseed(&cfg).unwrap();
        assert!(preseed.contains("docker"));
        assert!(preseed.contains("libvirt"));
    }

    #[test]
    fn default_mirror_is_mint_not_ubuntu() {
        let cfg = InjectConfig::default();
        let preseed = generate_mint_preseed(&cfg).unwrap();
        assert!(
            preseed.contains("packages.linuxmint.com"),
            "default mirror must be Mint's own mirror, not Ubuntu's"
        );
        assert!(
            !preseed.contains("archive.ubuntu.com"),
            "Ubuntu mirror must not appear when no apt_mirror is set for Mint"
        );
    }

    #[test]
    fn custom_apt_mirror_overrides_default() {
        let cfg = InjectConfig {
            apt_mirror: Some("http://mirror.example.com/ubuntu".into()),
            ..Default::default()
        };
        let preseed = generate_mint_preseed(&cfg).unwrap();
        assert!(preseed.contains("mirror.example.com"));
    }

    #[test]
    fn proxy_settings_appear_in_late_command() {
        let cfg = InjectConfig {
            proxy: ProxyConfig {
                http_proxy: Some("http://proxy.corp:3128".into()),
                https_proxy: None,
                no_proxy: vec![],
            },
            ..Default::default()
        };
        let preseed = generate_mint_preseed(&cfg).unwrap();
        assert!(
            preseed.contains("late_command"),
            "late_command must be emitted when proxy is set"
        );
        assert!(
            preseed.contains("http_proxy"),
            "http_proxy must appear in late_command"
        );
    }

    #[test]
    fn services_appear_in_late_command() {
        let cfg = InjectConfig {
            enable_services: vec!["docker".into()],
            disable_services: vec!["snapd".into()],
            ..Default::default()
        };
        let preseed = generate_mint_preseed(&cfg).unwrap();
        assert!(
            preseed.contains("late_command"),
            "late_command must be emitted for services"
        );
        assert!(preseed.contains("docker"), "enable_services must appear");
        assert!(preseed.contains("snapd"), "disable_services must appear");
    }

    #[test]
    fn firewall_appears_in_late_command() {
        let cfg = InjectConfig {
            firewall: FirewallConfig {
                enabled: true,
                default_policy: Some("deny".into()),
                allow_ports: vec!["22".into()],
                deny_ports: vec![],
            },
            ..Default::default()
        };
        let preseed = generate_mint_preseed(&cfg).unwrap();
        assert!(
            preseed.contains("ufw"),
            "ufw commands must appear in late_command for Mint (Debian-based)"
        );
    }

    #[test]
    fn ntp_first_server_in_preseed_directive() {
        let cfg = InjectConfig {
            network: NetworkConfig {
                ntp_servers: vec!["time.cloudflare.com".into(), "time.google.com".into()],
                dns_servers: vec![],
            },
            ..Default::default()
        };
        let preseed = generate_mint_preseed(&cfg).unwrap();
        assert!(
            preseed.contains("clock-setup/ntp-server string time.cloudflare.com"),
            "first NTP server must appear in ntp-server directive"
        );
    }

    #[test]
    fn no_plaintext_password_in_preseed() {
        let cfg = InjectConfig {
            password: Some("SuperSecret123!".into()),
            ..Default::default()
        };
        let preseed = generate_mint_preseed(&cfg).unwrap();
        assert!(
            !preseed.contains("SuperSecret123!"),
            "plaintext password must never appear in preseed"
        );
        assert!(
            preseed.contains("$6$"),
            "password must be SHA-512-crypt hashed"
        );
    }
}
