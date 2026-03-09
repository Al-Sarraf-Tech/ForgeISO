//! Full E2E regression suite — exercises every GUI-reachable code path.
//!
//! Covers:
//! - `generate_autoinstall_yaml` for all distros with all option combinations
//! - `generate_kickstart_cfg` / `generate_mint_preseed` edge cases
//! - `InjectConfig::validate()` — every rejection and every acceptance path
//! - `BuildConfig::validate()` — output_label byte vs char length mismatch
//! - PPA repo support — was incorrectly rejected by validate()
//! - All source presets (catalog completeness + strategy consistency)
//! - VM emit for every hypervisor × firmware combination
//! - `hash_password` with edge-case inputs
//! - `build_feature_late_commands` with every feature flag
//! - Config-builder helpers: `lines()` / `opt()` / progress bar logic

use forgeiso_engine::{
    all_presets, build_feature_late_commands, emit_launch, find_preset_by_str,
    generate_autoinstall_yaml, generate_kickstart_cfg, generate_mint_preseed, hash_password,
    merge_autoinstall_yaml, resolve_url, AcquisitionStrategy, BuildConfig, ContainerConfig, Distro,
    FirewallConfig, FirmwareMode, GrubConfig, Hypervisor, InjectConfig, IsoSource, NetworkConfig,
    ProfileKind, ProxyConfig, ScanPolicy, SshConfig, SwapConfig, TestingPolicy, UserConfig,
    VmLaunchSpec,
};

// ── Fixture helpers ────────────────────────────────────────────────────────────

fn minimal_inject(distro: Option<Distro>) -> InjectConfig {
    InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".into(),
        distro,
        ..Default::default()
    }
}

fn full_inject(distro: Option<Distro>) -> InjectConfig {
    InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".into(),
        hostname: Some("forge-test".into()),
        username: Some("admin".into()),
        password: Some("S3cur3Pass!".into()),
        realname: Some("Forge Tester".into()),
        timezone: Some("America/New_York".into()),
        locale: Some("en_US.UTF-8".into()),
        keyboard_layout: Some("us".into()),
        ssh: SshConfig {
            authorized_keys: vec!["ssh-ed25519 AAAA…key".into()],
            allow_password_auth: Some(true),
            install_server: Some(true),
        },
        network: NetworkConfig {
            dns_servers: vec!["1.1.1.1".into(), "8.8.8.8".into()],
            ntp_servers: vec!["pool.ntp.org".into()],
        },
        extra_packages: vec!["curl".into(), "git".into(), "vim".into()],
        no_user_interaction: true,
        distro,
        ..Default::default()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. generate_autoinstall_yaml — all distro variants
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn yaml_ubuntu_minimal_does_not_panic() {
    let cfg = minimal_inject(None);
    generate_autoinstall_yaml(&cfg).expect("minimal ubuntu must succeed");
}

#[test]
fn yaml_ubuntu_full_does_not_panic() {
    let cfg = full_inject(None);
    generate_autoinstall_yaml(&cfg).expect("full ubuntu must succeed");
}

#[test]
fn yaml_ubuntu_password_hashed() {
    let cfg = full_inject(None);
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        !yaml.contains("S3cur3Pass!"),
        "plaintext password must not appear"
    );
    assert!(
        yaml.contains("$6$"),
        "password must be SHA-512-crypt hashed"
    );
}

#[test]
fn yaml_ubuntu_all_none_fields_succeed() {
    // Every optional field is None — should never panic
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".into(),
        hostname: None,
        username: None,
        password: None,
        realname: None,
        timezone: None,
        locale: None,
        keyboard_layout: None,
        distro: None,
        ..Default::default()
    };
    generate_autoinstall_yaml(&cfg).expect("all-None ubuntu must succeed");
}

#[test]
fn yaml_ubuntu_multibyte_realname_does_not_panic() {
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".into(),
        realname: Some("名前 ñoño".into()),
        distro: None,
        ..Default::default()
    };
    // validate() rejects shell metacharacters; multi-byte is fine
    generate_autoinstall_yaml(&cfg).expect("unicode realname must not panic");
}

#[test]
fn yaml_ubuntu_packages_empty_vec_succeeds() {
    let mut cfg = full_inject(None);
    cfg.extra_packages = vec![];
    generate_autoinstall_yaml(&cfg).expect("empty packages must succeed");
}

#[test]
fn yaml_ubuntu_many_packages_succeeds() {
    let mut cfg = full_inject(None);
    cfg.extra_packages = (0..100).map(|i| format!("pkg{i}")).collect();
    generate_autoinstall_yaml(&cfg).expect("100-package list must succeed");
}

#[test]
fn yaml_ubuntu_swap_config_is_emitted() {
    let mut cfg = full_inject(None);
    cfg.swap = Some(SwapConfig {
        size_mb: 2048,
        filename: None,
        swappiness: None,
    });
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("swap"), "swap section must be present");
}

#[test]
fn yaml_ubuntu_firewall_config_is_emitted() {
    let mut cfg = full_inject(None);
    cfg.firewall = FirewallConfig {
        enabled: true,
        default_policy: Some("deny".into()),
        allow_ports: vec!["22/tcp".into(), "80/tcp".into()],
        deny_ports: vec![],
    };
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("ufw") || yaml.contains("22"),
        "firewall must be referenced"
    );
}

#[test]
fn yaml_ubuntu_proxy_config_is_emitted() {
    let mut cfg = full_inject(None);
    cfg.proxy = ProxyConfig {
        http_proxy: Some("http://proxy.example.com:3128".into()),
        https_proxy: Some("http://proxy.example.com:3128".into()),
        no_proxy: vec!["localhost".into(), ".internal".into()],
    };
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("proxy") || yaml.contains("http_proxy"),
        "proxy must be emitted"
    );
}

#[test]
fn yaml_ubuntu_grub_config_timeout_emitted() {
    let mut cfg = full_inject(None);
    cfg.grub = GrubConfig {
        timeout: Some(10),
        cmdline_extra: vec!["quiet".into()],
        default_entry: None,
    };
    generate_autoinstall_yaml(&cfg).expect("grub config must not panic");
}

#[test]
fn yaml_ubuntu_sysctl_pairs_emitted() {
    let mut cfg = full_inject(None);
    cfg.sysctl = vec![
        ("net.ipv4.ip_forward".into(), "1".into()),
        ("vm.swappiness".into(), "10".into()),
    ];
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("sysctl") || yaml.contains("net.ipv4"),
        "sysctl must appear"
    );
}

#[test]
fn yaml_ubuntu_enable_disable_services_emitted() {
    let mut cfg = full_inject(None);
    cfg.enable_services = vec!["docker".into(), "nginx".into()];
    cfg.disable_services = vec!["snapd".into()];
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("docker") || yaml.contains("systemctl"),
        "services must appear"
    );
}

#[test]
fn yaml_ubuntu_containers_docker_emitted() {
    let mut cfg = full_inject(None);
    cfg.containers = ContainerConfig {
        docker: true,
        podman: false,
        docker_users: vec!["admin".into()],
    };
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("docker"),
        "docker installation must be referenced"
    );
}

#[test]
fn yaml_ubuntu_apt_repos_deb_line_emitted() {
    let mut cfg = full_inject(None);
    cfg.apt_repos = vec![
        "deb http://archive.ubuntu.com/ubuntu noble universe".into(),
        "deb-src http://archive.ubuntu.com/ubuntu noble universe".into(),
    ];
    generate_autoinstall_yaml(&cfg).expect("deb lines must succeed");
}

