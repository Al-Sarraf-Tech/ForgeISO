use super::*;
use crate::config::{
    ContainerConfig, Distro, FirewallConfig, GrubConfig, InjectConfig, IsoSource, NetworkConfig,
    ProxyConfig, SshConfig, UserConfig,
};

#[test]
fn test_generate_minimal_yaml() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.starts_with("#cloud-config"),
        "YAML should start with #cloud-config"
    );
    assert!(
        yaml.contains("autoinstall:"),
        "YAML should contain autoinstall section"
    );
    assert!(
        yaml.contains("version: 1"),
        "YAML should contain version: 1"
    );
}

#[test]
fn run_command_containing_cloud_config_substring_is_not_filtered() {
    // Regression: the old implementation filtered lines by substring match
    // `!line.contains("cloud-config:")`. Any late-command string value whose
    // YAML serialisation contained that substring would be silently dropped,
    // producing a YAML with a missing late-command.
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        run_commands: vec!["echo 'cloud-config: done'".to_string()],
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("cloud-config: done"),
        "run_command containing 'cloud-config:' must not be filtered from YAML: {yaml}"
    );
    assert!(
        yaml.starts_with("#cloud-config"),
        "YAML must still start with #cloud-config header"
    );
}

#[test]
fn test_generate_with_identity() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: Some("test-host".to_string()),
        username: Some("testuser".to_string()),
        password: Some("testpass".to_string()),
        realname: Some("Test User".to_string()),
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("identity:"),
        "YAML should contain identity section"
    );
    assert!(yaml.contains("test-host"), "hostname should be in YAML");
    assert!(yaml.contains("testuser"), "username should be in YAML");
    assert!(yaml.contains("$6$"), "password should be hashed with $6$");
    assert!(yaml.contains("Test User"), "realname should be in YAML");
}

#[test]
fn test_generate_with_ssh_keys() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: crate::config::SshConfig {
            authorized_keys: vec![
                "ssh-ed25519 AAAA...".to_string(),
                "ssh-rsa BBBB...".to_string(),
            ],
            allow_password_auth: None,
            install_server: None,
        },
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("ssh:"), "YAML should contain ssh section");
    assert!(yaml.contains("AAAA"), "first key should be in YAML");
    assert!(yaml.contains("BBBB"), "second key should be in YAML");
    assert!(
        yaml.contains("allow-pw: false"),
        "allow-pw should be false when keys present"
    );
    // Regression: the old default was `authorized_keys.is_empty()` which
    // evaluated to `false` when keys were provided, setting install-server
    // to false and making the authorized keys unusable (no SSH daemon).
    assert!(
        yaml.contains("install-server: true"),
        "install-server must default to true even when authorized_keys are provided: {yaml}"
    );
}

#[test]
fn test_generate_with_dns() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: crate::config::NetworkConfig {
            dns_servers: vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()],
            ntp_servers: vec![],
        },
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("network:"),
        "YAML should contain network section"
    );
    assert!(yaml.contains("1.1.1.1"), "DNS 1 should be in YAML");
    assert!(yaml.contains("8.8.8.8"), "DNS 2 should be in YAML");
}

#[test]
fn test_generate_with_wallpaper() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: Some(std::path::PathBuf::from("/tmp/bg.jpg")),
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("late-commands:"),
        "YAML should contain late-commands"
    );
    assert!(
        yaml.contains("cp /cdrom/wallpaper/bg.jpg"),
        "copy command should be present"
    );
    assert!(
        yaml.contains("dconf update"),
        "dconf update should be present"
    );
    assert!(
        yaml.contains("dconf-cli"),
        "dconf-cli should be in packages"
    );
}

#[test]
fn test_merge_preserves_existing() {
    let existing = r"
autoinstall:
  version: 1
  storage:
    layout:
      name: lvm
";
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: Some("newhost".to_string()),
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let result = merge_autoinstall_yaml(existing, &cfg).unwrap();
    assert!(
        result.contains("lvm"),
        "existing storage layout should be preserved"
    );
    assert!(result.contains("newhost"), "new hostname should be present");
}

