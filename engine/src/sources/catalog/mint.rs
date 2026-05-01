//! Linux Mint editions (Ubuntu 24.04 base) sourced from `mirrors.edge.kernel.org`.

use super::super::preset::IsoPreset;
use super::super::preset_id::PresetId;
use super::super::strategy::AcquisitionStrategy;

pub(super) static PRESETS: &[IsoPreset] = &[
    IsoPreset {
        id: PresetId::LinuxMintCinnamon,
        name: "Linux Mint 22.3 Cinnamon",
        distro: "mint",
        edition: "cinnamon",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://linuxmint.com/edition.php?id=326",
        direct_url: Some(
            "https://mirrors.edge.kernel.org/linuxmint/stable/22.3/linuxmint-22.3-cinnamon-64bit.iso",
        ),
        checksum_url: Some(
            "https://mirrors.edge.kernel.org/linuxmint/stable/22.3/sha256sum.txt",
        ),
        filename_suffix: Some("-cinnamon-64bit.iso"),
        note: "Linux Mint 22.3 Zena — Cinnamon desktop, Ubuntu 24.04 base",
    },
    IsoPreset {
        id: PresetId::LinuxMintMate,
        name: "Linux Mint 22.3 MATE",
        distro: "mint",
        edition: "mate",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://linuxmint.com/edition.php?id=327",
        direct_url: Some(
            "https://mirrors.edge.kernel.org/linuxmint/stable/22.3/linuxmint-22.3-mate-64bit.iso",
        ),
        checksum_url: Some(
            "https://mirrors.edge.kernel.org/linuxmint/stable/22.3/sha256sum.txt",
        ),
        filename_suffix: Some("-mate-64bit.iso"),
        note: "Linux Mint 22.3 Zena — MATE desktop, Ubuntu 24.04 base",
    },
    IsoPreset {
        id: PresetId::LinuxMintXfce,
        name: "Linux Mint 22.3 Xfce",
        distro: "mint",
        edition: "xfce",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://linuxmint.com/edition.php?id=328",
        direct_url: Some(
            "https://mirrors.edge.kernel.org/linuxmint/stable/22.3/linuxmint-22.3-xfce-64bit.iso",
        ),
        checksum_url: Some(
            "https://mirrors.edge.kernel.org/linuxmint/stable/22.3/sha256sum.txt",
        ),
        filename_suffix: Some("-xfce-64bit.iso"),
        note: "Linux Mint 22.3 Zena — Xfce desktop, Ubuntu 24.04 base",
    },
];
