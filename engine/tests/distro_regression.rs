//! Comprehensive distro regression test suite.
//!
//! Each test exercises the full config-generation pipeline for a specific
//! Linux distribution family supported by ForgeISO, verifying that:
//!
//! * The correct config format is produced (cloud-init, Kickstart, preseed,
//!   archinstall-json)
//! * Mandatory sections are present
//! * Passwords are hashed — plaintext never appears in output
//! * Identity (hostname, username, realname) is present
//! * SSH key injection, package lists, and late-command features work
//! * InjectConfig::validate() accepts a fully-populated config per distro
//!
//! These are *integration* tests (live under `engine/tests/`) so they can
//! import the engine as an external crate.  No ISO download or system tools
//! are required — all assertions are against the generated text.

use forgeiso_engine::{
    autoinstall::generate_autoinstall_yaml,
    config::{
        ContainerConfig, Distro, FirewallConfig, GrubConfig, InjectConfig, IsoSource,
        NetworkConfig, ProxyConfig, SshConfig, SwapConfig, UserConfig,
    },
    kickstart::generate_kickstart_cfg,
    mint_preseed::generate_mint_preseed,
};

// ── Shared fixture helpers ────────────────────────────────────────────────────

fn base_cfg_for(distro: Option<Distro>) -> InjectConfig {
    InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "output.iso".into(),
        hostname: Some("forge-test".into()),
        username: Some("admin".into()),
        password: Some("S3cur3Pass!".into()),
        realname: Some("Forge Tester".into()),
        timezone: Some("America/New_York".into()),
        locale: Some("en_US.UTF-8".into()),
        keyboard_layout: Some("us".into()),
        ssh: SshConfig {
            authorized_keys: vec!["ssh-ed25519 AAAA…testkey".into()],
            allow_password_auth: Some(false),
            install_server: Some(true),
        },
        network: NetworkConfig {
            dns_servers: vec!["1.1.1.1".into(), "8.8.8.8".into()],
            ntp_servers: vec!["time.cloudflare.com".into()],
        },
        extra_packages: vec!["curl".into(), "git".into()],
        no_user_interaction: true,
        distro,
        ..Default::default()
    }
}

fn assert_no_plaintext_password(output: &str, label: &str) {
    assert!(
        !output.contains("S3cur3Pass!"),
        "{label}: plaintext password must not appear in generated config"
    );
}

// ── Ubuntu / cloud-init path ──────────────────────────────────────────────────

#[test]
fn ubuntu_cloud_init_structure() {
    let cfg = base_cfg_for(None); // None = Ubuntu (default)
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        yaml.starts_with("#cloud-config"),
        "must start with #cloud-config header"
    );
    assert!(
        yaml.contains("autoinstall:"),
        "must have autoinstall section"
    );
    assert!(yaml.contains("version: 1"), "must declare version");
    assert!(yaml.contains("identity:"), "must have identity section");
    assert!(yaml.contains("forge-test"), "hostname must be present");
    assert!(yaml.contains("admin"), "username must be present");
    assert!(
        yaml.contains("$6$"),
        "password must be SHA-512-crypt hashed"
    );
    assert_no_plaintext_password(&yaml, "ubuntu cloud-init");
}

#[test]
fn ubuntu_cloud_init_ssh_keys_injected() {
    let cfg = base_cfg_for(None);
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        yaml.contains("AAAA…testkey"),
        "SSH key must appear in output"
    );
    assert!(yaml.contains("ssh:"), "ssh section must be present");
}

#[test]
fn ubuntu_cloud_init_packages_included() {
    let cfg = base_cfg_for(None);
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(yaml.contains("curl"), "curl must be in packages");
    assert!(yaml.contains("git"), "git must be in packages");
}

#[test]
fn ubuntu_cloud_init_ntp_in_late_commands() {
    let cfg = base_cfg_for(None);
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        yaml.contains("timesyncd") || yaml.contains("time.cloudflare.com"),
        "NTP server must appear in late-commands"
    );
}

#[test]
fn ubuntu_cloud_init_no_interaction_section() {
    let cfg = base_cfg_for(None);
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    // no_user_interaction = true → interactive-sections must be empty list
    assert!(
        yaml.contains("interactive-sections:"),
        "interactive-sections must be set when no_user_interaction=true"
    );
}

#[test]
fn ubuntu_cloud_init_validate_accepts_full_config() {
    let cfg = base_cfg_for(None);
    assert!(
        cfg.validate().is_ok(),
        "fully-populated Ubuntu config must validate"
    );
}

#[test]
fn ubuntu_cloud_init_firewall_adds_ufw_package() {
    let cfg = InjectConfig {
        distro: None,
        firewall: FirewallConfig {
            enabled: true,
            allow_ports: vec!["22".into(), "443".into()],
            ..Default::default()
        },
        ..base_cfg_for(None)
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        yaml.contains("ufw"),
        "ufw package must be added when firewall enabled"
    );
}

#[test]
fn ubuntu_cloud_init_swap_appears_in_late_commands() {
    let cfg = InjectConfig {
        swap: Some(SwapConfig {
            size_mb: 2048,
            ..Default::default()
        }),
        ..base_cfg_for(None)
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        yaml.contains("fallocate") || yaml.contains("swapfile"),
        "swap setup must appear in late-commands"
    );
}

