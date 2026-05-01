//! Fedora Server / Workstation / KDE Spin presets.

use super::super::preset::IsoPreset;
use super::super::preset_id::PresetId;
use super::super::strategy::AcquisitionStrategy;

pub(super) static PRESETS: &[IsoPreset] = &[
    IsoPreset {
        id: PresetId::FedoraServer,
        name: "Fedora 42 Server",
        distro: "fedora",
        edition: "server",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://fedoraproject.org/server/download/",
        direct_url: Some(
            "https://dl.fedoraproject.org/pub/fedora/linux/releases/42/Server/x86_64/iso/Fedora-Server-netinst-x86_64-42-1.1.iso",
        ),
        checksum_url: Some(
            "https://dl.fedoraproject.org/pub/fedora/linux/releases/42/Server/x86_64/iso/Fedora-Server-42-1.1-x86_64-CHECKSUM",
        ),
        filename_suffix: Some("-Server-netinst-x86_64-"),
        note: "Fedora 42 Server — network install; unattended via Kickstart",
    },
    IsoPreset {
        id: PresetId::FedoraWorkstation,
        name: "Fedora 42 Workstation",
        distro: "fedora",
        edition: "workstation",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://fedoraproject.org/workstation/download/",
        direct_url: Some(
            "https://dl.fedoraproject.org/pub/fedora/linux/releases/42/Workstation/x86_64/iso/Fedora-Workstation-Live-42-1.1.x86_64.iso",
        ),
        checksum_url: Some(
            "https://dl.fedoraproject.org/pub/fedora/linux/releases/42/Workstation/x86_64/iso/Fedora-Workstation-42-1.1-x86_64-CHECKSUM",
        ),
        filename_suffix: Some("-Workstation-Live-"),
        note: "Fedora 42 Workstation — GNOME live image; Kickstart injection",
    },
    IsoPreset {
        id: PresetId::FedoraKde,
        name: "Fedora 42 KDE",
        distro: "fedora",
        edition: "kde",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://fedoraproject.org/spins/kde/download/",
        direct_url: Some(
            "https://dl.fedoraproject.org/pub/fedora/linux/releases/42/KDE/x86_64/iso/Fedora-KDE-Desktop-Live-42-1.1.x86_64.iso",
        ),
        checksum_url: Some(
            "https://dl.fedoraproject.org/pub/fedora/linux/releases/42/KDE/x86_64/iso/Fedora-KDE-42-1.1-x86_64-CHECKSUM",
        ),
        filename_suffix: Some("-KDE-Desktop-Live-"),
        note: "Fedora 42 KDE — Plasma desktop live spin",
    },
];
