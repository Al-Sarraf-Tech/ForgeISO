use serde::{Deserialize, Serialize};

use super::preset_id::PresetId;
use super::strategy::AcquisitionStrategy;
use crate::error::EngineResult;

/// Describes a known ISO source for a distro edition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsoPreset {
    /// Unique identifier for this preset, used on the CLI and in config files.
    pub id: PresetId,
    /// Human-readable display name shown in the GUI preset picker.
    pub name: &'static str,
    /// Distro family identifier (e.g. `"ubuntu"`, `"fedora"`, `"arch"`).
    pub distro: &'static str,
    /// Edition or flavour within the distro (e.g. `"server-lts"`, `"kde"`).
    pub edition: &'static str,
    /// CPU architecture string (e.g. `"x86_64"`, `"aarch64"`).
    pub architecture: &'static str,
    /// How the ISO is obtained — direct URL, discovery page, or user-supplied.
    pub strategy: AcquisitionStrategy,
    /// Official release/download page (always set).
    pub official_page: &'static str,
    /// Stable direct URL when strategy == DirectUrl (may be None for others).
    pub direct_url: Option<&'static str>,
    /// URL to fetch checksums file (SHA256SUMS or similar). May be None.
    pub checksum_url: Option<&'static str>,
    /// Expected filename suffix to recognise the right .iso in a listing.
    pub filename_suffix: Option<&'static str>,
    /// Human-readable note shown to the user about this preset.
    pub note: &'static str,
}

/// Return all built-in presets.
pub fn all_presets() -> &'static [IsoPreset] {
    super::catalog::ALL_PRESETS.as_slice()
}

/// Find a preset by its PresetId.
pub fn find_preset(id: &PresetId) -> Option<&'static IsoPreset> {
    super::catalog::ALL_PRESETS.iter().find(|p| &p.id == id)
}

/// Find a preset by its string identifier (case-insensitive kebab-case).
pub fn find_preset_by_str(s: &str) -> Option<&'static IsoPreset> {
    let id = PresetId::parse(s)?;
    find_preset(&id)
}

/// Resolve what URL to use for this preset.
/// Returns Ok(Some(url)) for direct URLs.
/// Returns Ok(None) when strategy == UserProvided or DiscoveryPage (caller must prompt).
/// Returns an error only for internal bugs.
pub fn resolve_url(preset: &IsoPreset) -> EngineResult<Option<String>> {
    match preset.strategy {
        AcquisitionStrategy::DirectUrl => Ok(preset.direct_url.map(|u| u.to_string())),
        AcquisitionStrategy::DiscoveryPage | AcquisitionStrategy::UserProvided => Ok(None),
    }
}

/// Format a user-facing summary of a preset (for CLI list output).
pub fn format_preset_summary(preset: &IsoPreset) -> String {
    format!(
        "{:<25} {:<12} {:<14} {}",
        preset.id.as_str(),
        preset.distro,
        preset.strategy.as_str(),
        preset.note
    )
}

/// Format a detailed view of a preset (for CLI show output).
pub fn format_preset_detail(preset: &IsoPreset) -> String {
    let direct_url = preset.direct_url.unwrap_or("none");
    let checksum_url = preset.checksum_url.unwrap_or("none");
    format!(
        "Preset:        {}\nName:          {}\nDistro:        {}\nEdition:       {}\nArchitecture:  {}\nStrategy:      {}\nOfficial page: {}\nDirect URL:    {}\nChecksum URL:  {}\nNote:          {}",
        preset.id.as_str(),
        preset.name,
        preset.distro,
        preset.edition,
        preset.architecture,
        preset.strategy.as_str(),
        preset.official_page,
        direct_url,
        checksum_url,
        preset.note,
    )
}
