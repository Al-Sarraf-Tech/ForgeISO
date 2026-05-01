//! openSUSE Leap + Tumbleweed presets sourced from `mirrors.kernel.org`.

use super::super::preset::IsoPreset;
use super::super::preset_id::PresetId;
use super::super::strategy::AcquisitionStrategy;

pub(super) static PRESETS: &[IsoPreset] = &[
    IsoPreset {
        id: PresetId::OpenSuseLeap,
        name: "openSUSE Leap 15.6 DVD",
        distro: "opensuse",
        edition: "leap-dvd",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://get.opensuse.org/leap/",
        direct_url: Some(
            "https://mirrors.kernel.org/opensuse/distribution/leap/15.6/iso/openSUSE-Leap-15.6-DVD-x86_64-Media.iso",
        ),
        checksum_url: Some(
            "https://mirrors.kernel.org/opensuse/distribution/leap/15.6/iso/openSUSE-Leap-15.6-DVD-x86_64-Media.iso.sha256",
        ),
        filename_suffix: Some("-DVD-x86_64-Media.iso"),
        note: "openSUSE Leap 15.6 — traditional LTS release; AutoYaST unattended install",
    },
    IsoPreset {
        id: PresetId::OpenSuseLeapNet,
        name: "openSUSE Leap 15.6 NET",
        distro: "opensuse",
        edition: "leap-net",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://get.opensuse.org/leap/",
        direct_url: Some(
            "https://mirrors.kernel.org/opensuse/distribution/leap/15.6/iso/openSUSE-Leap-15.6-NET-x86_64-Media.iso",
        ),
        checksum_url: Some(
            "https://mirrors.kernel.org/opensuse/distribution/leap/15.6/iso/openSUSE-Leap-15.6-NET-x86_64-Media.iso.sha256",
        ),
        filename_suffix: Some("-NET-x86_64-Media.iso"),
        note: "openSUSE Leap 15.6 — network installer; smaller download",
    },
    IsoPreset {
        id: PresetId::OpenSuseTumbleweed,
        name: "openSUSE Tumbleweed DVD",
        distro: "opensuse",
        edition: "tumbleweed",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://get.opensuse.org/tumbleweed/",
        direct_url: Some(
            "https://mirrors.kernel.org/opensuse/tumbleweed/iso/openSUSE-Tumbleweed-DVD-x86_64-Current.iso",
        ),
        checksum_url: Some(
            "https://mirrors.kernel.org/opensuse/tumbleweed/iso/openSUSE-Tumbleweed-DVD-x86_64-Current.iso.sha256",
        ),
        filename_suffix: Some("Tumbleweed-DVD-x86_64-Current.iso"),
        note: "openSUSE Tumbleweed — rolling release; Current alias always points to latest",
    },
];