#[test]
fn ubuntu_cloud_init_proxy_written_to_environment() {
    let cfg = InjectConfig {
        proxy: ProxyConfig {
            http_proxy: Some("http://proxy.corp.example.com:3128".into()),
            https_proxy: Some("http://proxy.corp.example.com:3128".into()),
            ..Default::default()
        },
        ..base_cfg_for(None)
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        yaml.contains("proxy.corp.example.com"),
        "proxy must appear in late-commands"
    );
}

#[test]
fn ubuntu_cloud_init_sysctl_pairs_written() {
    let cfg = InjectConfig {
        sysctl: vec![
            ("net.ipv4.ip_forward".into(), "1".into()),
            ("vm.swappiness".into(), "10".into()),
        ],
        ..base_cfg_for(None)
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        yaml.contains("net.ipv4.ip_forward"),
        "sysctl key must appear"
    );
    assert!(
        yaml.contains("vm.swappiness"),
        "second sysctl key must appear"
    );
}

#[test]
fn ubuntu_cloud_init_grub_timeout_written() {
    let cfg = InjectConfig {
        grub: GrubConfig {
            timeout: Some(5),
            ..Default::default()
        },
        ..base_cfg_for(None)
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        yaml.contains("GRUB_TIMEOUT") || yaml.contains("grub"),
        "GRUB timeout must appear"
    );
}

// ── Linux Mint / preseed path ─────────────────────────────────────────────────

#[test]
fn mint_preseed_structure() {
    let cfg = base_cfg_for(Some(Distro::Mint));
    let preseed = generate_mint_preseed(&cfg).expect("generate must succeed");
    assert!(
        preseed.contains("d-i"),
        "must use d-i directives (Debian preseed)"
    );
    assert!(preseed.contains("forge-test"), "hostname must be present");
    assert!(preseed.contains("admin"), "username must be present");
    assert_no_plaintext_password(&preseed, "mint preseed");
}

#[test]
fn mint_preseed_password_is_hashed() {
    let cfg = base_cfg_for(Some(Distro::Mint));
    let preseed = generate_mint_preseed(&cfg).expect("generate must succeed");
    assert!(
        preseed.contains("$6$"),
        "password must be SHA-512-crypt hashed"
    );
}

#[test]
fn mint_preseed_locale_and_timezone() {
    let cfg = base_cfg_for(Some(Distro::Mint));
    let preseed = generate_mint_preseed(&cfg).expect("generate must succeed");
    assert!(preseed.contains("en_US.UTF-8"), "locale must be set");
    assert!(preseed.contains("America/New_York"), "timezone must be set");
}

#[test]
fn mint_preseed_packages_included() {
    let cfg = base_cfg_for(Some(Distro::Mint));
    let preseed = generate_mint_preseed(&cfg).expect("generate must succeed");
    assert!(preseed.contains("curl"), "curl must be in package list");
    assert!(preseed.contains("git"), "git must be in package list");
}

#[test]
fn mint_preseed_extra_groups_written() {
    let cfg = InjectConfig {
        distro: Some(Distro::Mint),
        user: UserConfig {
            groups: vec!["docker".into(), "libvirt".into()],
            ..Default::default()
        },
        ..base_cfg_for(Some(Distro::Mint))
    };
    let preseed = generate_mint_preseed(&cfg).expect("generate must succeed");
    assert!(preseed.contains("docker"), "docker group must appear");
    assert!(preseed.contains("libvirt"), "libvirt group must appear");
}

#[test]
fn mint_preseed_validate_accepts_full_config() {
    let cfg = base_cfg_for(Some(Distro::Mint));
    assert!(
        cfg.validate().is_ok(),
        "fully-populated Mint config must validate"
    );
}

#[test]
fn mint_preseed_ntp_server_written() {
    let cfg = base_cfg_for(Some(Distro::Mint));
    let preseed = generate_mint_preseed(&cfg).expect("generate must succeed");
    assert!(
        preseed.contains("time.cloudflare.com"),
        "NTP server must be in preseed"
    );
}

// ── Fedora / Kickstart path ───────────────────────────────────────────────────

#[test]
fn fedora_kickstart_structure() {
    let cfg = base_cfg_for(Some(Distro::Fedora));
    let ks = generate_kickstart_cfg(&cfg).expect("generate must succeed");
    assert!(
        ks.contains("# Generated by ForgeISO"),
        "must have ForgeISO header"
    );
    assert!(ks.contains("lang en_US.UTF-8"), "locale must be set");
    assert!(
        ks.contains("timezone America/New_York"),
        "timezone must be set"
    );
    assert!(ks.contains("rootpw --lock"), "root must be locked");
    assert_no_plaintext_password(&ks, "fedora kickstart");
}

#[test]
fn fedora_kickstart_user_with_hashed_password() {
    let cfg = base_cfg_for(Some(Distro::Fedora));
    let ks = generate_kickstart_cfg(&cfg).expect("generate must succeed");
    assert!(
        ks.contains("user --name=admin"),
        "user directive must be present"
    );
    assert!(
        ks.contains("--iscrypted"),
        "password must be marked as hashed"
    );
    assert!(ks.contains("$6$"), "password must use SHA-512-crypt format");
}

#[test]
fn fedora_kickstart_ssh_key_directive() {
    let cfg = base_cfg_for(Some(Distro::Fedora));
    let ks = generate_kickstart_cfg(&cfg).expect("generate must succeed");
    assert!(
        ks.contains("sshkey --username=admin"),
        "sshkey directive must be present"
    );
    assert!(
        ks.contains("AAAA…testkey"),
        "actual key content must be present"
    );
}

