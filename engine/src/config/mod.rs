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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Distro {
    Ubuntu,
    Mint,
    Fedora,
    Arch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKind {
    Minimal,
    Desktop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Passed,
    Failed,
    Unavailable,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum IsoSource {
    Path(PathBuf),
    Url(String),
}

impl Default for IsoSource {
    fn default() -> Self {
        IsoSource::Path(PathBuf::new())
    }
}

impl IsoSource {
    #[must_use]
    pub fn from_raw(input: impl Into<String>) -> Self {
        let raw = input.into();
        if raw.starts_with("http://") || raw.starts_with("https://") {
            Self::Url(raw)
        } else {
            Self::Path(PathBuf::from(raw))
        }
    }

    #[must_use]
    pub fn display_value(&self) -> String {
        match self {
            Self::Path(path) => path.display().to_string(),
            Self::Url(url) => url.clone(),
        }
    }

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
