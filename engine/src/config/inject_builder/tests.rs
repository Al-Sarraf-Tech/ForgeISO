//! Behaviour tests for [`InjectConfigBuilder`] across all per-concern
//! setter submodules and the final `build()` validation.

use super::*;
use crate::config::components::{
    ContainerConfig, FirewallConfig, GrubConfig, NetworkConfig, ProxyConfig, SwapConfig, UserConfig,
};
use crate::config::SshConfig;

#[test]
fn builder_minimal_valid() {
    let cfg = InjectConfigBuilder::new(IsoSource::from_raw("/tmp/ubuntu.iso"), "my-custom.iso")
        .hostname("web-server")
        .username("admin")
        .build()
        .expect("minimal builder config must pass validation");

    assert_eq!(cfg.hostname.as_deref(), Some("web-server"));
    assert_eq!(cfg.username.as_deref(), Some("admin"));
    assert_eq!(cfg.out_name, "my-custom.iso");
    assert!(matches!(cfg.source, IsoSource::Path(_)));
}

#[test]
fn builder_with_ssh() {
    let ssh = SshConfig {
        authorized_keys: vec![
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIG9vbWV0aGluZw== user@host".to_string(),
        ],
        allow_password_auth: Some(false),
        install_server: Some(true),
    };
    let cfg = InjectConfigBuilder::new(IsoSource::from_raw("/tmp/ubuntu.iso"), "ssh-test.iso")
        .hostname("ssh-host")
        .username("admin")
        .ssh(ssh)
        .build()
        .expect("builder with SSH config must pass validation");

    assert_eq!(cfg.ssh.authorized_keys.len(), 1);
    assert_eq!(cfg.ssh.allow_password_auth, Some(false));
    assert_eq!(cfg.ssh.install_server, Some(true));
}

#[test]
fn builder_validation_fails_on_bad_hostname() {
    let result = InjectConfigBuilder::new(IsoSource::from_raw("/tmp/ubuntu.iso"), "bad-host.iso")
        .hostname("bad;host")
        .build();

    assert!(
        result.is_err(),
        "hostname with ';' must fail builder validation"
    );
}

// ── Scalar setter coverage ─────────────────────────────────────────────
// Each setter test exercises a single chained method and asserts the
// value reaches the InjectConfig after build(). This covers the unhit
// setter bodies in the per-concern submodules.

fn base_builder() -> InjectConfigBuilder {
    InjectConfigBuilder::new(IsoSource::from_raw("/tmp/ubuntu.iso"), "out.iso")
        .hostname("h")
        .username("u")
}

#[test]
fn builder_scalar_setters_propagate_path_and_string_fields() {
    let cfg = base_builder()
        .autoinstall_yaml(std::path::PathBuf::from("/tmp/ai.yaml"))
        .output_label("MYISO")
        .expected_sha256("a".repeat(64))
        .password("Secret123!")
        .realname("Real Name")
        .timezone("UTC")
        .locale("en_US.UTF-8")
        .keyboard_layout("us")
        .storage_layout("lvm")
        .apt_mirror("http://archive.ubuntu.com/ubuntu")
        .static_ip("10.0.0.10/24")
        .gateway("10.0.0.1")
        .dnf_mirror("http://example.com/fedora")
        .pacman_mirror("http://example.com/arch")
        .encrypt_passphrase("luks-pass")
        .build()
        .expect("scalar setters must yield a valid config");

    assert_eq!(
        cfg.autoinstall_yaml.as_deref(),
        Some(std::path::Path::new("/tmp/ai.yaml"))
    );
    assert_eq!(cfg.output_label.as_deref(), Some("MYISO"));
    assert_eq!(
        cfg.expected_sha256.as_deref(),
        Some("a".repeat(64).as_str())
    );
    assert_eq!(cfg.password.as_deref(), Some("Secret123!"));
    assert_eq!(cfg.realname.as_deref(), Some("Real Name"));
    assert_eq!(cfg.timezone.as_deref(), Some("UTC"));
    assert_eq!(cfg.locale.as_deref(), Some("en_US.UTF-8"));
    assert_eq!(cfg.keyboard_layout.as_deref(), Some("us"));
    assert_eq!(cfg.storage_layout.as_deref(), Some("lvm"));
    assert_eq!(
        cfg.apt_mirror.as_deref(),
        Some("http://archive.ubuntu.com/ubuntu")
    );
    assert_eq!(cfg.static_ip.as_deref(), Some("10.0.0.10/24"));
    assert_eq!(cfg.gateway.as_deref(), Some("10.0.0.1"));
    assert_eq!(cfg.dnf_mirror.as_deref(), Some("http://example.com/fedora"));
    assert_eq!(
        cfg.pacman_mirror.as_deref(),
        Some("http://example.com/arch")
    );
    assert_eq!(cfg.encrypt_passphrase.as_deref(), Some("luks-pass"));
}