#[test]
fn fedora_kickstart_packages_section() {
    let cfg = base_cfg_for(Some(Distro::Fedora));
    let ks = generate_kickstart_cfg(&cfg).expect("generate must succeed");
    assert!(
        ks.contains("%packages"),
        "%packages section must be present"
    );
    assert!(ks.contains("curl"), "curl must be in %packages");
    assert!(ks.contains("git"), "git must be in %packages");
    assert!(ks.contains("%end"), "%end must close %packages");
}

#[test]
fn fedora_kickstart_validate_accepts_full_config() {
    let cfg = base_cfg_for(Some(Distro::Fedora));
    assert!(
        cfg.validate().is_ok(),
        "fully-populated Fedora config must validate"
    );
}

#[test]
fn fedora_kickstart_no_user_no_user_directive() {
    // If no username is set, no `user` directive should be emitted.
    let cfg = InjectConfig {
        username: None,
        password: None,
        ..base_cfg_for(Some(Distro::Fedora))
    };
    let ks = generate_kickstart_cfg(&cfg).expect("generate must succeed");
    assert!(
        !ks.contains("user --name="),
        "no user directive expected when username=None"
    );
}

#[test]
fn fedora_kickstart_services_enabled_and_disabled() {
    let cfg = InjectConfig {
        enable_services: vec!["sshd".into(), "docker".into()],
        disable_services: vec!["firewalld".into()],
        ..base_cfg_for(Some(Distro::Fedora))
    };
    let ks = generate_kickstart_cfg(&cfg).expect("generate must succeed");
    assert!(ks.contains("sshd"), "sshd must appear as enabled service");
    assert!(
        ks.contains("firewalld"),
        "firewalld must appear as disabled service"
    );
}

#[test]
fn fedora_kickstart_ntp_in_post_section() {
    let cfg = base_cfg_for(Some(Distro::Fedora));
    let ks = generate_kickstart_cfg(&cfg).expect("generate must succeed");
    // NTP config ends up in %post via build_feature_late_commands
    assert!(
        ks.contains("time.cloudflare.com") || ks.contains("timesyncd"),
        "NTP server must appear in kickstart %post"
    );
}

#[test]
fn fedora_kickstart_keyboard_directive() {
    let cfg = base_cfg_for(Some(Distro::Fedora));
    let ks = generate_kickstart_cfg(&cfg).expect("generate must succeed");
    assert!(ks.contains("keyboard us"), "keyboard layout must be set");
}

#[test]
fn fedora_kickstart_run_commands_in_post() {
    let cfg = InjectConfig {
        run_commands: vec!["echo 'ForgeISO installed'".into()],
        ..base_cfg_for(Some(Distro::Fedora))
    };
    let ks = generate_kickstart_cfg(&cfg).expect("generate must succeed");
    assert!(
        ks.contains("%post"),
        "%post section must be present when run_commands set"
    );
    assert!(
        ks.contains("ForgeISO installed"),
        "run_commands must appear in %post"
    );
}

// ── RHEL-family regression (Rocky, AlmaLinux, CentOS via Fedora/Kickstart) ───

#[test]
fn rhel_family_uses_fedora_kickstart_path() {
    // Rocky/Alma/CentOS all go through the Fedora/Kickstart code path in the
    // engine — the inject config uses Distro::Fedora as the closest enum match.
    // Verify the output is valid Kickstart.
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora), // RHEL-family maps to Distro::Fedora
        hostname: Some("rocky-node".into()),
        username: Some("rhadmin".into()),
        password: Some("RhPass1!".into()),
        extra_packages: vec!["epel-release".into(), "vim".into()],
        ..Default::default()
    };
    let ks = generate_kickstart_cfg(&cfg).expect("RHEL-family kickstart must succeed");
    assert!(ks.contains("rhadmin"), "hostname must appear");
    assert!(
        ks.contains("epel-release"),
        "epel-release package must be present"
    );
    assert!(
        !ks.contains("RhPass1!"),
        "plaintext password must not appear"
    );
}

// ── Arch Linux path ───────────────────────────────────────────────────────────

#[test]
fn arch_cloud_init_uses_ubuntu_autoinstall_yaml() {
    // The engine generates a cloud-init-like YAML even for Arch as a
    // best-effort; the orchestrator additionally writes archinstall-config.json.
    // Verify the base generate_autoinstall_yaml does not fail for Arch.
    let cfg = base_cfg_for(Some(Distro::Arch));
    let yaml = generate_autoinstall_yaml(&cfg).expect("arch autoinstall must succeed");
    assert!(
        yaml.starts_with("#cloud-config"),
        "must have cloud-config header"
    );
}

#[test]
fn arch_inject_config_validate_accepts_full_config() {
    let cfg = base_cfg_for(Some(Distro::Arch));
    assert!(
        cfg.validate().is_ok(),
        "fully-populated Arch config must validate"
    );
}

#[test]
fn arch_inject_config_with_pacman_repos() {
    let cfg = InjectConfig {
        distro: Some(Distro::Arch),
        pacman_repos: vec!["Server = https://mirror.pkgbuild.com/$repo/os/$arch".into()],
        pacman_mirror: Some("https://mirror.pkgbuild.com".into()),
        ..base_cfg_for(Some(Distro::Arch))
    };
    // validate() does not check pacman_repos content — just that fields are set
    assert!(
        cfg.validate().is_ok(),
        "Arch config with pacman repos must validate"
    );
    assert_eq!(cfg.pacman_repos.len(), 1);
    assert!(cfg.pacman_mirror.is_some());
}

