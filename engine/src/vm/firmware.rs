use serde::{Deserialize, Serialize};

/// Boot firmware mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirmwareMode {
    Bios,
    Uefi,
}

impl FirmwareMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            FirmwareMode::Bios => "bios",
            FirmwareMode::Uefi => "uefi",
        }
    }

    /// Parse from a lowercase string. Returns `None` for unknown values.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "bios" | "legacy" => Some(FirmwareMode::Bios),
            "uefi" | "efi" => Some(FirmwareMode::Uefi),
            _ => None,
        }
    }
}

impl std::fmt::Display for FirmwareMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_returns_canonical_lower_case_token() {
        assert_eq!(FirmwareMode::Bios.as_str(), "bios");
        assert_eq!(FirmwareMode::Uefi.as_str(), "uefi");
    }

    #[test]
    fn from_str_accepts_canonical_and_alias_tokens() {
        assert_eq!(FirmwareMode::from_str("bios"), Some(FirmwareMode::Bios));
        assert_eq!(FirmwareMode::from_str("legacy"), Some(FirmwareMode::Bios));
        assert_eq!(FirmwareMode::from_str("uefi"), Some(FirmwareMode::Uefi));
        assert_eq!(FirmwareMode::from_str("efi"), Some(FirmwareMode::Uefi));
    }

    #[test]
    fn from_str_is_case_insensitive() {
        assert_eq!(FirmwareMode::from_str("BIOS"), Some(FirmwareMode::Bios));
        assert_eq!(FirmwareMode::from_str("UEFI"), Some(FirmwareMode::Uefi));
        assert_eq!(FirmwareMode::from_str("Uefi"), Some(FirmwareMode::Uefi));
    }

    #[test]
    fn from_str_returns_none_for_unknown_token() {
        assert!(FirmwareMode::from_str("coreboot").is_none());
        assert!(FirmwareMode::from_str("").is_none());
    }

    #[test]
    fn display_matches_as_str() {
        assert_eq!(format!("{}", FirmwareMode::Bios), "bios");
        assert_eq!(format!("{}", FirmwareMode::Uefi), "uefi");
    }
}
