//! Debian and Debian-derived security distros (Kali Linux).

use super::super::preset::IsoPreset;
use super::super::preset_id::PresetId;
use super::super::strategy::AcquisitionStrategy;

pub(super) static PRESETS: &[IsoPreset] = &[
    IsoPreset {
        id: PresetId::DebianNetInst,
        name: "Debian 13.3.0 Netinstall",
        distro: "debian",
        edition: "netinstall",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://www.debian.org/CD/netinst/",
        direct_url: Some(
            "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-13.3.0-amd64-netinst.iso",
        ),
        checksum_url: Some(
            "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA256SUMS",
        ),
        filename_suffix: Some("-amd64-netinst.iso"),
        note: "Debian 13 (Trixie) — minimal netinstall; preseed unattended install",
    },
    IsoPreset {
        id: PresetId::KaliLinux,
        name: "Kali Linux 2025.4 Installer",
        distro: "debian",
        edition: "kali",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://www.kali.org/get-kali/",
        direct_url: Some(
            "https://cdimage.kali.org/current/kali-linux-2025.4-installer-amd64.iso",
        ),
        checksum_url: Some("https://cdimage.kali.org/current/SHA256SUMS"),
        filename_suffix: Some("-installer-amd64.iso"),
        note: "Kali Linux 2025.4 — full installer; preseed supported",
    },
    IsoPreset {
        id: PresetId::KaliLinuxNetinst,
        name: "Kali Linux 2025.4 Netinstall",
        distro: "debian",
        edition: "kali-netinst",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://www.kali.org/get-kali/",
        direct_url: Some(
            "https://cdimage.kali.org/current/kali-linux-2025.4-installer-netinst-amd64.iso",
        ),
        checksum_url: Some("https://cdimage.kali.org/current/SHA256SUMS"),
        filename_suffix: Some("-installer-netinst-amd64.iso"),
        note: "Kali Linux 2025.4 — netinstall; minimal download, packages from network",
    },
];