// ── Arch-based distros (EndeavourOS, Garuda, Manjaro) ────────────────────────

#[test]
fn endeavouros_uses_arch_path() {
    // EndeavourOS is Arch-based; form distro = "arch" → Distro::Arch
    let cfg = InjectConfig {
        distro: Some(Distro::Arch),
        hostname: Some("endeavour-node".into()),
        username: Some("eos".into()),
        password: Some("EnPass1!".into()),
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("must succeed");
    assert!(yaml.starts_with("#cloud-config"));
    assert!(!yaml.contains("EnPass1!"), "plaintext must not appear");
}

#[test]
fn garuda_uses_arch_path() {
    let cfg = InjectConfig {
        distro: Some(Distro::Arch),
        hostname: Some("garuda-node".into()),
        username: Some("garuda".into()),
        password: Some("GarPass1!".into()),
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("must succeed");
    assert!(
        !yaml.contains("GarPass1!"),
        "plaintext must not appear in garuda config"
    );
}

#[test]
fn manjaro_uses_arch_path() {
    let cfg = InjectConfig {
        distro: Some(Distro::Arch),
        hostname: Some("manjaro-node".into()),
        username: Some("mj".into()),
        password: Some("MjPass1!".into()),
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("must succeed");
    assert!(
        !yaml.contains("MjPass1!"),
        "plaintext must not appear in manjaro config"
    );
}

// ── Debian / preseed path (uses Ubuntu cloud-init path) ──────────────────────

#[test]
fn debian_uses_ubuntu_cloud_init_path() {
    // Debian has no dedicated engine path; it falls through to Ubuntu cloud-init.
    // The distro field maps to None (engine default) or Ubuntu in the GUI mapping.
    let cfg = InjectConfig {
        distro: None, // debian → "ubuntu" in form → None in engine
        hostname: Some("debian-node".into()),
        username: Some("debadmin".into()),
        password: Some("DebPass1!".into()),
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("must succeed");
    assert!(yaml.contains("debian-node"), "hostname must appear");
    assert!(!yaml.contains("DebPass1!"), "plaintext must not appear");
}

// ── Kali Linux (Debian-based) ─────────────────────────────────────────────────

#[test]
fn kali_uses_ubuntu_cloud_init_path() {
    let cfg = InjectConfig {
        distro: None,
        hostname: Some("kali-node".into()),
        username: Some("kali".into()),
        password: Some("KaliPass1!".into()),
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("must succeed");
    assert!(yaml.contains("kali-node"), "hostname must appear");
    assert!(!yaml.contains("KaliPass1!"), "plaintext must not appear");
}

// ── openSUSE (falls back to Ubuntu cloud-init path) ───────────────────────────

#[test]
fn opensuse_falls_back_to_ubuntu_cloud_init() {
    // openSUSE → form distro "ubuntu" → Distro::None → generate_autoinstall_yaml
    let cfg = InjectConfig {
        distro: None,
        hostname: Some("suse-node".into()),
        username: Some("suseadmin".into()),
        password: Some("SusePass1!".into()),
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("must succeed");
    assert!(yaml.contains("suse-node"), "hostname must appear");
    assert!(!yaml.contains("SusePass1!"), "plaintext must not appear");
}

// ── Pop!_OS (Ubuntu-based) ───────────────────────────────────────────────────

#[test]
fn popos_uses_ubuntu_cloud_init_path() {
    let cfg = InjectConfig {
        distro: None, // Pop!_OS is Ubuntu-based
        hostname: Some("pop-node".into()),
        username: Some("popadmin".into()),
        password: Some("PopPass1!".into()),
        extra_packages: vec!["flatpak".into()],
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("must succeed");
    assert!(yaml.contains("pop-node"), "hostname must appear");
    assert!(yaml.contains("flatpak"), "flatpak package must be present");
    assert!(!yaml.contains("PopPass1!"), "plaintext must not appear");
}

// ── Ubuntu variants (Desktop LTS, Server LTS) ────────────────────────────────

#[test]
fn ubuntu_server_lts_full_stack() {
    let cfg = InjectConfig {
        distro: None,
        hostname: Some("ubuntu-server".into()),
        username: Some("srvadmin".into()),
        password: Some("SrvPass1!".into()),
        ssh: SshConfig {
            authorized_keys: vec!["ssh-ed25519 BBBB…srv".into()],
            allow_password_auth: Some(false),
            install_server: Some(true),
        },
        enable_services: vec!["docker".into()],
        extra_packages: vec!["docker.io".into()],
        firewall: FirewallConfig {
            enabled: true,
            allow_ports: vec!["22".into(), "80".into(), "443".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    cfg.validate().expect("ubuntu server config must validate");
    let yaml = generate_autoinstall_yaml(&cfg).expect("must succeed");
    assert!(yaml.contains("ubuntu-server"), "hostname");
    assert!(yaml.contains("docker.io"), "docker.io package");
    assert!(yaml.contains("ufw"), "ufw added for firewall");
    assert!(!yaml.contains("SrvPass1!"), "plaintext must not appear");
}

#[test]
fn ubuntu_desktop_lts_full_stack() {
    let cfg = InjectConfig {
        distro: None,
        hostname: Some("ubuntu-desktop".into()),
        username: Some("deskadmin".into()),
        password: Some("DeskPass1!".into()),
        locale: Some("fr_FR.UTF-8".into()),
        timezone: Some("Europe/Paris".into()),
        keyboard_layout: Some("fr".into()),
        extra_packages: vec!["gnome-tweaks".into(), "vlc".into()],
        ..Default::default()
    };
    cfg.validate().expect("ubuntu desktop config must validate");
    let yaml = generate_autoinstall_yaml(&cfg).expect("must succeed");
    assert!(yaml.contains("fr_FR.UTF-8"), "locale must be set");
    assert!(yaml.contains("Europe/Paris"), "timezone must be set");
    assert!(
        yaml.contains("gnome-tweaks"),
        "desktop package must be present"
    );
    assert!(!yaml.contains("DeskPass1!"), "plaintext must not appear");
}

// ── Linux Mint variants (Cinnamon, MATE, Xfce) ───────────────────────────────

#[test]
fn mint_cinnamon_full_stack() {
    let cfg = InjectConfig {
        distro: Some(Distro::Mint),
        hostname: Some("mint-cinnamon".into()),
        username: Some("mintuser".into()),
        password: Some("MintPass1!".into()),
        locale: Some("en_GB.UTF-8".into()),
        timezone: Some("Europe/London".into()),
        ..Default::default()
    };
    cfg.validate().expect("mint cinnamon config must validate");
    let preseed = generate_mint_preseed(&cfg).expect("must succeed");
    assert!(preseed.contains("mint-cinnamon"), "hostname");
    assert!(preseed.contains("Europe/London"), "timezone");
    assert!(preseed.contains("en_GB.UTF-8"), "locale");
    assert!(!preseed.contains("MintPass1!"), "plaintext must not appear");
}

#[test]
fn mint_with_apt_mirror() {
    let cfg = InjectConfig {
        distro: Some(Distro::Mint),
        apt_mirror: Some("http://mirrors.us.kernel.org/ubuntu".into()),
        ..base_cfg_for(Some(Distro::Mint))
    };
    cfg.validate()
        .expect("config with apt_mirror must validate");
    let preseed = generate_mint_preseed(&cfg).expect("must succeed");
    assert!(
        preseed.contains("mirrors.us.kernel.org"),
        "apt mirror host must appear in preseed"
    );
}

// ── Fedora variants (Server, Workstation) ────────────────────────────────────

#[test]
fn fedora_server_with_dnf_repos() {
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora),
        hostname: Some("fedora-server".into()),
        username: Some("fedadmin".into()),
        password: Some("FedPass1!".into()),
        dnf_repos: vec!["[rpmfusion-free]\nbaseurl=https://mirrors.rpmfusion.org/free/fedora/$releasever/$basearch\nenabled=1".into()],
        extra_packages: vec!["vim-enhanced".into(), "htop".into()],
        ..Default::default()
    };
    cfg.validate().expect("fedora server config must validate");
    let ks = generate_kickstart_cfg(&cfg).expect("must succeed");
    assert!(ks.contains("fedadmin"), "username must appear");
    assert!(ks.contains("vim-enhanced"), "package must appear");
    assert!(!ks.contains("FedPass1!"), "plaintext must not appear");
}

#[test]
fn fedora_workstation_with_static_ip() {
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora),
        hostname: Some("fedora-ws".into()),
        username: Some("feduser".into()),
        password: Some("WsPass1!".into()),
        static_ip: Some("192.168.1.100/24".into()),
        gateway: Some("192.168.1.1".into()),
        network: NetworkConfig {
            dns_servers: vec!["192.168.1.1".into()],
            ntp_servers: vec!["pool.ntp.org".into()],
        },
        ..Default::default()
    };
    cfg.validate()
        .expect("fedora workstation config must validate");
    let ks = generate_kickstart_cfg(&cfg).expect("must succeed");
    assert!(ks.contains("192.168.1.100"), "static IP must appear");
}

// ── Rocky Linux (RHEL-family via Fedora path) ─────────────────────────────────

#[test]
fn rocky_linux_full_stack() {
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora), // Rocky → Distro::Fedora path
        hostname: Some("rocky-prod".into()),
        username: Some("rockyadmin".into()),
        password: Some("RockyPass1!".into()),
        extra_packages: vec!["epel-release".into(), "vim".into(), "bind-utils".into()],
        enable_services: vec!["sshd".into()],
        disable_services: vec!["cockpit".into()],
        firewall: FirewallConfig {
            enabled: true,
            allow_ports: vec!["22".into(), "8080".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    cfg.validate().expect("rocky linux config must validate");
    let ks = generate_kickstart_cfg(&cfg).expect("must succeed");
    assert!(ks.contains("rocky-prod"), "hostname must appear");
    assert!(ks.contains("epel-release"), "epel package must appear");
    assert!(ks.contains("sshd"), "enabled service must appear");
    assert!(!ks.contains("RockyPass1!"), "plaintext must not appear");
}

// ── AlmaLinux (RHEL-family via Fedora path) ───────────────────────────────────

#[test]
fn almalinux_full_stack() {
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora),
        hostname: Some("alma-prod".into()),
        username: Some("almaadmin".into()),
        password: Some("AlmaPass1!".into()),
        extra_packages: vec!["epel-release".into()],
        ..Default::default()
    };
    cfg.validate().expect("almalinux config must validate");
    let ks = generate_kickstart_cfg(&cfg).expect("must succeed");
    assert!(ks.contains("alma-prod"));
    assert!(!ks.contains("AlmaPass1!"));
}

// ── CentOS Stream (RHEL-family via Fedora path) ───────────────────────────────

#[test]
fn centos_stream_full_stack() {
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora),
        hostname: Some("centos-stream".into()),
        username: Some("centosadmin".into()),
        password: Some("CentosPass1!".into()),
        ..Default::default()
    };
    cfg.validate().expect("centos stream config must validate");
    let ks = generate_kickstart_cfg(&cfg).expect("must succeed");
    assert!(ks.contains("centos-stream"));
    assert!(!ks.contains("CentosPass1!"));
}

// ── Security: no plaintext secrets in any distro output ──────────────────────

#[test]
fn no_plaintext_password_across_all_distros() {
    let password = "S3cur3P@ssw0rd!#$";
    let distros = [
        (None, "ubuntu"),
        (Some(Distro::Mint), "mint"),
        (Some(Distro::Fedora), "fedora"),
        (Some(Distro::Arch), "arch"),
    ];
    for (distro, label) in &distros {
        let cfg = InjectConfig {
            distro: *distro,
            username: Some("sec-test".into()),
            password: Some(password.into()),
            ..Default::default()
        };
        match distro {
            Some(Distro::Mint) => {
                let out = generate_mint_preseed(&cfg).expect("must succeed");
                assert!(
                    !out.contains(password),
                    "{label}: plaintext password must not appear in preseed"
                );
            }
            Some(Distro::Fedora) => {
                let out = generate_kickstart_cfg(&cfg).expect("must succeed");
                assert!(
                    !out.contains(password),
                    "{label}: plaintext password must not appear in kickstart"
                );
            }
            _ => {
                let out = generate_autoinstall_yaml(&cfg).expect("must succeed");
                assert!(
                    !out.contains(password),
                    "{label}: plaintext password must not appear in cloud-init YAML"
                );
            }
        }
    }
}

// ── Config validation: each distro accepts a full config without error ────────

#[test]
fn all_distros_validate_minimal_config() {
    let distros = [
        (None, "ubuntu"),
        (Some(Distro::Mint), "mint"),
        (Some(Distro::Fedora), "fedora"),
        (Some(Distro::Arch), "arch"),
    ];
    for (distro, label) in distros {
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".into(),
            distro,
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "{label}: minimal InjectConfig must pass validate()"
        );
    }
}

#[test]
fn all_distros_validate_full_config() {
    let distros = [
        (None, "ubuntu"),
        (Some(Distro::Mint), "mint"),
        (Some(Distro::Fedora), "fedora"),
        (Some(Distro::Arch), "arch"),
    ];
    for (distro, label) in distros {
        let cfg = base_cfg_for(distro);
        assert!(
            cfg.validate().is_ok(),
            "{label}: fully-populated InjectConfig must pass validate()"
        );
    }
}

// ── ISO preset catalog: each preset resolves without panic ────────────────────

#[test]
fn all_presets_resolve_without_panic() {
    use forgeiso_engine::sources::{all_presets, resolve_url};
    for preset in all_presets() {
        let result = resolve_url(preset);
        assert!(
            result.is_ok(),
            "resolve_url panicked or errored for preset {}",
            preset.id.as_str()
        );
    }
}

#[test]
fn all_preset_direct_url_strings_start_with_https() {
    use forgeiso_engine::sources::{all_presets, resolve_url, AcquisitionStrategy};
    for preset in all_presets() {
        if preset.strategy == AcquisitionStrategy::DirectUrl {
            if let Ok(Some(url)) = resolve_url(preset) {
                assert!(
                    url.starts_with("https://"),
                    "preset {} direct URL must use HTTPS, got: {}",
                    preset.id.as_str(),
                    url
                );
            }
        }
    }
}

// ── SSH inject across all distros ─────────────────────────────────────────────

#[test]
fn ssh_key_injection_across_all_distros() {
    let key = "ssh-ed25519 AAAAC3Nz…test regression key";
    let distros: &[(Option<Distro>, &str)] = &[(None, "ubuntu"), (Some(Distro::Fedora), "fedora")];
    for (distro, label) in distros {
        let cfg = InjectConfig {
            distro: *distro,
            username: Some("keytest".into()),
            ssh: SshConfig {
                authorized_keys: vec![key.into()],
                allow_password_auth: Some(false),
                install_server: Some(true),
            },
            ..Default::default()
        };
        let out = if matches!(distro, Some(Distro::Fedora)) {
            generate_kickstart_cfg(&cfg).expect("must succeed")
        } else {
            generate_autoinstall_yaml(&cfg).expect("must succeed")
        };
        assert!(
            out.contains("AAAAC3Nz…test regression key"),
            "{label}: SSH key must appear in generated config"
        );
    }
}

// ── Mint SSH key injection via late_command ───────────────────────────────────

#[test]
fn mint_preseed_ssh_keys_in_late_command() {
    let key = "ssh-ed25519 AAAAC3Nz…mint-regression-key";
    let cfg = InjectConfig {
        distro: Some(Distro::Mint),
        username: Some("mintuser".into()),
        ssh: SshConfig {
            authorized_keys: vec![key.into()],
            install_server: Some(true),
            allow_password_auth: Some(false),
        },
        ..base_cfg_for(Some(Distro::Mint))
    };
    let preseed = generate_mint_preseed(&cfg).expect("must succeed");
    assert!(
        preseed.contains("late_command"),
        "late_command must be emitted when SSH keys are set for Mint"
    );
    assert!(
        preseed.contains("AAAAC3Nz…mint-regression-key"),
        "SSH key must appear in late_command"
    );
    assert!(
        preseed.contains(".ssh/authorized_keys"),
        "authorized_keys path must appear in late_command"
    );
    assert!(
        preseed.contains("chmod 700"),
        ".ssh dir must have correct permissions"
    );
}

#[test]
fn mint_preseed_no_late_command_without_features() {
    // A minimal preseed with no features requiring late_command should not emit one.
    let cfg = InjectConfig {
        distro: Some(Distro::Mint),
        hostname: Some("plain-mint".into()),
        username: Some("user".into()),
        password: Some("Pass1!".into()),
        timezone: Some("UTC".into()),
        locale: Some("en_US.UTF-8".into()),
        keyboard_layout: Some("us".into()),
        ..Default::default()
    };
    let preseed = generate_mint_preseed(&cfg).expect("must succeed");
    assert!(
        !preseed.contains("late_command"),
        "plain config should not emit late_command"
    );
}

// ── Mint preseed DNS / static IP ──────────────────────────────────────────────

#[test]
fn mint_preseed_custom_dns_servers() {
    let cfg = InjectConfig {
        distro: Some(Distro::Mint),
        network: NetworkConfig {
            dns_servers: vec!["8.8.8.8".into(), "1.1.1.1".into()],
            ntp_servers: vec![],
        },
        ..base_cfg_for(Some(Distro::Mint))
    };
    let preseed = generate_mint_preseed(&cfg).expect("must succeed");
    assert!(
        preseed.contains("netcfg/get_nameservers string 8.8.8.8 1.1.1.1"),
        "DNS servers must appear in preseed netcfg directive"
    );
}

#[test]
fn mint_preseed_static_ip_directives() {
    let cfg = InjectConfig {
        distro: Some(Distro::Mint),
        static_ip: Some("192.168.1.50/24".into()),
        gateway: Some("192.168.1.1".into()),
        network: NetworkConfig {
            dns_servers: vec!["8.8.8.8".into()],
            ntp_servers: vec![],
        },
        ..base_cfg_for(Some(Distro::Mint))
    };
    let preseed = generate_mint_preseed(&cfg).expect("must succeed");
    assert!(
        preseed.contains("netcfg/disable_autoconfig boolean true"),
        "static IP must disable autoconfig"
    );
    assert!(
        preseed.contains("netcfg/get_ipaddress string 192.168.1.50"),
        "static IP address must appear"
    );
    assert!(
        preseed.contains("netcfg/get_netmask string 255.255.255.0"),
        "netmask derived from /24 must appear"
    );
    assert!(
        preseed.contains("netcfg/get_gateway string 192.168.1.1"),
        "gateway must appear"
    );
    assert!(
        preseed.contains("netcfg/confirm_static boolean true"),
        "static confirmation must appear"
    );
}

// ── Arch SSH keys in archinstall JSON ────────────────────────────────────────
// NOTE: build_archinstall_config is a private fn in orchestrator.rs; the SSH
// key injection is tested via unit tests in that module.  This integration test
// verifies the distro_regression fixture for Arch validates cleanly and that
// the autoinstall YAML path (used as a fallback) still produces a valid config.

#[test]
fn arch_full_config_with_ssh_key_validates() {
    let cfg = InjectConfig {
        distro: Some(Distro::Arch),
        username: Some("archuser".into()),
        password: Some("ArchPass1!".into()),
        ssh: SshConfig {
            authorized_keys: vec!["ssh-ed25519 AAAAC3Nz…arch-regression-key".into()],
            install_server: Some(true),
            allow_password_auth: Some(false),
        },
        ..base_cfg_for(Some(Distro::Arch))
    };
    assert!(
        cfg.validate().is_ok(),
        "Arch config with SSH keys must validate"
    );
    // generate_autoinstall_yaml for Arch uses the Ubuntu cloud-init YAML fallback;
    // confirm the key still appears there for any path that uses that output.
    let yaml = generate_autoinstall_yaml(&cfg).expect("must succeed");
    assert!(
        yaml.contains("AAAAC3Nz…arch-regression-key"),
        "SSH key must appear in cloud-init YAML for Arch fallback path"
    );
}

// ── Fedora Docker/Podman in %post ─────────────────────────────────────────────

#[test]
fn fedora_kickstart_docker_in_post() {
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora),
        containers: ContainerConfig {
            docker: true,
            docker_users: vec!["fedadmin".into()],
            podman: false,
        },
        ..base_cfg_for(Some(Distro::Fedora))
    };
    let ks = generate_kickstart_cfg(&cfg).expect("must succeed");
    assert!(
        ks.contains("docker-ce.repo"),
        "Docker CE repo must be added in %post"
    );
    assert!(
        ks.contains("docker-ce"),
        "docker-ce package must be installed"
    );
    assert!(
        ks.contains("systemctl enable docker"),
        "docker must be enabled"
    );
    assert!(
        ks.contains("usermod -aG docker fedadmin"),
        "user must be added to docker group"
    );
}