#[test]
fn test_merge_overrides_identity() {
    let existing = r"
autoinstall:
  identity:
    username: olduser
    hostname: oldhost
";
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: Some("newhost".to_string()),
        username: Some("newuser".to_string()),
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let result = merge_autoinstall_yaml(existing, &cfg).unwrap();
    assert!(result.contains("newuser"), "new username should override");
    assert!(result.contains("newhost"), "new hostname should override");
    assert!(!result.contains("olduser"), "old username should be gone");
    assert!(!result.contains("oldhost"), "old hostname should be gone");
}

#[test]
fn test_merge_appends_late_commands() {
    let existing = r#"
autoinstall:
  late-commands:
    - "echo existing"
"#;
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec!["echo new".to_string()],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let result = merge_autoinstall_yaml(existing, &cfg).unwrap();
    assert!(
        result.contains("echo existing"),
        "existing command should be preserved"
    );
    assert!(
        result.contains("echo new"),
        "new command should be appended"
    );
}

#[test]
fn test_generate_with_user_groups() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: Some("testuser".to_string()),
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: crate::config::UserConfig {
            groups: vec!["sudo".to_string(), "docker".to_string()],
            shell: None,
            sudo_nopasswd: false,
            sudo_commands: vec![],
        },
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("usermod -aG sudo,docker testuser"),
        "usermod command should add groups"
    );
}

#[test]
fn test_generate_with_sudo_nopasswd() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: Some("testuser".to_string()),
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: crate::config::UserConfig {
            groups: vec![],
            shell: None,
            sudo_nopasswd: true,
            sudo_commands: vec![],
        },
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("NOPASSWD:ALL"),
        "sudo NOPASSWD should be configured"
    );
    assert!(yaml.contains("chmod 440"), "sudoers file permissions");
}

#[test]
fn test_generate_with_firewall() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: crate::config::FirewallConfig {
            enabled: true,
            default_policy: Some("deny".to_string()),
            allow_ports: vec!["22".to_string(), "443".to_string()],
            deny_ports: vec![],
        },
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("ufw"), "firewall package should be added");
    assert!(yaml.contains("ufw --force enable"), "ufw enable command");
    assert!(yaml.contains("ufw allow 22"), "allow port 22");
}

#[test]
fn test_generate_with_static_ip() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: Some("10.0.0.5/24".to_string()),
        gateway: Some("10.0.0.1".to_string()),
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("dhcp4: false"),
        "static IP should disable DHCP"
    );
    assert!(yaml.contains("10.0.0.5/24"), "static IP should be present");
    assert!(yaml.contains("10.0.0.1"), "gateway should be present");
}

#[test]
fn test_generate_with_proxy() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: crate::config::ProxyConfig {
            http_proxy: Some("http://proxy.example.com:8080".to_string()),
            https_proxy: Some("http://proxy.example.com:8443".to_string()),
            no_proxy: vec!["localhost".to_string(), "127.0.0.1".to_string()],
        },
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("http_proxy"), "http_proxy in environment");
    assert!(yaml.contains("Acquire::http::Proxy"), "apt http proxy");
    assert!(yaml.contains("no_proxy"), "no_proxy in environment");
}

#[test]
fn test_generate_with_services() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec!["nginx".to_string()],
        disable_services: vec!["bluetooth".to_string()],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("systemctl enable nginx"), "enable nginx");
    assert!(
        yaml.contains("systemctl disable bluetooth"),
        "disable bluetooth"
    );
}

#[test]
fn test_generate_with_sysctl() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![
            ("vm.swappiness".to_string(), "10".to_string()),
            ("net.ipv4.ip_forward".to_string(), "1".to_string()),
        ],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("vm.swappiness=10"), "sysctl setting");
    assert!(
        yaml.contains("sysctl.d/99-forgeiso.conf"),
        "sysctl config file"
    );
}

#[test]
fn test_generate_with_swap() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: Some(crate::config::SwapConfig {
            size_mb: 4096,
            filename: Some("/swapfile".to_string()),
            swappiness: Some(10),
        }),
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("fallocate -l 4096M"), "swap allocation");
    assert!(yaml.contains("mkswap"), "swap mkswap");
    assert!(yaml.contains("/etc/fstab"), "fstab entry");
}

#[test]
fn test_generate_with_docker() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: Some("admin".to_string()),
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: crate::config::ContainerConfig {
            docker: true,
            podman: false,
            docker_users: vec!["admin".to_string()],
        },
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("docker-ce"), "docker packages");
    assert!(yaml.contains("download.docker.com"), "docker repo");
    assert!(yaml.contains("usermod -aG docker admin"), "docker user");
}

