//! Pop!_OS presets sourced from `iso.pop-os.org` (Ubuntu base, classified
//! under the `ubuntu` distro for downstream injection paths).

use super::super::preset::IsoPreset;
use super::super::preset_id::PresetId;
use super::super::strategy::AcquisitionStrategy;

pub(super) static PRESETS: &[IsoPreset] = &[
    IsoPreset {
        id: PresetId::PopOs22Intel,
        name: "Pop!_OS 22.04 (Intel/AMD)",
        distro: "ubuntu",
        edition: "pop-os-intel",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://pop.system76.com/",
        direct_url: Some(
            "https://iso.pop-os.org/22.04/amd64/intel/46/pop-os_22.04_amd64_intel_46.iso",
        ),
        checksum_url: Some("https://iso.pop-os.org/22.04/amd64/intel/46/SHA256SUMS"),
        filename_suffix: Some("_amd64_intel_"),
        note: "Pop!_OS 22.04 — Intel/AMD GPU build; Ubuntu 22.04 base",
    },
    IsoPreset {
        id: PresetId::PopOs22Nvidia,
        name: "Pop!_OS 22.04 (NVIDIA)",
        distro: "ubuntu",
        edition: "pop-os-nvidia",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://pop.system76.com/",
        direct_url: Some(
            "https://iso.pop-os.org/22.04/amd64/nvidia/46/pop-os_22.04_amd64_nvidia_46.iso",
        ),
        checksum_url: Some("https://iso.pop-os.org/22.04/amd64/nvidia/46/SHA256SUMS"),
        filename_suffix: Some("_amd64_nvidia_"),
        note: "Pop!_OS 22.04 — NVIDIA GPU build with proprietary drivers bundled",
    },
    IsoPreset {
        id: PresetId::PopOs24Intel,
        name: "Pop!_OS 24.04 (Intel/AMD)",
        distro: "ubuntu",
        edition: "pop-os-24-intel",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://pop.system76.com/",
        direct_url: Some(
            "https://iso.pop-os.org/24.04/amd64/intel/9/pop-os_24.04_amd64_intel_9.iso",
        ),
        checksum_url: Some("https://iso.pop-os.org/24.04/amd64/intel/9/SHA256SUMS"),
        filename_suffix: Some("_amd64_intel_"),
        note: "Pop!_OS 24.04 — Intel/AMD GPU build; Ubuntu 24.04 base",
    },
];