#[test]
fn fedora_kickstart_podman_in_post() {
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora),
        containers: ContainerConfig {
            podman: true,
            docker: false,
            docker_users: vec![],
        },
        ..base_cfg_for(Some(Distro::Fedora))
    };
    let ks = generate_kickstart_cfg(&cfg).expect("must succeed");
    assert!(
        ks.contains("dnf install -y podman"),
        "podman must be installed via dnf in %post"
    );
}

// ── InjectConfig output_label validation ─────────────────────────────────────

#[test]
fn validate_rejects_blank_output_label() {
    let cfg = InjectConfig {
        output_label: Some("   ".into()),
        ..base_cfg_for(None)
    };
    assert!(
        cfg.validate().is_err(),
        "blank output_label must be rejected"
    );
}

#[test]
fn validate_rejects_output_label_over_32_chars() {
    let cfg = InjectConfig {
        output_label: Some("A".repeat(33)),
        ..base_cfg_for(None)
    };
    assert!(
        cfg.validate().is_err(),
        "output_label > 32 chars must be rejected"
    );
}

#[test]
fn validate_accepts_valid_output_label() {
    let cfg = InjectConfig {
        output_label: Some("UBUNTU_HARDENED_22_04".into()),
        ..base_cfg_for(None)
    };
    assert!(cfg.validate().is_ok(), "valid output_label must pass");
}