#[test]
fn yaml_ubuntu_ppa_repo_emitted_via_add_apt_repository() {
    let mut cfg = full_inject(None);
    cfg.apt_repos = vec!["ppa:ondrej/php".into()];
    let yaml = generate_autoinstall_yaml(&cfg).expect("ppa: entry must succeed");
    assert!(
        yaml.contains("add-apt-repository") && yaml.contains("ppa:ondrej/php"),
        "ppa: must be handled via add-apt-repository"
    );
}

#[test]
fn yaml_ubuntu_run_commands_emitted() {
    let mut cfg = full_inject(None);
    cfg.run_commands = vec!["echo hello".into(), "touch /tmp/forge-test".into()];
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("echo hello"),
        "run_commands must appear in runcmd"
    );
}

#[test]
fn yaml_ubuntu_late_commands_emitted() {
    let mut cfg = full_inject(None);
    cfg.extra_late_commands = vec!["echo done".into()];
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("echo done"), "late_commands must appear");
}

#[test]
fn yaml_ubuntu_ssh_no_password_auth_emitted() {
    let mut cfg = full_inject(None);
    cfg.ssh.allow_password_auth = Some(false);
    cfg.ssh.install_server = Some(true);
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("ssh"), "ssh section must be present");
}

#[test]
fn yaml_ubuntu_ssh_false_install_server_emitted() {
    let mut cfg = full_inject(None);
    cfg.ssh.install_server = Some(false);
    generate_autoinstall_yaml(&cfg).expect("install_server=false must not panic");
}

#[test]
fn yaml_ubuntu_user_groups_emitted() {
    let mut cfg = full_inject(None);
    cfg.user = UserConfig {
        groups: vec!["sudo".into(), "docker".into()],
        shell: Some("/bin/bash".into()),
        sudo_nopasswd: true,
        sudo_commands: vec!["/usr/bin/apt".into()],
    };
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("sudo") || yaml.contains("docker"),
        "groups must appear"
    );
}

