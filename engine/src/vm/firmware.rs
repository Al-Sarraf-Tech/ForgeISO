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