// ── Fedora firewall late-commands ─────────────────────────────────────────────

#[test]
fn fedora_kickstart_firewall_directive_in_header() {
    // A native `firewall --enabled` directive must appear in the Kickstart
    // header when firewall is enabled (before %packages).
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora),
        firewall: FirewallConfig {
            enabled: true,
            ..Default::default()
        },
        ..base_cfg_for(Some(Distro::Fedora))
    };
    let ks = generate_kickstart_cfg(&cfg).expect("must succeed");
    assert!(
        ks.contains("firewall --enabled"),
        "kickstart must contain native 'firewall --enabled' directive"
    );
}

#[test]
fn fedora_kickstart_firewall_disabled_directive() {
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora),
        firewall: FirewallConfig {
            enabled: false,
            ..Default::default()
        },
        ..base_cfg_for(Some(Distro::Fedora))
    };
    let ks = generate_kickstart_cfg(&cfg).expect("must succeed");
    assert!(
        ks.contains("firewall --disabled"),
        "kickstart must contain 'firewall --disabled' when firewall is off"
    );
}

#[test]
fn fedora_kickstart_firewall_port_rules_in_post() {
    // firewall-cmd commands for allow/deny ports must appear in %post.
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora),
        firewall: FirewallConfig {
            enabled: true,
            allow_ports: vec!["22/tcp".into(), "443/tcp".into()],
            deny_ports: vec!["23/tcp".into()],
            default_policy: Some("deny".into()),
        },
        ..base_cfg_for(Some(Distro::Fedora))
    };
    let ks = generate_kickstart_cfg(&cfg).expect("must succeed");
    assert!(
        ks.contains("firewall-cmd --permanent --add-port=22/tcp"),
        "allow port 22 must appear as firewall-cmd in %post"
    );
    assert!(
        ks.contains("firewall-cmd --permanent --add-port=443/tcp"),
        "allow port 443 must appear as firewall-cmd in %post"
    );
    assert!(
        ks.contains("firewall-cmd --permanent --remove-port=23/tcp"),
        "deny port 23 must appear as firewall-cmd --remove-port in %post"
    );
    assert!(
        ks.contains("firewall-cmd --permanent --set-target=DROP"),
        "deny policy must map to DROP target in %post"
    );
    assert!(
        ks.contains("systemctl enable firewalld"),
        "firewalld must be enabled in %post"
    );
}