#[test]
fn test_generate_with_grub() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: crate::config::GrubConfig {
            timeout: Some(5),
            cmdline_extra: vec!["quiet".to_string(), "iommu=on".to_string()],
            default_entry: None,
        },
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("GRUB_TIMEOUT=5"), "grub timeout");
    assert!(yaml.contains("update-grub"), "update-grub command");
}

#[test]
fn test_generate_with_mounts() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: None,
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: vec!["/dev/sdb1 /data ext4 defaults 0 2".to_string()],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(yaml.contains("mkdir -p /target/data"), "create mount point");
    assert!(yaml.contains("/dev/sdb1 /data"), "fstab entry");
}

#[test]
fn test_generate_with_encryption() {
    let cfg = InjectConfig {
        source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
        autoinstall_yaml: None,
        out_name: "out.iso".to_string(),
        output_label: None,
        expected_sha256: None,
        hostname: None,
        username: None,
        password: None,
        realname: None,
        ssh: SshConfig::default(),
        network: NetworkConfig::default(),
        timezone: None,
        locale: None,
        keyboard_layout: None,
        storage_layout: Some("lvm".to_string()),
        apt_mirror: None,
        extra_packages: vec![],
        wallpaper: None,
        extra_late_commands: vec![],
        no_user_interaction: false,
        user: UserConfig::default(),
        firewall: FirewallConfig::default(),
        proxy: ProxyConfig::default(),
        static_ip: None,
        gateway: None,
        enable_services: vec![],
        disable_services: vec![],
        sysctl: vec![],
        swap: None,
        apt_repos: vec![],
        containers: ContainerConfig::default(),
        grub: GrubConfig::default(),
        encrypt: true,
        encrypt_passphrase: Some("secret".to_string()),
        mounts: vec![],
        run_commands: vec![],
        distro: None,
        dnf_repos: vec![],
        dnf_mirror: None,
        pacman_repos: vec![],
        pacman_mirror: None,
    };

    let yaml = generate_autoinstall_yaml(&cfg).unwrap();
    assert!(
        yaml.contains("password:"),
        "encryption password in storage section"
    );
    assert!(yaml.contains("secret"), "passphrase should be in YAML");
}

// ── merge_autoinstall_yaml edge cases ─────────────────────────────────────

#[test]
fn merge_autoinstall_yaml_with_no_autoinstall_key_creates_it() {
    // YAML that has a version key but NO autoinstall: key.
    // merge_autoinstall_yaml must create the autoinstall section rather than error.
    let bare = "version: 1\nidentity:\n  hostname: old\n";
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        hostname: Some("new-host".to_string()),
        ..Default::default()
    };
    let result = merge_autoinstall_yaml(bare, &cfg);
    assert!(result.is_ok(), "must not error on missing autoinstall key");
    let yaml = result.unwrap();
    assert!(
        yaml.contains("autoinstall"),
        "autoinstall key must be created"
    );
    assert!(yaml.contains("new-host"), "new hostname must appear");
}

#[test]
fn merge_autoinstall_yaml_with_empty_input_creates_valid_yaml() {
    // Completely empty string is valid YAML (null document).
    // The function should create a minimal autoinstall section.
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        locale: Some("en_US.UTF-8".to_string()),
        ..Default::default()
    };
    let result = merge_autoinstall_yaml("", &cfg);
    assert!(result.is_ok(), "empty YAML must not error");
}

#[test]
fn merge_autoinstall_yaml_malformed_input_returns_error() {
    // Tabs at column 0 are illegal in YAML — must return Err, not panic.
    let bad = "\t\tinvalid: [yaml\n";
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        ..Default::default()
    };
    let result = merge_autoinstall_yaml(bad, &cfg);
    assert!(result.is_err(), "malformed YAML must return an error");
}

#[test]
fn merge_autoinstall_yaml_preserves_cloud_config_header() {
    let existing = "#cloud-config\nautoinstall:\n  version: 1\n";
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        locale: Some("en_GB.UTF-8".to_string()),
        ..Default::default()
    };
    let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
    assert!(
        yaml.starts_with("#cloud-config"),
        "cloud-config header must be preserved"
    );
    assert!(yaml.contains("en_GB.UTF-8"));
}