#[test]
fn builder_vec_setters_propagate_collections() {
    let cfg = base_builder()
        .extra_packages(vec!["vim".into(), "git".into()])
        .extra_late_commands(vec!["echo done".into()])
        .enable_services(vec!["ssh".into()])
        .disable_services(vec!["telnet".into()])
        .sysctl(vec![("net.ipv4.ip_forward".into(), "1".into())])
        .apt_repos(vec!["deb http://example.com main".into()])
        .dnf_repos(vec!["[mycorp]\nname=corp".into()])
        .pacman_repos(vec!["custom-repo".into()])
        .mounts(vec!["/dev/sda1 / ext4 defaults 0 1".into()])
        .run_commands(vec!["touch /tmp/done".into()])
        .build()
        .expect("vec setters must yield a valid config");

    assert_eq!(cfg.extra_packages, vec!["vim", "git"]);
    assert_eq!(cfg.extra_late_commands, vec!["echo done"]);
    assert_eq!(cfg.enable_services, vec!["ssh"]);
    assert_eq!(cfg.disable_services, vec!["telnet"]);
    assert_eq!(cfg.sysctl.len(), 1);
    assert_eq!(cfg.sysctl[0].0, "net.ipv4.ip_forward");
    assert_eq!(cfg.apt_repos.len(), 1);
    assert_eq!(cfg.dnf_repos.len(), 1);
    assert_eq!(cfg.pacman_repos.len(), 1);
    assert_eq!(cfg.mounts.len(), 1);
    assert_eq!(cfg.run_commands, vec!["touch /tmp/done"]);
}

#[test]
fn builder_bool_setters_propagate_flags() {
    let cfg = base_builder()
        .no_user_interaction(true)
        .encrypt(true)
        .encrypt_passphrase("luks-pass") // required when encrypt is set
        .storage_layout("lvm") // required when encrypt is set
        .build()
        .expect("bool setters must yield a valid config");

    assert!(cfg.no_user_interaction);
    assert!(cfg.encrypt);
}

#[test]
fn builder_wallpaper_setter_propagates_path() {
    let dir = tempfile::tempdir().expect("tmp");
    let wp = dir.path().join("bg.png");
    std::fs::write(&wp, b"\x89PNG\r\n").expect("write wallpaper");
    let cfg = base_builder()
        .wallpaper(wp.clone())
        .build()
        .expect("wallpaper setter");
    assert_eq!(cfg.wallpaper.as_deref(), Some(wp.as_path()));
}

#[test]
fn builder_distro_setter_propagates_variant() {
    let cfg = base_builder()
        .distro(Distro::Fedora)
        .build()
        .expect("distro setter");
    assert_eq!(cfg.distro, Some(Distro::Fedora));
}

#[test]
fn builder_subconfig_setters_propagate_objects() {
    let user = UserConfig {
        groups: vec!["wheel".into()],
        shell: Some("/bin/bash".into()),
        sudo_nopasswd: true,
        sudo_commands: vec!["/usr/bin/dnf".into()],
    };
    let firewall = FirewallConfig {
        enabled: true,
        default_policy: Some("deny".into()),
        allow_ports: vec!["22/tcp".into()],
        deny_ports: vec![],
    };
    let proxy = ProxyConfig {
        http_proxy: Some("http://proxy:8080".into()),
        https_proxy: Some("http://proxy:8080".into()),
        no_proxy: vec!["localhost".into()],
    };
    let swap = SwapConfig {
        size_mb: 2048,
        filename: Some("/swap".into()),
        swappiness: Some(10),
    };
    let containers = ContainerConfig {
        docker: true,
        podman: false,
        docker_users: vec!["u".into()],
    };
    let grub = GrubConfig {
        timeout: Some(5),
        cmdline_extra: vec!["quiet".into()],
        default_entry: Some("0".into()),
    };
    let network = NetworkConfig {
        dns_servers: vec!["1.1.1.1".into()],
        ntp_servers: vec!["pool.ntp.org".into()],
    };

    let cfg = base_builder()
        .user(user.clone())
        .firewall(firewall.clone())
        .proxy(proxy.clone())
        .swap(swap.clone())
        .containers(containers.clone())
        .grub(grub.clone())
        .network(network.clone())
        .build()
        .expect("subconfig setters");

    assert_eq!(cfg.user.shell.as_deref(), Some("/bin/bash"));
    assert!(cfg.user.sudo_nopasswd);
    assert!(cfg.firewall.enabled);
    assert_eq!(cfg.firewall.default_policy.as_deref(), Some("deny"));
    assert_eq!(cfg.proxy.http_proxy.as_deref(), Some("http://proxy:8080"));
    let cfg_swap = cfg.swap.as_ref().expect("swap kept");
    assert_eq!(cfg_swap.size_mb, 2048);
    assert!(cfg.containers.docker);
    assert_eq!(cfg.grub.timeout, Some(5));
    assert_eq!(cfg.network.dns_servers, vec!["1.1.1.1"]);
}
