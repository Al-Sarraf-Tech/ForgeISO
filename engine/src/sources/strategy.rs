use serde::{Deserialize, Serialize};

/// Acquisition strategy for this preset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionStrategy {
    /// A direct, stable download URL is known.
    DirectUrl,
    /// A download page must be consulted to find the current URL.
    DiscoveryPage,
    /// The user must supply a URL or local path (e.g., RHEL).
    UserProvided,
}

impl AcquisitionStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DirectUrl => "direct_url",
            Self::DiscoveryPage => "discovery_page",
            Self::UserProvided => "user_provided",
        }
    }
}
