use serde::{Deserialize, Serialize};

use crate::error::EngineResult;

/// A well-known distro edition that ForgeISO knows how to find.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PresetId {
    UbuntuServerLts,
    UbuntuDesktopLts,
    LinuxMintCinnamon,
    FedoraServer,
    FedoraWorkstation,
    RockyLinux,
    AlmaLinux,
    CentOsStream,
    ArchLinux,
    RhelCustom,
}

impl PresetId {
    /// Parse from a user-supplied string (kebab-case, case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ubuntu-server-lts" => Some(Self::UbuntuServerLts),
            "ubuntu-desktop-lts" => Some(Self::UbuntuDesktopLts),
            "linux-mint-cinnamon" => Some(Self::LinuxMintCinnamon),
            "fedora-server" => Some(Self::FedoraServer),
            "fedora-workstation" => Some(Self::FedoraWorkstation),
            "rocky-linux" => Some(Self::RockyLinux),
            "almalinux" => Some(Self::AlmaLinux),
            "centos-stream" => Some(Self::CentOsStream),
            "arch-linux" => Some(Self::ArchLinux),
            "rhel-custom" => Some(Self::RhelCustom),
            _ => None,
        }
    }

    /// Return the canonical kebab-case name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UbuntuServerLts => "ubuntu-server-lts",
            Self::UbuntuDesktopLts => "ubuntu-desktop-lts",
            Self::LinuxMintCinnamon => "linux-mint-cinnamon",
            Self::FedoraServer => "fedora-server",
            Self::FedoraWorkstation => "fedora-workstation",
            Self::RockyLinux => "rocky-linux",
            Self::AlmaLinux => "almalinux",
            Self::CentOsStream => "centos-stream",
            Self::ArchLinux => "arch-linux",
            Self::RhelCustom => "rhel-custom",
        }
    }
}

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

/// Describes a known ISO source for a distro edition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsoPreset {
    pub id: PresetId,
    pub name: &'static str,
    pub distro: &'static str,
    pub edition: &'static str,
    pub architecture: &'static str,
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