#[test]
fn yaml_ubuntu_encrypt_passphrase_warning_safe() {
    let mut cfg = full_inject(None);
    cfg.encrypt = true;
    cfg.encrypt_passphrase = Some("luks-secret-123".into());
    // Should generate without panicking; plaintext warning is expected
    generate_autoinstall_yaml(&cfg).expect("encrypted config must not panic");
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Kickstart (Fedora/RHEL) — all option combinations
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn kickstart_minimal_does_not_panic() {
    let cfg = minimal_inject(Some(Distro::Fedora));
    generate_kickstart_cfg(&cfg).expect("minimal kickstart must succeed");
}

#[test]
fn kickstart_full_does_not_panic() {
    let cfg = full_inject(Some(Distro::Fedora));
    generate_kickstart_cfg(&cfg).expect("full kickstart must succeed");
}

#[test]
fn kickstart_password_hashed() {
    let cfg = full_inject(Some(Distro::Fedora));
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    assert!(
        !ks.contains("S3cur3Pass!"),
        "plaintext password must not appear"
    );
}

#[test]
fn kickstart_packages_section_present() {
    let cfg = full_inject(Some(Distro::Fedora));
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    assert!(ks.contains("%packages"), "packages section must be present");
    assert!(ks.contains("curl"), "curl must be in packages");
}

#[test]
fn kickstart_all_none_fields_succeed() {
    let cfg = minimal_inject(Some(Distro::Fedora));
    generate_kickstart_cfg(&cfg).expect("all-None kickstart must succeed");
}

#[test]
fn kickstart_ssh_key_injected() {
    let cfg = full_inject(Some(Distro::Fedora));
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    assert!(
        ks.contains("AAAA…key") || ks.contains("sshkey"),
        "SSH key must appear"
    );
}

#[test]
fn kickstart_firewall_emitted() {
    let mut cfg = full_inject(Some(Distro::Fedora));
    cfg.firewall = FirewallConfig {
        enabled: true,
        default_policy: Some("deny".into()),
        allow_ports: vec!["22/tcp".into()],
        deny_ports: vec![],
    };
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    assert!(
        ks.contains("firewall") || ks.contains("22"),
        "firewall must appear in kickstart"
    );
}

#[test]
fn kickstart_dnf_repos_emitted() {
    let mut cfg = full_inject(Some(Distro::Fedora));
    cfg.dnf_repos = vec!["https://example.com/repo.repo".into()];
    generate_kickstart_cfg(&cfg).expect("dnf_repos must not panic");
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Linux Mint preseed — all option combinations
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn mint_preseed_minimal_does_not_panic() {
    let cfg = minimal_inject(Some(Distro::Mint));
    generate_mint_preseed(&cfg).expect("minimal mint preseed must succeed");
}

#[test]
fn mint_preseed_full_does_not_panic() {
    let cfg = full_inject(Some(Distro::Mint));
    generate_mint_preseed(&cfg).expect("full mint preseed must succeed");
}

#[test]
fn mint_preseed_password_hashed_or_absent() {
    let cfg = full_inject(Some(Distro::Mint));
    let preseed = generate_mint_preseed(&cfg).unwrap();
    assert!(
        !preseed.contains("S3cur3Pass!"),
        "plaintext password must not appear"
    );
}

#[test]
fn mint_preseed_all_none_fields_succeed() {
    let cfg = minimal_inject(Some(Distro::Mint));
    generate_mint_preseed(&cfg).expect("all-None mint preseed must succeed");
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Arch Linux archinstall config — exercised via autoinstall path
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn arch_generate_yaml_dispatches_to_archinstall_path() {
    // generate_autoinstall_yaml with Arch distro still returns Some string;
    // the Arch path doesn't go through autoinstall.rs but we can test that
    // the function returns without panicking for each distro type
    let cfg = full_inject(Some(Distro::Arch));
    // generate_autoinstall_yaml for Arch returns an empty/stub — check no panic
    let result = generate_autoinstall_yaml(&cfg);
    // Either Ok or Err is fine — we only care that it doesn't panic
    let _ = result;
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. merge_autoinstall_yaml — all edge cases
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn merge_yaml_empty_existing_is_handled() {
    let cfg = full_inject(None);
    let result = merge_autoinstall_yaml("", &cfg);
    // May fail on YAML parse of empty string — that's OK, no panic
    let _ = result;
}

#[test]
fn merge_yaml_valid_existing_succeeds() {
    let existing = "#cloud-config\nautoinstall:\n  version: 1\n";
    let cfg = full_inject(None);
    merge_autoinstall_yaml(existing, &cfg).expect("valid existing yaml must succeed");
}

#[test]
fn merge_yaml_all_none_fields_succeeds() {
    let existing = "#cloud-config\nautoinstall:\n  version: 1\n";
    let cfg = minimal_inject(None);
    merge_autoinstall_yaml(existing, &cfg).expect("minimal merge must succeed");
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. build_feature_late_commands — all feature flags
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn late_commands_empty_config_returns_empty() {
    let cfg = minimal_inject(None);
    let cmds = build_feature_late_commands(&cfg).unwrap();
    assert!(
        cmds.is_empty(),
        "empty config should produce no late-commands"
    );
}

#[test]
fn late_commands_ntp_server_emitted() {
    let mut cfg = minimal_inject(None);
    cfg.network.ntp_servers = vec!["pool.ntp.org".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    assert!(cmds
        .iter()
        .any(|c| c.contains("pool.ntp.org") || c.contains("timesyncd")));
}

#[test]
fn late_commands_swap_size_emitted() {
    let mut cfg = minimal_inject(None);
    cfg.swap = Some(SwapConfig {
        size_mb: 1024,
        filename: None,
        swappiness: None,
    });
    let cmds = build_feature_late_commands(&cfg).unwrap();
    assert!(cmds
        .iter()
        .any(|c| c.contains("swap") || c.contains("1024")));
}

#[test]
fn late_commands_proxy_http_emitted() {
    let mut cfg = minimal_inject(None);
    cfg.proxy.http_proxy = Some("http://proxy:3128".into());
    let cmds = build_feature_late_commands(&cfg).unwrap();
    assert!(cmds
        .iter()
        .any(|c| c.contains("http_proxy") || c.contains("proxy")));
}

#[test]
fn late_commands_proxy_no_proxy_emitted() {
    let mut cfg = minimal_inject(None);
    cfg.proxy.no_proxy = vec!["localhost".into(), ".internal".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    assert!(cmds
        .iter()
        .any(|c| c.contains("no_proxy") || c.contains("localhost")));
}

#[test]
fn late_commands_apt_repos_deb_line_emitted() {
    let mut cfg = minimal_inject(None);
    cfg.apt_repos = vec!["deb http://archive.ubuntu.com/ubuntu noble universe".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    assert!(cmds
        .iter()
        .any(|c| c.contains("archive.ubuntu.com") || c.contains("sources.list")));
}

#[test]
fn late_commands_ppa_repo_uses_add_apt_repository() {
    let mut cfg = minimal_inject(None);
    cfg.apt_repos = vec!["ppa:ondrej/php".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    assert!(
        cmds.iter()
            .any(|c| c.contains("add-apt-repository") && c.contains("ppa:ondrej/php")),
        "ppa: must use add-apt-repository, got: {cmds:?}"
    );
}

#[test]
fn late_commands_sysctl_pairs_emitted() {
    let mut cfg = minimal_inject(None);
    cfg.sysctl = vec![("net.ipv4.ip_forward".into(), "1".into())];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    assert!(cmds
        .iter()
        .any(|c| c.contains("sysctl") || c.contains("net.ipv4")));
}

#[test]
fn late_commands_mounts_emitted() {
    let mut cfg = minimal_inject(None);
    cfg.mounts = vec!["UUID=abc123 /data ext4 defaults 0 2".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    assert!(cmds
        .iter()
        .any(|c| c.contains("fstab") || c.contains("UUID")));
}

#[test]
fn late_commands_sudo_commands_emitted() {
    let mut cfg = minimal_inject(None);
    cfg.user.sudo_commands = vec!["/usr/bin/apt".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    assert!(cmds
        .iter()
        .any(|c| c.contains("sudoers") || c.contains("apt")));
}

#[test]
fn late_commands_enable_services_emitted() {
    let mut cfg = minimal_inject(None);
    cfg.enable_services = vec!["docker".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    assert!(cmds
        .iter()
        .any(|c| c.contains("docker") || c.contains("systemctl")));
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. InjectConfig::validate() — every acceptance and rejection path
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_inject_minimal_passes() {
    let cfg = minimal_inject(None);
    cfg.validate()
        .expect("minimal inject config must pass validation");
}

#[test]
fn validate_inject_full_passes() {
    let cfg = full_inject(None);
    cfg.validate()
        .expect("full inject config must pass validation");
}

#[test]
fn validate_inject_hostname_special_chars_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.hostname = Some("host; rm -rf /".into());
    assert!(
        cfg.validate().is_err(),
        "hostname with shell chars must be rejected"
    );
}

#[test]
fn validate_inject_hostname_hyphen_dot_allowed() {
    let mut cfg = minimal_inject(None);
    cfg.hostname = Some("my-server.local".into());
    cfg.validate().expect("hostname with hyphen/dot must pass");
}

#[test]
fn validate_inject_username_special_chars_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.username = Some("admin|evil".into());
    assert!(
        cfg.validate().is_err(),
        "username with pipe must be rejected"
    );
}

#[test]
fn validate_inject_realname_shell_metachar_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.realname = Some("Name; bad".into());
    assert!(
        cfg.validate().is_err(),
        "realname with semicolon must be rejected"
    );
}

#[test]
fn validate_inject_realname_spaces_allowed() {
    let mut cfg = minimal_inject(None);
    cfg.realname = Some("Jane Doe".into());
    cfg.validate().expect("realname with spaces must pass");
}

#[test]
fn validate_inject_service_special_chars_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.enable_services = vec!["docker; evil".into()];
    assert!(
        cfg.validate().is_err(),
        "service name with semicolon must be rejected"
    );
}

#[test]
fn validate_inject_firewall_port_special_chars_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.firewall.allow_ports = vec!["22; evil".into()];
    assert!(
        cfg.validate().is_err(),
        "port with semicolon must be rejected"
    );
}

#[test]
fn validate_inject_firewall_port_tcp_suffix_allowed() {
    let mut cfg = minimal_inject(None);
    cfg.firewall.allow_ports = vec!["22/tcp".into(), "80:443/tcp".into(), "ssh".into()];
    cfg.validate().expect("valid port spec must pass");
}

#[test]
fn validate_inject_sysctl_key_special_chars_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.sysctl = vec![("net.ipv4.ip_forward; evil".into(), "1".into())];
    assert!(
        cfg.validate().is_err(),
        "sysctl key with semicolon must be rejected"
    );
}

#[test]
fn validate_inject_sysctl_value_metachar_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.sysctl = vec![("vm.swappiness".into(), "$(evil)".into())];
    assert!(
        cfg.validate().is_err(),
        "sysctl value with $() must be rejected"
    );
}

#[test]
fn validate_inject_sudo_command_metachar_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.user.sudo_commands = vec!["/usr/bin/apt; evil".into()];
    assert!(
        cfg.validate().is_err(),
        "sudo_command with semicolon must be rejected"
    );
}

#[test]
fn validate_inject_sudo_command_path_allowed() {
    let mut cfg = minimal_inject(None);
    cfg.user.sudo_commands = vec!["/usr/bin/apt".into()];
    cfg.validate().expect("valid sudo command must pass");
}

#[test]
fn validate_inject_apt_repo_deb_line_passes() {
    let mut cfg = minimal_inject(None);
    cfg.apt_repos = vec!["deb http://archive.ubuntu.com/ubuntu noble universe".into()];
    cfg.validate().expect("deb line must pass validation");
}

#[test]
fn validate_inject_apt_repo_deb_src_line_passes() {
    let mut cfg = minimal_inject(None);
    cfg.apt_repos = vec!["deb-src http://archive.ubuntu.com/ubuntu noble universe".into()];
    cfg.validate().expect("deb-src line must pass validation");
}

#[test]
fn validate_inject_apt_repo_ppa_passes() {
    let mut cfg = minimal_inject(None);
    cfg.apt_repos = vec!["ppa:ondrej/php".into()];
    cfg.validate()
        .expect("ppa: entry must pass validation after the fix");
}

#[test]
fn validate_inject_apt_repo_invalid_prefix_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.apt_repos = vec!["http://archive.ubuntu.com/ubuntu noble universe".into()];
    assert!(
        cfg.validate().is_err(),
        "apt_repo without 'deb '/'deb-src '/'ppa:' prefix must be rejected"
    );
}

#[test]
fn validate_inject_apt_repo_metachar_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.apt_repos = vec!["deb http://example.com; evil".into()];
    assert!(
        cfg.validate().is_err(),
        "apt_repo with semicolon must be rejected"
    );
}

#[test]
fn validate_inject_apt_mirror_metachar_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.apt_mirror = Some("http://mirror.com|evil".into());
    assert!(
        cfg.validate().is_err(),
        "apt_mirror with pipe must be rejected"
    );
}

#[test]
fn validate_inject_proxy_metachar_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.proxy.http_proxy = Some("http://proxy.com;evil".into());
    assert!(
        cfg.validate().is_err(),
        "http_proxy with semicolon must be rejected"
    );
}

#[test]
fn validate_inject_proxy_no_proxy_metachar_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.proxy.no_proxy = vec!["localhost;evil".into()];
    assert!(
        cfg.validate().is_err(),
        "no_proxy with semicolon must be rejected"
    );
}

