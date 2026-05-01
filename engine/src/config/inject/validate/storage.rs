//! Validators for swap, encryption, mounts, wallpaper.
//!
//! Logic preserved verbatim from the original monolithic
//! `engine/src/config/inject.rs::validate()`.

use crate::config::components::SwapConfig;
use crate::config::validation::is_safe_path;
use crate::error::{EngineError, EngineResult};
use std::path::PathBuf;

// -- Storage / encryption / mounts / wallpaper -------------------------------

pub(super) fn validate_swap(swap: Option<&SwapConfig>) -> EngineResult<()> {
    // Swap filename
    // The filename is interpolated as:
    //   fallocate -l {mb}M /target{fname}   -> requires leading / to produce /target/swapfile
    //   chroot /target mkswap {fname}        -> requires absolute path inside the chroot
    //   echo '{fname} none swap ...' >> fstab -> requires absolute path
    // A relative name like "myswap" would create /targetmyswap (no separator),
    // and mkswap/fstab would reference a relative path that doesn't exist.
    if let Some(swap) = swap {
        if swap.size_mb == 0 {
            return Err(EngineError::InvalidConfig(
                "swap.size_mb must be greater than 0".to_string(),
            ));
        }
        if let Some(v) = swap.swappiness {
            if v > 100 {
                return Err(EngineError::InvalidConfig(format!(
                    "swap.swappiness must be 0\u{2013}100, got {v}"
                )));
            }
        }
        if let Some(fname) = &swap.filename {
            is_safe_path(fname, "swap_filename")?;
            if !fname.starts_with('/') {
                return Err(EngineError::InvalidConfig(format!(
                    "swap_filename must be an absolute path starting with '/': {fname:?}"
                )));
            }
            // Block .. path components: fallocate and chmod are called as
            // `command /target{fname}` so a traversal like `/../etc/passwd`
            // would resolve to /etc/passwd on the installer's running system.
            if fname.split('/').any(|c| c == "..") {
                return Err(EngineError::InvalidConfig(format!(
                    "swap_filename must not contain '..' path traversal: {fname:?}"
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_swap_size(swap: Option<&SwapConfig>) -> EngineResult<()> {
    // Swap size upper bound -- accepting arbitrarily large values (e.g. 999 GB)
    // would not fail validation but would produce a swap file that can never be
    // allocated, causing the installer to hang or error at runtime.
    // Cap at 128 GB (131072 MB), which is larger than any reasonable swap need.
    if let Some(swap) = swap {
        if swap.size_mb > 131_072 {
            return Err(EngineError::InvalidConfig(format!(
                "swap.size_mb {} exceeds maximum of 131072 (128 GiB)",
                swap.size_mb
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_mounts(mounts: &[String]) -> EngineResult<()> {
    // Mount entries -- written into fstab via echo
    for entry in mounts {
        if entry
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "mount entry contains shell metacharacters: {entry:?}"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_encryption(
    encrypt: bool,
    encrypt_passphrase: Option<&String>,
    storage_layout: Option<&String>,
) -> EngineResult<()> {
    // Encryption: a passphrase is required when encrypt=true.
    // cloud-init autoinstall requires storage.layout.password; without it
    // the installer fails or silently uses an empty LUKS passphrase, which
    // is a serious security defect. There is no interactive fallback in
    // unattended mode.
    if encrypt && encrypt_passphrase.is_none() {
        return Err(EngineError::InvalidConfig(
            "encrypt is enabled but no encrypt_passphrase was provided; \
             Ubuntu cloud-init requires a LUKS passphrase in the storage layout"
                .to_string(),
        ));
    }

    // Encryption also requires a storage_layout -- without one, the autoinstall
    // YAML has no storage.layout block, so the LUKS password has nowhere to go
    // and encryption is silently skipped by cloud-init.
    if encrypt && storage_layout.is_none() {
        return Err(EngineError::InvalidConfig(
            "encrypt is enabled but no storage_layout was provided; \
             Ubuntu cloud-init requires a named storage layout (e.g. 'lvm' or 'direct') \
             to attach the LUKS passphrase to"
                .to_string(),
        ));
    }

    Ok(())
}

pub(super) fn validate_wallpaper(wallpaper: Option<&PathBuf>) -> EngineResult<()> {
    // Wallpaper -- the filename component is used directly in an unquoted shell
    // `cp /cdrom/wallpaper/{filename}` command.  A malicious filename like
    // `foo; rm -rf /.jpg` would execute arbitrary code on the installer's
    // running system.  Apply the same character set as is_safe_path: only
    // alphanumeric, dash, underscore, dot, and plus are allowed.
    if let Some(wp) = wallpaper {
        if let Some(fname) = wp.file_name().and_then(|n| n.to_str()) {
            if fname
                .chars()
                .any(|c| !c.is_alphanumeric() && !matches!(c, '-' | '_' | '.' | '+'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "wallpaper filename contains unsafe characters: {fname:?} \
                     (only alphanumeric, dash, underscore, dot, plus allowed)"
                )));
            }
        } else {
            return Err(EngineError::InvalidConfig(
                "wallpaper path must have a valid UTF-8 filename component".to_string(),
            ));
        }
    }
    Ok(())
}
