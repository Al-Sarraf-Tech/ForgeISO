//! Configuration types — the declarative inputs that drive every
//! engine operation.
//!
//! Two top-level shapes:
//!
//! - [`InjectConfig`] — describes *what* to inject into a source ISO:
//!   user account, hostname, network, packages, services, firewall,
//!   storage, GRUB. Front-ends build it via the typed
//!   [`InjectConfigBuilder`].
//! - [`BuildConfig`] — describes *how* to build: source preset or
//!   path, output label, profile selector, scan/test toggles.
//!   `BuildConfig` carries (or references) an `InjectConfig`.
//!
//! Both types are `serde`-serializable for round-tripping through
//! YAML or JSON and form part of the project's stability contract;
//! see [`STABILITY.md`](https://github.com/Al-Sarraf-Tech/ForgeISO/blob/main/STABILITY.md)
//! for which fields can change in 1.x and which are frozen.
//!
//! Per-concern validators live in
//! [`crate::config::validation`](self::validation) (crate-private)
//! and are exercised under `engine/tests/proptest_config.rs`.

mod build;
mod components;
mod inject;
mod inject_builder;
pub(crate) mod validation;

pub use build::{BuildConfig, ScanPolicy, TestingPolicy};
pub use components::{
    ContainerConfig, FirewallConfig, GrubConfig, NetworkConfig, ProxyConfig, SshConfig, SwapConfig,
    UserConfig,
};
pub use inject::InjectConfig;
pub use inject_builder::InjectConfigBuilder;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Target Linux distribution family that determines which installer format is generated.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Distro {
    /// Ubuntu and Ubuntu-derived systems (uses cloud-init autoinstall YAML).
    Ubuntu,
    /// Linux Mint (uses Calamares preseed.cfg).
    Mint,
    /// Fedora and RHEL-family systems (uses Kickstart cfg).
    Fedora,
    /// Arch Linux (uses archinstall configuration).
    Arch,
}

/// Selects the software profile applied during ISO build.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKind {
    /// Minimal installation: no desktop environment, server-oriented package set.
    Minimal,
    /// Desktop installation: full GUI environment and associated tooling.
    Desktop,
}

/// Result of executing an optional scanning or testing tool during the build pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    /// Tool ran and completed without errors.
    Passed,
    /// Tool ran but reported one or more errors or policy violations.
    Failed,
    /// Tool binary was not found or could not be executed on this host.
    Unavailable,
    /// Tool was skipped because its corresponding policy flag was disabled.
    Skipped,
}

/// Location of the source ISO: either a local filesystem path or an HTTP(S) URL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum IsoSource {
    /// Path to a locally accessible ISO file.
    Path(PathBuf),
    /// HTTP or HTTPS URL from which the ISO will be downloaded before use.
    Url(String),
}

impl Default for IsoSource {
    fn default() -> Self {
        IsoSource::Path(PathBuf::new())
    }
}

impl IsoSource {
    /// Parse a raw string into an `IsoSource`, treating `http://` and `https://` prefixes as URLs
    /// and anything else as a local filesystem path.
    #[must_use]
    pub fn from_raw(input: impl Into<String>) -> Self {
        let raw = input.into();
        if raw.starts_with("http://") || raw.starts_with("https://") {
            Self::Url(raw)
        } else {
            Self::Path(PathBuf::from(raw))
        }
    }

    /// Return a human-readable string representation: the URL string for remote sources, or the
    /// path converted via [`std::path::Path::display`] for local paths.
    #[must_use]
    pub fn display_value(&self) -> String {
        match self {
            Self::Path(path) => path.display().to_string(),
            Self::Url(url) => url.clone(),
        }
    }

    /// Return `true` if the source is an HTTP(S) URL that must be downloaded before use.
    #[must_use]
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Url(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_url_source() {
        let source = IsoSource::from_raw("https://example.test/test.iso");
        assert!(matches!(source, IsoSource::Url(_)));
    }

    // -- IsoSource --

    #[test]
    fn iso_source_from_raw_detects_https_url() {
        let src = IsoSource::from_raw("https://releases.ubuntu.com/noble/ubuntu.iso");
        assert!(src.is_remote());
        assert!(matches!(src, IsoSource::Url(_)));
    }

    #[test]
    fn iso_source_from_raw_detects_http_url() {
        let src = IsoSource::from_raw("http://mirror.example.com/ubuntu.iso");
        assert!(src.is_remote());
    }

    #[test]
    fn iso_source_from_raw_treats_local_path_as_path() {
        let src = IsoSource::from_raw("/tmp/ubuntu.iso");
        assert!(!src.is_remote());
        assert!(matches!(src, IsoSource::Path(_)));
    }

    #[test]
    fn iso_source_display_value_url() {
        let url = "https://example.com/ubuntu.iso";
        let src = IsoSource::from_raw(url);
        assert_eq!(src.display_value(), url);
    }

    #[test]
    fn iso_source_display_value_path() {
        let src = IsoSource::from_raw("/tmp/ubuntu.iso");
        assert_eq!(src.display_value(), "/tmp/ubuntu.iso");
    }

    #[test]
    fn iso_source_from_raw_uppercase_http_treated_as_path() {
        // `from_raw` does an ASCII-case-sensitive prefix check; uppercase HTTP:// is
        // NOT a recognised scheme and must fall through to path.
        let src = IsoSource::from_raw("HTTP://example.com/file.iso");
        assert!(
            matches!(src, IsoSource::Path(_)),
            "uppercase scheme must be treated as path, not URL"
        );
    }

    #[test]
    fn iso_source_from_raw_empty_string_is_path() {
        let src = IsoSource::from_raw("");
        assert!(matches!(src, IsoSource::Path(_)));
    }

    #[test]
    fn iso_source_display_value_round_trips() {
        let url = "https://example.com/ubuntu.iso";
        let src = IsoSource::from_raw(url);
        assert_eq!(src.display_value(), url);

        let path = "/tmp/local.iso";
        let src = IsoSource::from_raw(path);
        assert_eq!(src.display_value(), path);
    }

    #[test]
    fn iso_source_is_remote_only_for_url() {
        assert!(IsoSource::from_raw("https://cdn.example.com/a.iso").is_remote());
        assert!(!IsoSource::from_raw("/tmp/local.iso").is_remote());
    }
}