// ── out_name regression ───────────────────────────────────────────────────────

#[test]
fn validate_rejects_out_name_path_traversal() {
    let cfg = InjectConfig {
        out_name: "../../etc/shadow".into(),
        ..base_cfg_for(None)
    };
    assert!(
        cfg.validate().is_err(),
        "out_name with path traversal must be rejected"
    );
}

#[test]
fn validate_accepts_plain_out_name() {
    let cfg = InjectConfig {
        out_name: "ubuntu-hardened.iso".into(),
        ..base_cfg_for(None)
    };
    assert!(cfg.validate().is_ok(), "plain out_name filename must pass");
}

// ── DNF mirror / repo regression ──────────────────────────────────────────────

#[test]
fn validate_rejects_dnf_mirror_with_pipe() {
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora),
        dnf_mirror: Some("https://mirror.example.com|sed s/x/y/".into()),
        ..base_cfg_for(Some(Distro::Fedora))
    };
    assert!(
        cfg.validate().is_err(),
        "dnf_mirror with | must be rejected (breaks sed command)"
    );
}

#[test]
fn validate_accepts_dnf_mirror_clean_url() {
    let cfg = InjectConfig {
        distro: Some(Distro::Fedora),
        dnf_mirror: Some("https://mirror.example.com/fedora".into()),
        ..base_cfg_for(Some(Distro::Fedora))
    };
    assert!(cfg.validate().is_ok(), "clean dnf_mirror URL must pass");
}

// ── Pacman mirror / repo regression ───────────────────────────────────────────

#[test]
fn validate_rejects_pacman_repo_with_single_quote() {
    let cfg = InjectConfig {
        distro: Some(Distro::Arch),
        pacman_repos: vec!["Server = https://mirror.example.com'; rm -rf /".into()],
        ..base_cfg_for(Some(Distro::Arch))
    };
    assert!(
        cfg.validate().is_err(),
        "pacman_repo with single quote must be rejected"
    );
}

#[test]
fn validate_accepts_pacman_repo_with_template_vars() {
    let cfg = InjectConfig {
        distro: Some(Distro::Arch),
        pacman_repos: vec!["Server = https://mirror.pkgbuild.com/$repo/os/$arch".into()],
        ..base_cfg_for(Some(Distro::Arch))
    };
    assert!(
        cfg.validate().is_ok(),
        "pacman Server= line with $repo/$arch template vars must pass"
    );
}
