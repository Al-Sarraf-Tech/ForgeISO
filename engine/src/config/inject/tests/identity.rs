//! Tests for identity / locale fields (hostname, username, realname, timezone, locale, keyboard layout).
//!
//! Bodies preserved verbatim from the original `inject.rs` test module.

use super::super::*;

#[test]
fn inject_rejects_shell_metachar_in_username() {
    let cfg = InjectConfig {
        username: Some("admin; rm -rf /".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

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
fn inject_accepts_empty_string_for_validated_fields() {
    // is_safe_identifier returns Ok on empty input -- validated fields may be empty.
    let cfg = InjectConfig {
        hostname: Some(String::new()),
        username: Some(String::new()),
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "empty strings must be allowed");
}

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