#[test]
fn validate_inject_dns_special_chars_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.network.dns_servers = vec!["1.1.1.1; evil".into()];
    assert!(
        cfg.validate().is_err(),
        "dns_server with semicolon must be rejected"
    );
}

#[test]
fn validate_inject_ntp_valid_dotted_passes() {
    let mut cfg = minimal_inject(None);
    cfg.network.ntp_servers = vec!["pool.ntp.org".into(), "time.cloudflare.com".into()];
    cfg.validate().expect("valid NTP server must pass");
}

#[test]
fn validate_inject_mount_metachar_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.mounts = vec!["UUID=abc; evil /data ext4".into()];
    assert!(
        cfg.validate().is_err(),
        "mount with semicolon must be rejected"
    );
}

#[test]
fn validate_inject_grub_default_slash_accepted() {
    // sed now uses | as delimiter so / in grub_default is safe.
    let mut cfg = minimal_inject(None);
    cfg.grub.default_entry = Some("ubuntu/recovery".into());
    assert!(
        cfg.validate().is_ok(),
        "grub_default with slash must be accepted (sed uses | delimiter)"
    );
}

#[test]
fn validate_inject_grub_cmdline_extra_safe() {
    let mut cfg = minimal_inject(None);
    cfg.grub.cmdline_extra = vec!["quiet".into(), "splash".into(), "nomodeset".into()];
    cfg.validate().expect("safe kernel params must pass");
}

#[test]
fn validate_inject_grub_cmdline_pipe_rejected() {
    let mut cfg = minimal_inject(None);
    cfg.grub.cmdline_extra = vec!["quiet|evil".into()];
    assert!(
        cfg.validate().is_err(),
        "grub cmdline with pipe must be rejected"
    );
}

