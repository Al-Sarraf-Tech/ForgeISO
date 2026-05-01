//! Tests for storage fields (swap, encryption, mounts, wallpaper).
//!
//! Bodies preserved verbatim from the original `inject.rs` test module.

use super::super::*;
use crate::config::SwapConfig;
use std::path::PathBuf;

#[test]
fn inject_rejects_shell_metachar_in_mount() {
    let cfg = InjectConfig {
        mounts: vec!["/dev/sda1 /mnt ext4 defaults 0 0; whoami".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
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