static ALL_PRESETS: &[IsoPreset] = &[
    IsoPreset {
        id: PresetId::UbuntuServerLts,
        name: "Ubuntu 24.04 LTS Server",
        distro: "ubuntu",
        edition: "server-lts",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/server",
        direct_url: Some(
            "https://releases.ubuntu.com/noble/ubuntu-24.04.2-live-server-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/noble/SHA256SUMS"),
        filename_suffix: Some("-live-server-amd64.iso"),
        note: "Ubuntu 24.04 LTS Server — fully unattended via cloud-init autoinstall",
    },
    IsoPreset {
        id: PresetId::UbuntuDesktopLts,
        name: "Ubuntu 24.04 LTS Desktop",
        distro: "ubuntu",
        edition: "desktop-lts",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/desktop",
        direct_url: Some(
            "https://releases.ubuntu.com/noble/ubuntu-24.04.2-desktop-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/noble/SHA256SUMS"),
        filename_suffix: Some("-desktop-amd64.iso"),
        note: "Ubuntu 24.04 LTS Desktop — autoinstall supported since 23.04",
    },
    IsoPreset {
        id: PresetId::LinuxMintCinnamon,
        name: "Linux Mint Cinnamon",
        distro: "mint",
        edition: "cinnamon",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DiscoveryPage,
        official_page: "https://linuxmint.com/edition.php?id=326",
        direct_url: None,
        checksum_url: None,
        filename_suffix: Some("-cinnamon-64bit.iso"),
        note: "Linux Mint Cinnamon — overlay/preseed remaster; see official page for current URL",
    },
    IsoPreset {
        id: PresetId::FedoraServer,
        name: "Fedora Server",
        distro: "fedora",
        edition: "server",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DiscoveryPage,
        official_page: "https://fedoraproject.org/server/download/",
        direct_url: None,
        checksum_url: None,
        filename_suffix: Some("-Server-dvd-x86_64-"),
        note: "Fedora Server — unattended via Kickstart ks.cfg injection",
    },
    IsoPreset {
        id: PresetId::FedoraWorkstation,
        name: "Fedora Workstation",
        distro: "fedora",
        edition: "workstation",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DiscoveryPage,
        official_page: "https://fedoraproject.org/workstation/download/",
        direct_url: None,
        checksum_url: None,
        filename_suffix: Some("-Workstation-Live-x86_64-"),
        note: "Fedora Workstation — Kickstart injection; boot to graphical installer",
    },
    IsoPreset {
        id: PresetId::RockyLinux,
        name: "Rocky Linux 9",
        distro: "rhel-family",
        edition: "server",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://rockylinux.org/download",
        direct_url: Some(
            "https://download.rockylinux.org/pub/rocky/9/isos/x86_64/Rocky-9.5-x86_64-boot.iso",
        ),
        checksum_url: Some(
            "https://download.rockylinux.org/pub/rocky/9/isos/x86_64/CHECKSUM",
        ),
        filename_suffix: Some("-x86_64-boot.iso"),
        note: "Rocky Linux 9 — RHEL-compatible; unattended via Kickstart (same path as Fedora)",
    },
    IsoPreset {
        id: PresetId::AlmaLinux,
        name: "AlmaLinux 9",
        distro: "rhel-family",
        edition: "server",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://almalinux.org/get-almalinux/",
        direct_url: Some(
            "https://repo.almalinux.org/almalinux/9/isos/x86_64/AlmaLinux-9.5-x86_64-boot.iso",
        ),
        checksum_url: Some(
            "https://repo.almalinux.org/almalinux/9/isos/x86_64/CHECKSUM",
        ),
        filename_suffix: Some("-x86_64-boot.iso"),
        note: "AlmaLinux 9 — RHEL-compatible; unattended via Kickstart",
    },
    IsoPreset {
        id: PresetId::CentOsStream,
        name: "CentOS Stream 10",
        distro: "rhel-family",
        edition: "stream",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://www.centos.org/download/",
        direct_url: Some(
            "https://mirror.stream.centos.org/10-stream/BaseOS/x86_64/iso/CentOS-Stream-10-latest-x86_64-boot.iso",
        ),
        checksum_url: None,
        filename_suffix: Some("-x86_64-boot.iso"),
        note: "CentOS Stream 10 — RHEL upstream; unattended via Kickstart",
    },
    IsoPreset {
        id: PresetId::ArchLinux,
        name: "Arch Linux",
        distro: "arch",
        edition: "rolling",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://archlinux.org/download/",
        direct_url: Some("https://geo.mirror.pkgbuild.com/iso/latest/archlinux-x86_64.iso"),
        checksum_url: Some(
            "https://geo.mirror.pkgbuild.com/iso/latest/sha256sums.txt",
        ),
        filename_suffix: Some("archlinux-x86_64.iso"),
        note: "Arch Linux — archinstall config injection; see docs/distro-support.md",
    },
    IsoPreset {
        id: PresetId::RhelCustom,
        name: "RHEL (Custom)",
        distro: "rhel-family",
        edition: "custom",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::UserProvided,
        official_page: "https://access.redhat.com/downloads/",
        direct_url: None,
        checksum_url: None,
        filename_suffix: None,
        note: "RHEL requires a subscription. Provide a local ISO path or your own URL.",
    },
];

/// Return all built-in presets.
pub fn all_presets() -> &'static [IsoPreset] {
    ALL_PRESETS
}

