//! Smoke tests for the curated [`super::ProfileKind`] catalog.
//!
//! Every profile, when applied to an empty [`InjectConfig`] for every
//! distro family we care about, MUST produce a config that passes
//! [`InjectConfig::validate`]. These are kill-tests for the populate
//! signature: a regression in any field name, package name, or sysctl
//! key trips a panic at the call site.

use super::{Profile, ProfileCatalog, ProfileKind};
use crate::config::InjectConfig;

/// Distros that every profile must be safe to populate against. Mix of
/// preset ids and family shorthands so both code paths in
/// [`super::DistroFamily::classify`] are exercised.
const DISTROS: &[&str] = &[
    "ubuntu",
    "ubuntu-server-lts",
    "ubuntu-server-jammy",
    "ubuntu-desktop-lts",
    "mint",
    "linux-mint-cinnamon",
    "fedora",
    "fedora-server",
    "fedora-workstation",
    "rhel-family",
    "rocky-linux",
    "almalinux",
    "centos-stream",
    "arch",
    "arch-linux",
    "endeavouros",
    "manjaro",
    "debian",
    "opensuse",
    "opensuse-leap",
    "popos",
    "pop-os-22-intel",
    "kali-linux",
    // Unknown distro -- must fall back to apt path without panicking.
    "totally-made-up-distro",
];