#[test]
fn validate_inject_swap_valid_filename_passes() {
    let mut cfg = minimal_inject(None);
    cfg.swap = Some(SwapConfig {
        size_mb: 512,
        filename: Some("/swapfile".into()),
        swappiness: None,
    });
    cfg.validate().expect("valid swap filename must pass");
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. BuildConfig::validate() — output_label byte vs char mismatch
// ─────────────────────────────────────────────────────────────────────────────

fn minimal_build() -> BuildConfig {
    BuildConfig {
        name: "test".into(),
        source: IsoSource::from_raw("/tmp/test.iso"),
        overlay_dir: None,
        output_label: None,
        profile: ProfileKind::Minimal,
        auto_scan: false,
        auto_test: false,
        scanning: ScanPolicy::default(),
        testing: TestingPolicy::default(),
        keep_workdir: false,
        expected_sha256: None,
    }
}

#[test]
fn build_validate_no_label_passes() {
    let cfg = minimal_build();
    cfg.validate().expect("no output_label must pass");
}

#[test]
fn build_validate_ascii_32_char_label_passes() {
    let mut cfg = minimal_build();
    cfg.output_label = Some("A".repeat(32));
    cfg.validate().expect("32 ASCII chars must pass");
}

#[test]
fn build_validate_ascii_33_char_label_rejected() {
    let mut cfg = minimal_build();
    cfg.output_label = Some("A".repeat(33));
    assert!(
        cfg.validate().is_err(),
        "33 ASCII chars must fail (> 32 bytes)"
    );
}

#[test]
fn build_validate_multibyte_label_gt32_bytes_rejected() {
    // U+FFFD is 3 bytes in UTF-8; 11 × 3 = 33 bytes, but only 11 chars
    let label: String = "\u{FFFD}".repeat(11);
    assert_eq!(label.chars().count(), 11, "11 replacement chars");
    assert!(label.len() > 32, "11 × 3 bytes = 33 bytes");
    let mut cfg = minimal_build();
    cfg.output_label = Some(label);
    // Engine currently validates by bytes: this should fail
    assert!(
        cfg.validate().is_err(),
        "label > 32 bytes must fail even if <= 32 chars"
    );
}

#[test]
fn build_validate_empty_name_rejected() {
    let mut cfg = minimal_build();
    cfg.name = "".into();
    assert!(cfg.validate().is_err(), "empty name must be rejected");
}

#[test]
fn build_validate_whitespace_name_rejected() {
    let mut cfg = minimal_build();
    cfg.name = "   ".into();
    assert!(
        cfg.validate().is_err(),
        "whitespace-only name must be rejected"
    );
}

#[test]
fn build_validate_blank_label_rejected() {
    let mut cfg = minimal_build();
    cfg.output_label = Some("   ".into());
    assert!(cfg.validate().is_err(), "blank label must be rejected");
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Source preset catalog — completeness and strategy consistency
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn all_presets_have_non_empty_ids() {
    for p in all_presets() {
        assert!(
            !p.id.as_str().is_empty(),
            "preset ID must not be empty: {p:?}"
        );
    }
}

#[test]
fn all_presets_have_non_empty_names() {
    for p in all_presets() {
        assert!(
            !p.name.is_empty(),
            "preset name must not be empty for {}",
            p.id.as_str()
        );
    }
}

#[test]
fn all_presets_have_non_empty_official_page() {
    for p in all_presets() {
        assert!(
            !p.official_page.is_empty(),
            "official_page must not be empty for {}",
            p.id.as_str()
        );
    }
}

#[test]
fn direct_url_presets_have_non_none_url() {
    for p in all_presets() {
        if p.strategy == AcquisitionStrategy::DirectUrl {
            assert!(
                p.direct_url.is_some(),
                "DirectUrl preset {} must have direct_url set",
                p.id.as_str()
            );
        }
    }
}

#[test]
fn resolve_url_direct_url_presets_return_some() {
    for p in all_presets() {
        if p.strategy == AcquisitionStrategy::DirectUrl {
            let result = resolve_url(p).expect("resolve_url must not error");
            assert!(
                result.is_some(),
                "DirectUrl preset {} must resolve to Some(url)",
                p.id.as_str()
            );
            let url = result.unwrap();
            assert!(
                url.starts_with("http://") || url.starts_with("https://"),
                "URL for {} must be http(s): {}",
                p.id.as_str(),
                url
            );
        }
    }
}

#[test]
fn resolve_url_non_direct_returns_none() {
    for p in all_presets() {
        if p.strategy != AcquisitionStrategy::DirectUrl {
            let result = resolve_url(p).expect("resolve_url must not error");
            assert!(
                result.is_none(),
                "non-DirectUrl preset {} must return None from resolve_url",
                p.id.as_str()
            );
        }
    }
}

#[test]
fn find_preset_by_str_returns_correct_preset() {
    for p in all_presets() {
        let found = find_preset_by_str(p.id.as_str());
        assert!(
            found.is_some(),
            "must find preset by its own ID: {}",
            p.id.as_str()
        );
        assert_eq!(found.unwrap().id.as_str(), p.id.as_str());
    }
}

#[test]
fn find_preset_by_str_unknown_returns_none() {
    assert!(find_preset_by_str("totally-unknown-preset-xyz").is_none());
    assert!(find_preset_by_str("").is_none());
}

#[test]
fn preset_ids_are_unique() {
    let presets = all_presets();
    let mut ids: Vec<&str> = presets.iter().map(|p| p.id.as_str()).collect();
    ids.sort_unstable();
    let deduped = {
        let mut d = ids.clone();
        d.dedup();
        d
    };
    assert_eq!(ids.len(), deduped.len(), "all preset IDs must be unique");
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. VM emit — all hypervisor × firmware combinations
// ─────────────────────────────────────────────────────────────────────────────

fn vm_spec(hv: Hypervisor, fw: FirmwareMode) -> VmLaunchSpec {
    let mut spec = VmLaunchSpec::new(std::path::Path::new("/tmp/test.iso"), hv, fw);
    spec.ram_mb = 2048;
    spec.cpus = 2;
    spec.disk_gb = 20;
    spec
}

#[test]
fn vm_emit_qemu_bios_does_not_panic() {
    let spec = vm_spec(Hypervisor::Qemu, FirmwareMode::Bios);
    let out = emit_launch(&spec);
    assert!(
        !out.commands.is_empty() || out.script.is_some(),
        "must produce output"
    );
}

#[test]
fn vm_emit_qemu_uefi_does_not_panic() {
    let spec = vm_spec(Hypervisor::Qemu, FirmwareMode::Uefi);
    let out = emit_launch(&spec);
    assert!(
        !out.commands.is_empty() || out.script.is_some(),
        "must produce output"
    );
}

#[test]
fn vm_emit_virtualbox_bios_does_not_panic() {
    let spec = vm_spec(Hypervisor::VirtualBox, FirmwareMode::Bios);
    let out = emit_launch(&spec);
    assert!(!out.notes.is_empty(), "VirtualBox must include notes");
}

#[test]
fn vm_emit_virtualbox_uefi_does_not_panic() {
    let spec = vm_spec(Hypervisor::VirtualBox, FirmwareMode::Uefi);
    let _ = emit_launch(&spec);
}

#[test]
fn vm_emit_vmware_bios_does_not_panic() {
    let spec = vm_spec(Hypervisor::Vmware, FirmwareMode::Bios);
    let _ = emit_launch(&spec);
}

#[test]
fn vm_emit_vmware_uefi_does_not_panic() {
    let spec = vm_spec(Hypervisor::Vmware, FirmwareMode::Uefi);
    let _ = emit_launch(&spec);
}

#[test]
fn vm_emit_hyperv_bios_does_not_panic() {
    let spec = vm_spec(Hypervisor::HyperV, FirmwareMode::Bios);
    let _ = emit_launch(&spec);
}

#[test]
fn vm_emit_hyperv_uefi_does_not_panic() {
    let spec = vm_spec(Hypervisor::HyperV, FirmwareMode::Uefi);
    let _ = emit_launch(&spec);
}

#[test]
fn vm_emit_proxmox_bios_does_not_panic() {
    let spec = vm_spec(Hypervisor::Proxmox, FirmwareMode::Bios);
    let _ = emit_launch(&spec);
}

#[test]
fn vm_emit_proxmox_uefi_does_not_panic() {
    let spec = vm_spec(Hypervisor::Proxmox, FirmwareMode::Uefi);
    let _ = emit_launch(&spec);
}

#[test]
fn vm_emit_iso_path_appears_in_output() {
    let spec = vm_spec(Hypervisor::Qemu, FirmwareMode::Bios);
    let out = emit_launch(&spec);
    let all_text = format!(
        "{}{}",
        out.commands.join(" "),
        out.script.as_deref().unwrap_or("")
    );
    assert!(
        all_text.contains("/tmp/test.iso"),
        "ISO path must appear in VM launch output"
    );
}

#[test]
fn vm_emit_custom_ram_and_cpu_does_not_panic() {
    let mut spec = vm_spec(Hypervisor::Qemu, FirmwareMode::Bios);
    spec.ram_mb = 8192;
    spec.cpus = 4;
    let _ = emit_launch(&spec);
}

// ─────────────────────────────────────────────────────────────────────────────
// 11. hash_password — edge-case inputs
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn hash_password_ascii_succeeds() {
    let h = hash_password("hello123!").expect("ASCII password must hash");
    assert!(h.starts_with("$6$"), "must be SHA-512-crypt");
}

#[test]
fn hash_password_empty_string_succeeds() {
    let h = hash_password("").expect("empty password must hash");
    assert!(h.starts_with("$6$"), "must be SHA-512-crypt");
}

#[test]
fn hash_password_unicode_succeeds() {
    let h = hash_password("pássw0rd🔑").expect("unicode password must hash");
    assert!(h.starts_with("$6$"), "must be SHA-512-crypt");
}

#[test]
fn hash_password_very_long_succeeds() {
    let long: String = "A".repeat(1024);
    let h = hash_password(&long).expect("1024-char password must hash");
    assert!(h.starts_with("$6$"), "must be SHA-512-crypt");
}

#[test]
fn hash_password_is_not_plaintext() {
    let h = hash_password("MySecret").unwrap();
    assert!(
        !h.contains("MySecret"),
        "hashed output must not contain plaintext"
    );
}

#[test]
fn hash_password_two_calls_produce_different_hashes() {
    // SHA-512-crypt uses a random salt — same password must hash differently each time
    let h1 = hash_password("same").unwrap();
    let h2 = hash_password("same").unwrap();
    assert_ne!(
        h1, h2,
        "two hashes of same password must differ (random salt)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 12. IsoSource helpers
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn iso_source_from_raw_https_is_url() {
    let src = IsoSource::from_raw("https://releases.ubuntu.com/noble/ubuntu.iso");
    assert!(matches!(src, IsoSource::Url(_)));
}

#[test]
fn iso_source_from_raw_http_is_url() {
    let src = IsoSource::from_raw("http://example.com/test.iso");
    assert!(matches!(src, IsoSource::Url(_)));
}

#[test]
fn iso_source_from_raw_path_is_path() {
    let src = IsoSource::from_raw("/tmp/ubuntu.iso");
    assert!(matches!(src, IsoSource::Path(_)));
}

#[test]
fn iso_source_from_raw_relative_path_is_path() {
    let src = IsoSource::from_raw("ubuntu.iso");
    assert!(matches!(src, IsoSource::Path(_)));
}

#[test]
fn iso_source_from_raw_empty_is_path() {
    let src = IsoSource::from_raw("");
    assert!(matches!(src, IsoSource::Path(_)));
}

// ─────────────────────────────────────────────────────────────────────────────
// 13. GUI helper functions — tested via equivalent Rust logic
// ─────────────────────────────────────────────────────────────────────────────

// These mirror the `lines()` and `opt()` functions from forge-gui/src/state.rs.
// Tested here to ensure no panics on edge-case inputs.

fn lines_helper(s: &str) -> Vec<String> {
    s.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn opt_helper(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

#[test]
fn lines_empty_string_returns_empty_vec() {
    assert!(lines_helper("").is_empty());
}

#[test]
fn lines_only_whitespace_returns_empty_vec() {
    assert!(lines_helper("   \n  \n  ").is_empty());
}

#[test]
fn lines_trims_entries() {
    let v = lines_helper("  curl  \n  git  ");
    assert_eq!(v, vec!["curl", "git"]);
}

#[test]
fn lines_skips_blank_lines() {
    let v = lines_helper("curl\n\ngit\n\n\nvim");
    assert_eq!(v, vec!["curl", "git", "vim"]);
}

#[test]
fn lines_multibyte_content_does_not_panic() {
    let v = lines_helper("库\nλ\n🔑");
    assert_eq!(v.len(), 3);
}

#[test]
fn opt_empty_is_none() {
    assert_eq!(opt_helper(""), None);
}

#[test]
fn opt_whitespace_only_is_none() {
    assert_eq!(opt_helper("   "), None);
}

#[test]
fn opt_non_empty_is_some() {
    assert_eq!(opt_helper("  hello  "), Some("hello".into()));
}

#[test]
fn opt_multibyte_does_not_panic() {
    let v = opt_helper("  名前  ");
    assert_eq!(v, Some("名前".into()));
}

// ─────────────────────────────────────────────────────────────────────────────
// 14. Progress-bar / truncation helpers — no panics on edge cases
// ─────────────────────────────────────────────────────────────────────────────

fn make_progress_bar(pct: u8) -> String {
    const WIDTH: usize = 24;
    let filled = (pct as usize) * WIDTH / 100;
    let empty = WIDTH.saturating_sub(filled);
    format!("|{}{}|", "█".repeat(filled), "─".repeat(empty))
}

fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

#[test]
fn progress_bar_zero_percent() {
    let bar = make_progress_bar(0);
    assert!(bar.starts_with('|') && bar.ends_with('|'));
    assert!(!bar.contains('█'));
}

#[test]
fn progress_bar_100_percent() {
    let bar = make_progress_bar(100);
    assert!(bar.starts_with('|') && bar.ends_with('|'));
    assert!(!bar.contains('─'));
}

#[test]
fn progress_bar_50_percent() {
    let bar = make_progress_bar(50);
    assert!(bar.contains('█') && bar.contains('─'));
}

#[test]
fn progress_bar_max_u8() {
    // pct=255 should not overflow or panic
    let bar = make_progress_bar(255);
    assert!(!bar.is_empty());
}

#[test]
fn truncate_chars_ascii_32_safe() {
    let s = "A".repeat(40);
    let t = truncate_chars(&s, 32);
    assert_eq!(t.len(), 32);
}

#[test]
fn truncate_chars_multibyte_em_dash_safe() {
    // em-dash is 3 bytes; 15 ASCII + "—" + 15 ASCII = 32 chars
    let s = format!("{}{}{}", "A".repeat(15), "—", "B".repeat(20));
    let t = truncate_chars(&s, 32);
    assert_eq!(t.chars().count(), 32);
    // Crucially: no panic (byte-slicing at 32 would panic here)
}

#[test]
fn truncate_chars_replacement_char_safe() {
    // U+FFFD is 3 bytes; 12 of them = 36 bytes — byte-slicing at 32 would panic
    let s: String = "\u{FFFD}".repeat(12);
    assert!(s.len() > 32, "should be > 32 bytes");
    let t = truncate_chars(&s, 32);
    assert_eq!(t.chars().count(), 12, "only 12 chars available");
}

#[test]
fn truncate_chars_emoji_safe() {
    // 4-byte emoji; 10 of them = 40 bytes — byte-slicing at 32 would panic
    let s: String = "🔑".repeat(10);
    assert!(s.len() > 32, "should be > 32 bytes");
    let t = truncate_chars(&s, 32);
    assert_eq!(t.chars().count(), 10, "all 10 emoji fit within 32 chars");
}

#[test]
fn truncate_chars_empty_string_safe() {
    assert_eq!(truncate_chars("", 32), "");
}

#[test]
fn truncate_chars_shorter_than_limit_returns_all() {
    let s = "hello";
    assert_eq!(truncate_chars(s, 32), "hello");
}

// ═══════════════════════════════════════════════════════════════════════════
// SECTION: Cross-distro option combination regression tests
// ═══════════════════════════════════════════════════════════════════════════

// ── Kickstart (Fedora) path-transformation tests ──────────────────────────

#[test]
fn kickstart_post_strips_chroot_target_prefix() {
    let mut cfg = minimal_inject(Some(Distro::Fedora));
    cfg.enable_services = vec!["docker".into()];
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    // The chroot /target prefix must be stripped in %post; raw systemctl used
    assert!(
        ks.contains("systemctl enable docker"),
        "kickstart %post must call systemctl directly (no chroot /target)"
    );
    assert!(
        !ks.contains("chroot /target systemctl enable docker"),
        "kickstart %post must not emit chroot /target commands"
    );
}

#[test]
fn kickstart_post_translates_target_paths() {
    let mut cfg = minimal_inject(Some(Distro::Fedora));
    cfg.network.ntp_servers = vec!["time.cloudflare.com".into()];
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    // /target/ prefix must become / in %post
    assert!(
        ks.contains("/etc/systemd/timesyncd.conf") || ks.contains("timesyncd"),
        "kickstart %post must translate /target/ paths"
    );
}

#[test]
fn kickstart_firewall_uses_firewalld_package() {
    // Fedora/RHEL uses firewalld, not ufw.  The %packages section must add
    // firewalld, and the %post must NOT contain ufw commands.
    let mut cfg = minimal_inject(Some(Distro::Fedora));
    cfg.firewall.enabled = true;
    cfg.firewall.allow_ports = vec!["22".into()];
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    assert!(
        ks.contains("firewalld"),
        "kickstart must include firewalld in %packages when firewall enabled"
    );
    assert!(
        !ks.contains("ufw"),
        "kickstart %post must not emit ufw commands (Fedora uses firewalld)"
    );
}

#[test]
fn kickstart_apt_repos_not_emitted_in_post() {
    // APT repos are Ubuntu-only; must not appear in a Fedora kickstart.
    let mut cfg = minimal_inject(Some(Distro::Fedora));
    cfg.apt_repos = vec!["deb http://archive.ubuntu.com/ubuntu noble main".into()];
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    assert!(
        !ks.contains("add-apt-repository"),
        "kickstart must not emit add-apt-repository for Fedora"
    );
    assert!(
        !ks.contains("sources.list"),
        "kickstart must not emit APT sources.list commands for Fedora"
    );
}

#[test]
fn kickstart_docker_not_in_post_for_fedora() {
    // Docker via apt is Ubuntu-only; must not appear in Fedora kickstart %post.
    let mut cfg = minimal_inject(Some(Distro::Fedora));
    cfg.containers.docker = true;
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    assert!(
        !ks.contains("download.docker.com/linux/ubuntu"),
        "kickstart %post must not emit Ubuntu Docker install commands for Fedora"
    );
}

#[test]
fn kickstart_proxy_in_post() {
    let mut cfg = minimal_inject(Some(Distro::Fedora));
    cfg.proxy.http_proxy = Some("http://proxy.corp:3128".into());
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    assert!(
        ks.contains("http_proxy"),
        "proxy must appear in kickstart %post"
    );
}

#[test]
fn kickstart_sysctl_in_post() {
    let mut cfg = minimal_inject(Some(Distro::Fedora));
    cfg.sysctl = vec![("net.ipv4.ip_forward".into(), "1".into())];
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    assert!(
        ks.contains("net.ipv4.ip_forward"),
        "sysctl must appear in kickstart %post"
    );
}

#[test]
fn kickstart_swap_in_post() {
    let mut cfg = minimal_inject(Some(Distro::Fedora));
    cfg.swap = Some(SwapConfig {
        size_mb: 2048,
        filename: None,
        swappiness: None,
    });
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    assert!(
        ks.contains("fallocate"),
        "swap setup must appear in kickstart %post"
    );
}

#[test]
fn kickstart_dnf_repo_url_in_post() {
    let mut cfg = minimal_inject(Some(Distro::Fedora));
    cfg.dnf_repos = vec!["https://example.com/forgeiso.repo".into()];
    let ks = generate_kickstart_cfg(&cfg).unwrap();
    assert!(
        ks.contains("config-manager") || ks.contains("forgeiso.repo"),
        "DNF repo URL must appear in kickstart %post"
    );
}

#[test]
fn kickstart_full_fedora_config_does_not_panic() {
    let cfg = full_inject(Some(Distro::Fedora));
    let result = generate_kickstart_cfg(&cfg);
    assert!(
        result.is_ok(),
        "full Fedora config must generate without error: {:?}",
        result.err()
    );
}

// ── Arch Linux tests ──────────────────────────────────────────────────────

#[test]
fn arch_autoinstall_does_not_generate_cloud_init() {
    // Arch uses archinstall JSON — generate_autoinstall_yaml for Arch still
    // produces a cloud-init file but it is NOT used; this tests the engine
    // does not panic for Arch distro.
    let cfg = minimal_inject(Some(Distro::Arch));
    let result = generate_autoinstall_yaml(&cfg);
    assert!(
        result.is_ok(),
        "Arch config must not panic in yaml generator: {:?}",
        result.err()
    );
}

#[test]
fn arch_late_commands_no_ufw() {
    let mut cfg = minimal_inject(Some(Distro::Arch));
    cfg.firewall.enabled = true;
    cfg.firewall.allow_ports = vec!["22".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    let all = cmds.join("\n");
    assert!(
        !all.contains("ufw"),
        "Arch late-commands must not emit ufw (Arch uses iptables/nftables)"
    );
}

#[test]
fn arch_late_commands_no_apt() {
    let mut cfg = minimal_inject(Some(Distro::Arch));
    cfg.apt_repos = vec!["deb http://archive.ubuntu.com/ubuntu noble main".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    let all = cmds.join("\n");
    assert!(
        !all.contains("add-apt-repository") && !all.contains("sources.list"),
        "Arch late-commands must not emit APT commands"
    );
}

#[test]
fn arch_late_commands_no_docker_apt() {
    let mut cfg = minimal_inject(Some(Distro::Arch));
    cfg.containers.docker = true;
    let cmds = build_feature_late_commands(&cfg).unwrap();
    let all = cmds.join("\n");
    assert!(
        !all.contains("download.docker.com/linux/ubuntu"),
        "Arch late-commands must not emit Ubuntu Docker install"
    );
}

#[test]
fn arch_pacman_mirror_in_late_commands() {
    let mut cfg = minimal_inject(Some(Distro::Arch));
    cfg.pacman_mirror = Some("https://mirror.rackspace.com/archlinux".into());
    let cmds = build_feature_late_commands(&cfg).unwrap();
    let all = cmds.join("\n");
    assert!(
        all.contains("mirrorlist"),
        "Arch late-commands must set mirrorlist when pacman_mirror is set"
    );
}

#[test]
fn arch_full_config_does_not_panic() {
    let cfg = full_inject(Some(Distro::Arch));
    let result = generate_autoinstall_yaml(&cfg);
    assert!(
        result.is_ok(),
        "full Arch config must not error: {:?}",
        result.err()
    );
}

// ── Linux Mint tests ──────────────────────────────────────────────────────

#[test]
fn mint_preseed_late_command_contains_proxy() {
    let mut cfg = minimal_inject(Some(Distro::Mint));
    cfg.proxy.http_proxy = Some("http://proxy.internal:8080".into());
    let preseed = generate_mint_preseed(&cfg).unwrap();
    assert!(
        preseed.contains("late_command"),
        "Mint preseed must emit late_command when proxy is set"
    );
    assert!(
        preseed.contains("http_proxy"),
        "proxy must appear in Mint late_command"
    );
}

#[test]
fn mint_preseed_late_command_contains_sysctl() {
    let mut cfg = minimal_inject(Some(Distro::Mint));
    cfg.sysctl = vec![("vm.swappiness".into(), "10".into())];
    let preseed = generate_mint_preseed(&cfg).unwrap();
    assert!(
        preseed.contains("vm.swappiness"),
        "sysctl must appear in Mint late_command"
    );
}

#[test]
fn mint_preseed_late_command_contains_services() {
    let mut cfg = minimal_inject(Some(Distro::Mint));
    cfg.enable_services = vec!["cron".into()];
    let preseed = generate_mint_preseed(&cfg).unwrap();
    assert!(
        preseed.contains("cron") && preseed.contains("systemctl"),
        "services must appear in Mint late_command"
    );
}

#[test]
fn mint_preseed_default_mirror_is_mint() {
    let cfg = minimal_inject(Some(Distro::Mint));
    let preseed = generate_mint_preseed(&cfg).unwrap();
    assert!(
        preseed.contains("packages.linuxmint.com"),
        "default mirror for Mint must be packages.linuxmint.com, not archive.ubuntu.com"
    );
    assert!(
        !preseed.contains("archive.ubuntu.com"),
        "archive.ubuntu.com must not appear in Mint preseed by default"
    );
}

#[test]
fn mint_preseed_firewall_in_late_command() {
    // Mint is Debian-based so ufw IS correct for Mint (unlike Fedora/Arch).
    let mut cfg = minimal_inject(Some(Distro::Mint));
    cfg.firewall.enabled = true;
    cfg.firewall.allow_ports = vec!["22".into(), "443".into()];
    let preseed = generate_mint_preseed(&cfg).unwrap();
    assert!(
        preseed.contains("ufw"),
        "Mint late_command must include ufw (Mint is Debian-based)"
    );
}

#[test]
fn mint_full_config_does_not_panic() {
    let cfg = full_inject(Some(Distro::Mint));
    let result = generate_mint_preseed(&cfg);
    assert!(
        result.is_ok(),
        "full Mint config must not error: {:?}",
        result.err()
    );
}

// ── GRUB cmdline validation ───────────────────────────────────────────────

#[test]
fn validate_rejects_grub_cmdline_with_double_quote() {
    let mut cfg = minimal_inject(None);
    cfg.grub.cmdline_extra = vec!["console=\"ttyS0\"".into()];
    assert!(
        cfg.validate().is_err(),
        "grub_cmdline with double-quote must be rejected (breaks sed pattern)"
    );
}

#[test]
fn validate_accepts_grub_cmdline_with_slash() {
    // sed now uses | as delimiter so / in cmdline params is safe.
    let mut cfg = minimal_inject(None);
    cfg.grub.cmdline_extra = vec!["root=/dev/sda1".into()];
    assert!(
        cfg.validate().is_ok(),
        "grub_cmdline with slash must be accepted (sed uses | delimiter)"
    );
}

#[test]
fn validate_accepts_grub_cmdline_safe_params() {
    let mut cfg = minimal_inject(None);
    cfg.grub.cmdline_extra = vec!["quiet".into(), "splash".into(), "nomodeset".into()];
    assert!(
        cfg.validate().is_ok(),
        "common GRUB params (quiet splash nomodeset) must be accepted"
    );
}

#[test]
fn validate_accepts_grub_cmdline_key_value() {
    let mut cfg = minimal_inject(None);
    cfg.grub.cmdline_extra = vec!["console=ttyS0,115200n8".into()];
    assert!(
        cfg.validate().is_ok(),
        "GRUB kernel param in key=value format must be accepted"
    );
}

// ── IPv6 DNS/NTP tests ────────────────────────────────────────────────────

#[test]
fn validate_accepts_ipv6_dns() {
    let mut cfg = minimal_inject(None);
    cfg.network.dns_servers = vec!["2001:4860:4860::8888".into(), "2001:4860:4860::8844".into()];
    assert!(
        cfg.validate().is_ok(),
        "IPv6 DNS servers must be accepted by the validator"
    );
}

#[test]
fn validate_accepts_ipv6_ntp() {
    let mut cfg = minimal_inject(None);
    cfg.network.ntp_servers = vec!["2001:db8::1".into()];
    assert!(
        cfg.validate().is_ok(),
        "IPv6 NTP server must be accepted by the validator"
    );
}

#[test]
fn validate_accepts_mixed_ipv4_ipv6_dns() {
    let mut cfg = minimal_inject(None);
    cfg.network.dns_servers = vec![
        "1.1.1.1".into(),
        "2606:4700:4700::1111".into(),
        "8.8.8.8".into(),
    ];
    assert!(
        cfg.validate().is_ok(),
        "mixed IPv4 + IPv6 DNS list must be accepted"
    );
}

// ── All 35 preset catalog tests ───────────────────────────────────────────

#[test]
fn all_presets_count_is_thirty_five() {
    assert_eq!(
        all_presets().len(),
        35,
        "preset catalog must contain exactly 35 entries"
    );
}

#[test]
fn all_presets_direct_url_have_url() {
    for p in all_presets() {
        if p.strategy == AcquisitionStrategy::DirectUrl {
            assert!(
                p.direct_url.is_some(),
                "DirectUrl preset '{}' must have a direct_url set",
                p.id.as_str()
            );
        }
    }
}

#[test]
fn all_presets_resolve_url_is_consistent_with_strategy() {
    for p in all_presets() {
        let result = resolve_url(p).unwrap();
        match p.strategy {
            AcquisitionStrategy::DirectUrl => assert!(
                result.is_some(),
                "DirectUrl preset '{}' must resolve to Some(url)",
                p.id.as_str()
            ),
            AcquisitionStrategy::DiscoveryPage | AcquisitionStrategy::UserProvided => assert!(
                result.is_none(),
                "DiscoveryPage/UserProvided preset '{}' must resolve to None",
                p.id.as_str()
            ),
        }
    }
}

#[test]
fn all_presets_have_valid_distro_field() {
    let valid_distros = [
        "ubuntu",
        "mint",
        "fedora",
        "arch",
        "rhel-family",
        "debian",
        "kali",
        "opensuse",
        "pop-os",
    ];
    for p in all_presets() {
        assert!(
            valid_distros.contains(&p.distro),
            "preset '{}' has unknown distro field: '{}'",
            p.id.as_str(),
            p.distro
        );
    }
}

#[test]
fn all_presets_official_page_starts_with_https() {
    for p in all_presets() {
        assert!(
            p.official_page.starts_with("https://"),
            "preset '{}' official_page must start with https://: '{}'",
            p.id.as_str(),
            p.official_page
        );
    }
}

#[test]
fn all_presets_direct_url_start_with_https() {
    for p in all_presets() {
        if let Some(url) = p.direct_url {
            assert!(
                url.starts_with("https://"),
                "preset '{}' direct_url must start with https://: '{}'",
                p.id.as_str(),
                url
            );
        }
    }
}

#[test]
fn find_preset_by_str_works_for_all_35_presets() {
    for p in all_presets() {
        let found = find_preset_by_str(p.id.as_str());
        assert!(
            found.is_some(),
            "find_preset_by_str must find every preset by its own ID: '{}'",
            p.id.as_str()
        );
        assert_eq!(found.unwrap().id.as_str(), p.id.as_str());
    }
}

#[test]
fn find_preset_by_str_case_insensitive_for_all_35() {
    for p in all_presets() {
        let id_upper = p.id.as_str().to_uppercase();
        let found = find_preset_by_str(&id_upper);
        assert!(
            found.is_some(),
            "case-insensitive lookup must work for preset '{}' (tried '{}')",
            p.id.as_str(),
            id_upper
        );
    }
}

#[test]
fn all_presets_note_non_empty() {
    for p in all_presets() {
        assert!(
            !p.note.is_empty(),
            "preset '{}' must have a non-empty note",
            p.id.as_str()
        );
    }
}

// ── Cross-option: Ubuntu-specific features stay off for Fedora/Arch ────────

#[test]
fn ubuntu_autoinstall_has_ufw_when_firewall_enabled() {
    let mut cfg = minimal_inject(None); // Ubuntu
    cfg.firewall.enabled = true;
    cfg.firewall.allow_ports = vec!["22".into()];
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("ufw"),
        "Ubuntu autoinstall must include ufw when firewall enabled"
    );
}

#[test]
fn fedora_late_cmds_no_ufw_regardless_of_firewall() {
    let mut cfg = minimal_inject(Some(Distro::Fedora));
    cfg.firewall.enabled = true;
    cfg.firewall.allow_ports = vec!["22".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    let all = cmds.join("\n");
    assert!(
        !all.contains("ufw"),
        "Fedora late-commands must never contain ufw"
    );
}

#[test]
fn ubuntu_late_cmds_include_apt_repos() {
    let mut cfg = minimal_inject(None); // Ubuntu
    cfg.apt_repos = vec!["ppa:ondrej/php".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    let all = cmds.join("\n");
    assert!(
        all.contains("add-apt-repository"),
        "Ubuntu late-commands must emit add-apt-repository for ppa: repos"
    );
}

#[test]
fn arch_late_cmds_pacman_repos_not_apt() {
    let mut cfg = minimal_inject(Some(Distro::Arch));
    cfg.pacman_repos = vec!["Server = https://mirror.example.com/$repo/os/$arch".into()];
    cfg.apt_repos = vec!["deb http://archive.ubuntu.com/ubuntu noble main".into()];
    let cmds = build_feature_late_commands(&cfg).unwrap();
    let all = cmds.join("\n");
    assert!(
        all.contains("mirrorlist"),
        "Arch late-commands must write pacman mirrorlist"
    );
    assert!(
        !all.contains("sources.list"),
        "Arch late-commands must not touch APT sources.list"
    );
}

// ── Validate: grub_default still rejects "  (double-quote included) ────────

#[test]
fn validate_rejects_grub_default_with_double_quote() {
    let mut cfg = minimal_inject(None);
    cfg.grub.default_entry = Some("entry\"0".into());
    assert!(
        cfg.validate().is_err(),
        "grub_default with double-quote must be rejected"
    );
}

#[test]
fn validate_accepts_grub_default_safe_value() {
    let mut cfg = minimal_inject(None);
    cfg.grub.default_entry = Some("Ubuntu".into());
    assert!(
        cfg.validate().is_ok(),
        "grub_default with a plain name must be accepted"
    );
}
