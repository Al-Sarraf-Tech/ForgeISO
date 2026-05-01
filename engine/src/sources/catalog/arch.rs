//! Arch Linux + Arch-derived distros (EndeavourOS, Garuda, Manjaro).

use super::super::preset::IsoPreset;
use super::super::preset_id::PresetId;
use super::super::strategy::AcquisitionStrategy;

pub(super) static PRESETS: &[IsoPreset] = &[
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
        id: PresetId::EndeavourOs,
        name: "EndeavourOS",
        distro: "arch",
        edition: "endeavouros",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://endeavouros.com/latest-release/",
        direct_url: Some(
            "https://mirrors.tuna.tsinghua.edu.cn/endeavouros/iso/EndeavourOS_Ganymede-Neo-2026.01.12.iso",
        ),
        checksum_url: Some(
            "https://mirrors.tuna.tsinghua.edu.cn/endeavouros/iso/EndeavourOS_Ganymede-Neo-2026.01.12.iso.sha512sum",
        ),
        filename_suffix: Some("EndeavourOS_Ganymede-Neo-"),
        note: "EndeavourOS — Arch-based, friendly installer; TUNA mirror",
    },
    IsoPreset {
        id: PresetId::GarudaDr460nized,
        name: "Garuda Linux dr460nized",
        distro: "arch",
        edition: "dr460nized",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://garudalinux.org/downloads",
        direct_url: Some(
            "https://iso.builds.garudalinux.org/iso/garuda/dr460nized/260308/garuda-dr460nized-linux-zen-260308.iso",
        ),
        checksum_url: None,
        filename_suffix: Some("garuda-dr460nized-linux-zen-"),
        note: "Garuda dr460nized — KDE Plasma eye-candy; build-dated URL (update periodically)",
    },
    IsoPreset {
        id: PresetId::GarudaGnome,
        name: "Garuda Linux GNOME",
        distro: "arch",
        edition: "gnome",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://garudalinux.org/downloads",
        direct_url: Some(
            "https://iso.builds.garudalinux.org/iso/garuda/gnome/260308/garuda-gnome-linux-zen-260308.iso",
        ),
        checksum_url: None,
        filename_suffix: Some("garuda-gnome-linux-zen-"),
        note: "Garuda GNOME — Arch-based; build-dated URL (update periodically)",
    },
    IsoPreset {
        id: PresetId::GarudaXfce,
        name: "Garuda Linux Xfce",
        distro: "arch",
        edition: "xfce",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://garudalinux.org/downloads",
        direct_url: Some(
            "https://iso.builds.garudalinux.org/iso/garuda/xfce/260308/garuda-xfce-linux-lts-260308.iso",
        ),
        checksum_url: None,
        filename_suffix: Some("garuda-xfce-linux-lts-"),
        note: "Garuda Xfce — Arch-based, lightweight; build-dated URL (update periodically)",
    },
    IsoPreset {
        id: PresetId::Manjaro,
        name: "Manjaro",
        distro: "arch",
        edition: "kde",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DiscoveryPage,
        official_page: "https://manjaro.org/download/",
        direct_url: None,
        checksum_url: None,
        filename_suffix: Some("manjaro-kde-"),
        note: "Manjaro — filename includes kernel+build stamp; visit download page for current URL",
    },
];