#[test]
fn every_profile_validates_for_every_distro() {
    let mut failures: Vec<String> = Vec::new();
    for kind in ProfileKind::all() {
        for distro in DISTROS {
            let cfg = Profile::new(*kind).populate(distro, InjectConfig::default());
            if let Err(e) = cfg.validate() {
                failures.push(format!(
                    "{} on {distro:?} failed validate(): {e}",
                    kind.as_str()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} profile/distro combos failed validate():\n{}",
        failures.len(),
        failures.join("\n"),
    );
}

#[test]
fn server_default_sets_baseline_fields() {
    let cfg = Profile::new(ProfileKind::ServerDefault).populate("ubuntu", InjectConfig::default());
    assert!(cfg.firewall.enabled, "firewall must be on");
    assert!(cfg.extra_packages.iter().any(|p| p == "vim"));
    assert!(cfg.extra_packages.iter().any(|p| p == "git"));
    assert_eq!(cfg.timezone.as_deref(), Some("UTC"));
    assert!(cfg.user.groups.iter().any(|g| g == "sudo"));
}

#[test]
fn server_default_uses_wheel_on_rhel() {
    let cfg =
        Profile::new(ProfileKind::ServerDefault).populate("rocky-linux", InjectConfig::default());
    assert!(cfg.user.groups.iter().any(|g| g == "wheel"));
    assert!(
        cfg.extra_packages.iter().any(|p| p == "epel-release"),
        "RHEL family must add epel-release"
    );
}

#[test]
fn server_hardened_disables_password_auth() {
    let cfg = Profile::new(ProfileKind::ServerHardened)
        .populate("fedora-server", InjectConfig::default());
    assert_eq!(cfg.ssh.allow_password_auth, Some(false));
    assert!(!cfg.containers.docker, "hardened must not enable docker");
    assert!(cfg.firewall.enabled);
    assert_eq!(cfg.firewall.default_policy.as_deref(), Some("deny"));
    assert!(cfg
        .sysctl
        .iter()
        .any(|(k, _)| k == "net.ipv4.tcp_syncookies"));
    assert!(
        cfg.enable_services.iter().any(|s| s == "auditd"),
        "auditd should be enabled"
    );
}

#[test]
fn server_hardened_includes_audit_packages() {
    let apt = Profile::new(ProfileKind::ServerHardened)
        .populate("ubuntu-server-lts", InjectConfig::default());
    assert!(apt.extra_packages.iter().any(|p| p == "auditd"));
    assert!(apt.extra_packages.iter().any(|p| p == "fail2ban"));
    assert!(apt.extra_packages.iter().any(|p| p == "apparmor"));
    let dnf = Profile::new(ProfileKind::ServerHardened)
        .populate("fedora-server", InjectConfig::default());
    assert!(dnf.extra_packages.iter().any(|p| p == "audit"));
    assert!(dnf
        .extra_packages
        .iter()
        .any(|p| p == "selinux-policy-targeted"));
}

#[test]
fn desktop_developer_enables_docker() {
    let cfg =
        Profile::new(ProfileKind::DesktopDeveloper).populate("arch-linux", InjectConfig::default());
    assert!(cfg.containers.docker);
    assert!(cfg.extra_packages.iter().any(|p| p == "git"));
    assert!(cfg.user.groups.iter().any(|g| g == "docker"));
    assert!(
        cfg.user.sudo_nopasswd,
        "developer profile gets passwordless sudo"
    );
}

#[test]
fn desktop_developer_propagates_username_to_docker_users() {
    let base = InjectConfig {
        username: Some("alice".to_string()),
        ..Default::default()
    };
    let cfg = Profile::new(ProfileKind::DesktopDeveloper).populate("ubuntu", base);
    assert!(cfg.containers.docker_users.iter().any(|u| u == "alice"));
}

#[test]
fn kiosk_writes_autologin_run_commands() {
    let base = InjectConfig {
        username: Some("kiosk".to_string()),
        ..Default::default()
    };
    let cfg = Profile::new(ProfileKind::Kiosk).populate("ubuntu", base);
    assert!(
        cfg.run_commands.iter().any(|c| c.contains("autologin")),
        "kiosk must include an autologin override"
    );
    assert!(cfg.firewall.enabled);
    // Only port 22 should be open.
    assert_eq!(cfg.firewall.allow_ports, vec!["22".to_string()]);
}

#[test]
fn kiosk_default_username_is_kiosk_when_unset() {
    let cfg = Profile::new(ProfileKind::Kiosk).populate("ubuntu", InjectConfig::default());
    assert!(cfg.run_commands.iter().any(|c| c.contains("kiosk")));
}

#[test]
fn minimal_cloud_keeps_password_auth_on() {
    let cfg = Profile::new(ProfileKind::MinimalCloud).populate("ubuntu", InjectConfig::default());
    assert_eq!(cfg.ssh.allow_password_auth, Some(true));
    assert!(
        !cfg.firewall.enabled,
        "cloud relies on the cloud net firewall"
    );
    assert!(cfg.extra_packages.iter().any(|p| p == "cloud-init"));
    assert!(cfg.enable_services.iter().any(|s| s == "cloud-init"));
}

#[test]
fn recommended_for_ubuntu_server_is_server_default() {
    let r = ProfileCatalog::recommended_for("ubuntu-server-lts");
    assert_eq!(r.first(), Some(&ProfileKind::ServerDefault));
    let r = ProfileCatalog::recommended_for("ubuntu-server-jammy");
    assert_eq!(r.first(), Some(&ProfileKind::ServerDefault));
}

#[test]
fn recommended_for_fedora_and_rhel_is_hardened() {
    assert_eq!(
        ProfileCatalog::recommended_for("fedora-server").first(),
        Some(&ProfileKind::ServerHardened)
    );
    for d in ["rocky-linux", "almalinux", "centos-stream"] {
        assert_eq!(
            ProfileCatalog::recommended_for(d).first(),
            Some(&ProfileKind::ServerHardened),
            "{d} should recommend ServerHardened first"
        );
    }
}

#[test]
fn recommended_for_arch_and_mint_is_developer() {
    assert_eq!(
        ProfileCatalog::recommended_for("arch-linux").first(),
        Some(&ProfileKind::DesktopDeveloper)
    );
    assert_eq!(
        ProfileCatalog::recommended_for("linux-mint-cinnamon").first(),
        Some(&ProfileKind::DesktopDeveloper)
    );
}

#[test]
fn recommended_for_unknown_falls_back_to_server_default() {
    let r = ProfileCatalog::recommended_for("not-a-real-distro");
    assert_eq!(r, vec![ProfileKind::ServerDefault]);
}

#[test]
fn recommended_for_is_case_insensitive_and_trims() {
    let r = ProfileCatalog::recommended_for("  Ubuntu-Server-Lts  ");
    assert_eq!(r.first(), Some(&ProfileKind::ServerDefault));
}

#[test]
fn profile_kind_as_str_round_trip_is_unique() {
    use std::collections::HashSet;
    let names: HashSet<&'static str> = ProfileKind::all().iter().map(|k| k.as_str()).collect();
    assert_eq!(names.len(), ProfileKind::all().len());
}

#[test]
fn profile_kind_descriptions_are_non_empty() {
    for k in ProfileKind::all() {
        assert!(
            !k.description().is_empty(),
            "{} missing description",
            k.as_str()
        );
    }
}

#[test]
fn profile_preserves_existing_fields() {
    // If the user already set a hostname / username, populate() must not
    // overwrite it.
    let base = InjectConfig {
        hostname: Some("preset-host".to_string()),
        username: Some("operator".to_string()),
        ..Default::default()
    };
    let cfg = Profile::new(ProfileKind::ServerDefault).populate("ubuntu", base);
    assert_eq!(cfg.hostname.as_deref(), Some("preset-host"));
    assert_eq!(cfg.username.as_deref(), Some("operator"));
}

#[test]
fn populate_is_idempotent() {
    let cfg1 = Profile::new(ProfileKind::ServerDefault).populate("ubuntu", InjectConfig::default());
    let cfg2 = Profile::new(ProfileKind::ServerDefault).populate("ubuntu", cfg1.clone());
    assert_eq!(cfg1.extra_packages, cfg2.extra_packages);
    assert_eq!(cfg1.enable_services, cfg2.enable_services);
    assert_eq!(cfg1.user.groups, cfg2.user.groups);
}
