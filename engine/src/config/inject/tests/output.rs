//! Tests for output fields (out_name, expected_sha256).
//!
//! Bodies preserved verbatim from the original `inject.rs` test module.

use super::super::*;

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