/// Find a preset by its PresetId.
pub fn find_preset(id: &PresetId) -> Option<&'static IsoPreset> {
    ALL_PRESETS.iter().find(|p| &p.id == id)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_returns_ten_items() {
        assert_eq!(all_presets().len(), 10);
    }

    #[test]
    fn find_preset_by_str_lowercase() {
        let preset = find_preset_by_str("ubuntu-server-lts");
        assert!(preset.is_some());
        assert_eq!(preset.unwrap().id, PresetId::UbuntuServerLts);
    }

    #[test]
    fn find_preset_by_str_uppercase() {
        let preset = find_preset_by_str("UBUNTU-SERVER-LTS");
        assert!(preset.is_some());
        assert_eq!(preset.unwrap().id, PresetId::UbuntuServerLts);
    }

    #[test]
    fn find_preset_by_str_mixed_case() {
        let preset = find_preset_by_str("Rocky-Linux");
        assert!(preset.is_some());
        assert_eq!(preset.unwrap().id, PresetId::RockyLinux);
    }

    #[test]
    fn find_preset_by_str_unknown_returns_none() {
        assert!(find_preset_by_str("does-not-exist").is_none());
    }

    #[test]
    fn resolve_url_direct_url_returns_some() {
        let preset = find_preset_by_str("ubuntu-server-lts").unwrap();
        let url = resolve_url(preset).unwrap();
        assert!(url.is_some());
        assert!(url.unwrap().starts_with("https://releases.ubuntu.com/"));
    }

    #[test]
    fn resolve_url_rocky_linux_returns_some() {
        let preset = find_preset_by_str("rocky-linux").unwrap();
        let url = resolve_url(preset).unwrap();
        assert!(url.is_some());
        assert!(url.unwrap().contains("rockylinux.org"));
    }

    #[test]
    fn resolve_url_user_provided_returns_none() {
        let preset = find_preset_by_str("rhel-custom").unwrap();
        let url = resolve_url(preset).unwrap();
        assert!(url.is_none());
    }

    #[test]
    fn resolve_url_discovery_page_returns_none() {
        let preset = find_preset_by_str("fedora-server").unwrap();
        let url = resolve_url(preset).unwrap();
        assert!(url.is_none());
    }

    #[test]
    fn format_preset_summary_contains_id_and_distro() {
        let preset = find_preset_by_str("arch-linux").unwrap();
        let summary = format_preset_summary(preset);
        assert!(summary.contains("arch-linux"));
        assert!(summary.contains("arch"));
    }

    #[test]
    fn format_preset_detail_contains_all_fields() {
        let preset = find_preset_by_str("ubuntu-server-lts").unwrap();
        let detail = format_preset_detail(preset);
        assert!(detail.contains("ubuntu-server-lts"));
        assert!(detail.contains("ubuntu"));
        assert!(detail.contains("server-lts"));
        assert!(detail.contains("direct_url"));
        assert!(detail.contains("releases.ubuntu.com"));
    }

    #[test]
    fn preset_id_as_str_round_trips() {
        for preset in all_presets() {
            let s = preset.id.as_str();
            let parsed = PresetId::parse(s);
            assert!(parsed.is_some(), "failed to round-trip: {s}");
            assert_eq!(&parsed.unwrap(), &preset.id);
        }
    }

    #[test]
    fn all_direct_url_presets_have_url() {
        for preset in all_presets() {
            if preset.strategy == AcquisitionStrategy::DirectUrl {
                assert!(
                    preset.direct_url.is_some(),
                    "DirectUrl preset '{}' missing direct_url",
                    preset.id.as_str()
                );
            }
        }
    }

    #[test]
    fn all_presets_have_nonempty_official_page() {
        for preset in all_presets() {
            assert!(
                !preset.official_page.is_empty(),
                "preset '{}' missing official_page",
                preset.id.as_str()
            );
        }
    }

    #[test]
    fn all_presets_have_nonempty_note() {
        for preset in all_presets() {
            assert!(
                !preset.note.is_empty(),
                "preset '{}' missing note",
                preset.id.as_str()
            );
        }
    }

    #[test]
    fn find_preset_returns_correct_struct() {
        let p = find_preset_by_str("ubuntu-server-lts").expect("should find preset");
        assert_eq!(p.id.as_str(), "ubuntu-server-lts");
        assert_eq!(p.strategy, AcquisitionStrategy::DirectUrl);
    }

    #[test]
    fn rhel_custom_is_user_provided() {
        let p = find_preset_by_str("rhel-custom").expect("rhel-custom preset must exist");
        assert_eq!(p.strategy, AcquisitionStrategy::UserProvided);
        assert!(resolve_url(p).unwrap().is_none());
    }

    #[test]
    fn discovery_page_presets_resolve_to_none() {
        // fedora-server and fedora-workstation use discovery pages
        for id in ["fedora-server", "fedora-workstation"] {
            let p = find_preset_by_str(id).unwrap_or_else(|| panic!("{id} must exist"));
            assert_eq!(p.strategy, AcquisitionStrategy::DiscoveryPage);
            assert!(
                resolve_url(p).unwrap().is_none(),
                "{id} should resolve to None"
            );
        }
    }

    #[test]
    fn format_preset_summary_width_is_consistent() {
        // Summary should not panic for any preset; spot-check content
        for preset in all_presets() {
            let s = format_preset_summary(preset);
            assert!(s.contains(preset.id.as_str()));
            assert!(s.contains(preset.strategy.as_str()));
        }
    }

    #[test]
    fn format_preset_detail_contains_official_page() {
        for preset in all_presets() {
            let d = format_preset_detail(preset);
            assert!(
                d.contains(preset.official_page),
                "detail missing official_page for {}",
                preset.id.as_str()
            );
        }
    }
}
