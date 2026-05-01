//! RHEL-compatible distros: Rocky Linux, AlmaLinux, CentOS Stream, RHEL custom.

use super::super::preset::IsoPreset;
use super::super::preset_id::PresetId;
use super::super::strategy::AcquisitionStrategy;

pub(super) static PRESETS: &[IsoPreset] = &[
    IsoPreset {
        id: PresetId::RockyLinux,
        name: "Rocky Linux 9",
        distro: "rhel-family",
        edition: "server",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://rockylinux.org/download",
        // -latest- alias always tracks the current 9.x point release;
        // avoids URL rot on every minor bump (9.5 → 9.6 → 9.7 …).
        direct_url: Some(
            "https://download.rockylinux.org/pub/rocky/9/isos/x86_64/Rocky-9-latest-x86_64-boot.iso",
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
        // -latest- alias always tracks the current 9.x point release;
        // avoids URL rot on every minor bump (9.5 → 9.6 → 9.7 …).
        direct_url: Some(
            "https://repo.almalinux.org/almalinux/9/isos/x86_64/AlmaLinux-9-latest-x86_64-boot.iso",
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