#[test]
fn merge_autoinstall_yaml_appends_to_existing_late_commands() {
    let existing = "autoinstall:\n  version: 1\n  late-commands:\n    - echo first\n";
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        network: NetworkConfig {
            ntp_servers: vec!["time.cloudflare.com".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };
    let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
    assert!(
        yaml.contains("echo first"),
        "original late-command preserved"
    );
    assert!(yaml.contains("timesyncd"), "new NTP late-command appended");
}

#[test]
fn merge_autoinstall_yaml_deduplicates_packages() {
    // The existing YAML already contains "curl"; cfg also adds "curl".
    // After merge, "curl" must appear exactly once.
    let existing = "autoinstall:\n  version: 1\n  packages:\n    - curl\n";
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        extra_packages: vec!["curl".to_string(), "git".to_string()],
        ..Default::default()
    };
    let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
    let curl_count = yaml.matches("curl").count();
    assert_eq!(curl_count, 1, "curl must appear exactly once after dedup");
    assert!(yaml.contains("git"), "git must appear in merged packages");
}

#[test]
fn merge_autoinstall_ntp_only_does_not_inject_empty_network_block() {
    // Regression: merge_autoinstall_yaml previously entered the network block
    // when only NTP servers were configured, inserting an empty `network: {}`
    // into the YAML.  NTP goes to systemd-timesyncd via late-commands only;
    // it must not touch the netplan network block.
    let existing = "autoinstall:\n  version: 1\n";
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        network: NetworkConfig {
            ntp_servers: vec!["time.cloudflare.com".to_string()],
            dns_servers: vec![],
        },
        static_ip: None,
        ..Default::default()
    };
    let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
    assert!(
        !yaml.contains("network:"),
        "no netplan network block should appear when only NTP is configured: {yaml}"
    );
    // NTP must still appear in the late-commands section.
    assert!(
        yaml.contains("timesyncd"),
        "NTP config must still be written to late-commands: {yaml}"
    );
}

#[test]
fn apt_mirror_uses_default_arches_not_amd64() {
    // Regression: arches was hardcoded to ["amd64"], causing cloud-init to silently
    // skip the apt primary entry on arm64 and other architectures.  Must be ["default"].
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        apt_mirror: Some("http://mirror.example.com/ubuntu".to_string()),
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        yaml.contains("default"),
        "apt primary arches must be 'default', not 'amd64': {yaml}"
    );
    assert!(
        !yaml.contains("amd64"),
        "apt primary arches must not be hardcoded to 'amd64': {yaml}"
    );
}

#[test]
fn merge_apt_mirror_uses_default_arches_not_amd64() {
    // Same regression as above but exercised via merge_autoinstall_yaml.
    let existing = "autoinstall:\n  version: 1\n";
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        apt_mirror: Some("http://mirror.example.com/ubuntu".to_string()),
        ..Default::default()
    };
    let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
    assert!(
        yaml.contains("default"),
        "merged apt primary arches must be 'default', not 'amd64': {yaml}"
    );
    assert!(
        !yaml.contains("amd64"),
        "merged apt primary arches must not be hardcoded to 'amd64': {yaml}"
    );
}

#[test]
fn arch_generate_with_firewall_does_not_add_ufw_package() {
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        distro: Some(Distro::Arch),
        firewall: FirewallConfig {
            enabled: true,
            default_policy: Some("deny".to_string()),
            allow_ports: vec!["22".to_string()],
            deny_ports: vec![],
        },
        ..Default::default()
    };

    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        !yaml.contains("ufw"),
        "Arch cloud-init fallback must not inject Ubuntu-specific ufw package: {yaml}"
    );
}

#[test]
fn merge_autoinstall_yaml_deduplicates_existing_late_commands() {
    let existing = concat!(
        "autoinstall:\n",
        "  version: 1\n",
        "  late-commands:\n",
        "    - chroot /target systemctl enable systemd-timesyncd\n"
    );
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/test.iso"),
        out_name: "out.iso".to_string(),
        network: NetworkConfig {
            ntp_servers: vec!["time.cloudflare.com".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };

    let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
    assert_eq!(
        yaml.matches("chroot /target systemctl enable systemd-timesyncd")
            .count(),
        1,
        "merge must keep existing late-commands stable instead of duplicating them: {yaml}"
    );
}
