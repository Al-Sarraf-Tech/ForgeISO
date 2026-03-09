use serde::{Deserialize, Serialize};

use crate::error::EngineResult;

/// A well-known distro edition that ForgeISO knows how to find.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PresetId {
    // Ubuntu — current LTS (24.04 Noble) via Xenyth mirror
    UbuntuServerLts,
    UbuntuDesktopLts,
    // Ubuntu — 25.10 Questing (non-LTS) via Xenyth mirror
    UbuntuServer2510,
    UbuntuDesktop2510,
    // Ubuntu — 22.04 Jammy LTS via Xenyth mirror
    UbuntuServerJammy,
    UbuntuDesktopJammy,
    // Ubuntu — 20.04 Focal LTS via Xenyth mirror
    UbuntuServerFocal,
    UbuntuDesktopFocal,
    // Ubuntu — 18.04 Bionic LTS via Xenyth mirror
    UbuntuServerBionic,
    UbuntuDesktopBionic,
    // Linux Mint 22.3 Zena — kernel.org mirror
    LinuxMintCinnamon,
    LinuxMintMate,
    LinuxMintXfce,
    // Fedora 42 — dl.fedoraproject.org
    FedoraServer,
    FedoraWorkstation,
    FedoraKde,
    // RHEL family
    RockyLinux,
    AlmaLinux,
    CentOsStream,
    RhelCustom,
    // Arch family
    ArchLinux,
    EndeavourOs,
    GarudaDr460nized,
    GarudaGnome,
    GarudaXfce,
    Manjaro,
    // Debian family
    DebianNetInst,
    // openSUSE — kernel.org mirror
    OpenSuseLeap,
    OpenSuseLeapNet,
    OpenSuseTumbleweed,
    // Security
    KaliLinux,
    KaliLinuxNetinst,
    // Pop!_OS — iso.pop-os.org
    PopOs22Intel,
    PopOs22Nvidia,
    PopOs24Intel,
}

impl PresetId {
    /// Parse from a user-supplied string (kebab-case, case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ubuntu-server-lts" => Some(Self::UbuntuServerLts),
            "ubuntu-desktop-lts" => Some(Self::UbuntuDesktopLts),
            "ubuntu-server-2510" => Some(Self::UbuntuServer2510),
            "ubuntu-desktop-2510" => Some(Self::UbuntuDesktop2510),
            "ubuntu-server-jammy" => Some(Self::UbuntuServerJammy),
            "ubuntu-desktop-jammy" => Some(Self::UbuntuDesktopJammy),
            "ubuntu-server-focal" => Some(Self::UbuntuServerFocal),
            "ubuntu-desktop-focal" => Some(Self::UbuntuDesktopFocal),
            "ubuntu-server-bionic" => Some(Self::UbuntuServerBionic),
            "ubuntu-desktop-bionic" => Some(Self::UbuntuDesktopBionic),
            "linux-mint-cinnamon" => Some(Self::LinuxMintCinnamon),
            "linux-mint-mate" => Some(Self::LinuxMintMate),
            "linux-mint-xfce" => Some(Self::LinuxMintXfce),
            "fedora-server" => Some(Self::FedoraServer),
            "fedora-workstation" => Some(Self::FedoraWorkstation),
            "fedora-kde" => Some(Self::FedoraKde),
            "rocky-linux" => Some(Self::RockyLinux),
            "almalinux" => Some(Self::AlmaLinux),
            "centos-stream" => Some(Self::CentOsStream),
            "rhel-custom" => Some(Self::RhelCustom),
            "arch-linux" => Some(Self::ArchLinux),
            "endeavouros" => Some(Self::EndeavourOs),
            "garuda-dr460nized" => Some(Self::GarudaDr460nized),
            "garuda-gnome" => Some(Self::GarudaGnome),
            "garuda-xfce" => Some(Self::GarudaXfce),
            "manjaro" => Some(Self::Manjaro),
            "debian-netinst" => Some(Self::DebianNetInst),
            "opensuse-leap" => Some(Self::OpenSuseLeap),
            "opensuse-leap-net" => Some(Self::OpenSuseLeapNet),
            "opensuse-tumbleweed" => Some(Self::OpenSuseTumbleweed),
            "kali-linux" => Some(Self::KaliLinux),
            "kali-linux-netinst" => Some(Self::KaliLinuxNetinst),
            "pop-os-22-intel" => Some(Self::PopOs22Intel),
            "pop-os-22-nvidia" => Some(Self::PopOs22Nvidia),
            "pop-os-24-intel" => Some(Self::PopOs24Intel),
            _ => None,
        }
    }

    /// Return the canonical kebab-case name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UbuntuServerLts => "ubuntu-server-lts",
            Self::UbuntuDesktopLts => "ubuntu-desktop-lts",
            Self::UbuntuServer2510 => "ubuntu-server-2510",
            Self::UbuntuDesktop2510 => "ubuntu-desktop-2510",
            Self::UbuntuServerJammy => "ubuntu-server-jammy",
            Self::UbuntuDesktopJammy => "ubuntu-desktop-jammy",
            Self::UbuntuServerFocal => "ubuntu-server-focal",
            Self::UbuntuDesktopFocal => "ubuntu-desktop-focal",
            Self::UbuntuServerBionic => "ubuntu-server-bionic",
            Self::UbuntuDesktopBionic => "ubuntu-desktop-bionic",
            Self::LinuxMintCinnamon => "linux-mint-cinnamon",
            Self::LinuxMintMate => "linux-mint-mate",
            Self::LinuxMintXfce => "linux-mint-xfce",
            Self::FedoraServer => "fedora-server",
            Self::FedoraWorkstation => "fedora-workstation",
            Self::FedoraKde => "fedora-kde",
            Self::RockyLinux => "rocky-linux",
            Self::AlmaLinux => "almalinux",
            Self::CentOsStream => "centos-stream",
            Self::RhelCustom => "rhel-custom",
            Self::ArchLinux => "arch-linux",
            Self::EndeavourOs => "endeavouros",
            Self::GarudaDr460nized => "garuda-dr460nized",
            Self::GarudaGnome => "garuda-gnome",
            Self::GarudaXfce => "garuda-xfce",
            Self::Manjaro => "manjaro",
            Self::DebianNetInst => "debian-netinst",
            Self::OpenSuseLeap => "opensuse-leap",
            Self::OpenSuseLeapNet => "opensuse-leap-net",
            Self::OpenSuseTumbleweed => "opensuse-tumbleweed",
            Self::KaliLinux => "kali-linux",
            Self::KaliLinuxNetinst => "kali-linux-netinst",
            Self::PopOs22Intel => "pop-os-22-intel",
            Self::PopOs22Nvidia => "pop-os-22-nvidia",
            Self::PopOs24Intel => "pop-os-24-intel",
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
    // ── Ubuntu (all via mirror.xenyth.net/ubuntu-releases) ───────────────────
    IsoPreset {
        id: PresetId::UbuntuServerLts,
        name: "Ubuntu 24.04.4 LTS Server",
        distro: "ubuntu",
        edition: "server-lts",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/server",
        direct_url: Some(
            "https://mirror.xenyth.net/ubuntu-releases/24.04.4/ubuntu-24.04.4-live-server-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/noble/SHA256SUMS"),
        filename_suffix: Some("-live-server-amd64.iso"),
        note: "Ubuntu 24.04.4 LTS Server (Noble) — unattended via cloud-init autoinstall",
    },
    IsoPreset {
        id: PresetId::UbuntuDesktopLts,
        name: "Ubuntu 24.04.4 LTS Desktop",
        distro: "ubuntu",
        edition: "desktop-lts",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/desktop",
        direct_url: Some(
            "https://mirror.xenyth.net/ubuntu-releases/24.04.4/ubuntu-24.04.4-desktop-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/noble/SHA256SUMS"),
        filename_suffix: Some("-desktop-amd64.iso"),
        note: "Ubuntu 24.04.4 LTS Desktop (Noble) — autoinstall supported since 23.04",
    },
    IsoPreset {
        id: PresetId::UbuntuServer2510,
        name: "Ubuntu 25.10 Server",
        distro: "ubuntu",
        edition: "server",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/server",
        direct_url: Some(
            "https://mirror.xenyth.net/ubuntu-releases/25.10/ubuntu-25.10-live-server-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/questing/SHA256SUMS"),
        filename_suffix: Some("-live-server-amd64.iso"),
        note: "Ubuntu 25.10 Server (Questing) — non-LTS; cloud-init autoinstall",
    },
    IsoPreset {
        id: PresetId::UbuntuDesktop2510,
        name: "Ubuntu 25.10 Desktop",
        distro: "ubuntu",
        edition: "desktop",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/desktop",
        direct_url: Some(
            "https://mirror.xenyth.net/ubuntu-releases/25.10/ubuntu-25.10-desktop-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/questing/SHA256SUMS"),
        filename_suffix: Some("-desktop-amd64.iso"),
        note: "Ubuntu 25.10 Desktop (Questing) — non-LTS",
    },
    IsoPreset {
        id: PresetId::UbuntuServerJammy,
        name: "Ubuntu 22.04.5 LTS Server",
        distro: "ubuntu",
        edition: "server",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/server",
        direct_url: Some(
            "https://mirror.xenyth.net/ubuntu-releases/22.04.5/ubuntu-22.04.5-live-server-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/jammy/SHA256SUMS"),
        filename_suffix: Some("-live-server-amd64.iso"),
        note: "Ubuntu 22.04.5 LTS Server (Jammy) — cloud-init autoinstall",
    },
    IsoPreset {
        id: PresetId::UbuntuDesktopJammy,
        name: "Ubuntu 22.04.5 LTS Desktop",
        distro: "ubuntu",
        edition: "desktop",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/desktop",
        direct_url: Some(
            "https://mirror.xenyth.net/ubuntu-releases/22.04.5/ubuntu-22.04.5-desktop-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/jammy/SHA256SUMS"),
        filename_suffix: Some("-desktop-amd64.iso"),
        note: "Ubuntu 22.04.5 LTS Desktop (Jammy) — supported until April 2027",
    },
    IsoPreset {
        id: PresetId::UbuntuServerFocal,
        name: "Ubuntu 20.04.6 LTS Server",
        distro: "ubuntu",
        edition: "server",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/server",
        direct_url: Some(
            "https://mirror.xenyth.net/ubuntu-releases/20.04.6/ubuntu-20.04.6-live-server-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/focal/SHA256SUMS"),
        filename_suffix: Some("-live-server-amd64.iso"),
        note: "Ubuntu 20.04.6 LTS Server (Focal) — cloud-init autoinstall",
    },
    IsoPreset {
        id: PresetId::UbuntuDesktopFocal,
        name: "Ubuntu 20.04.6 LTS Desktop",
        distro: "ubuntu",
        edition: "desktop",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/desktop",
        direct_url: Some(
            "https://mirror.xenyth.net/ubuntu-releases/20.04.6/ubuntu-20.04.6-desktop-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/focal/SHA256SUMS"),
        filename_suffix: Some("-desktop-amd64.iso"),
        note: "Ubuntu 20.04.6 LTS Desktop (Focal) — supported until April 2025",
    },
    IsoPreset {
        id: PresetId::UbuntuServerBionic,
        name: "Ubuntu 18.04.6 LTS Server",
        distro: "ubuntu",
        edition: "server",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/server",
        direct_url: Some(
            "https://mirror.xenyth.net/ubuntu-releases/18.04.6/ubuntu-18.04.6-live-server-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/bionic/SHA256SUMS"),
        filename_suffix: Some("-live-server-amd64.iso"),
        note: "Ubuntu 18.04.6 LTS Server (Bionic) — ESM only; legacy deployments",
    },
    IsoPreset {
        id: PresetId::UbuntuDesktopBionic,
        name: "Ubuntu 18.04.6 LTS Desktop",
        distro: "ubuntu",
        edition: "desktop",
        architecture: "x86_64",
        strategy: AcquisitionStrategy::DirectUrl,
        official_page: "https://ubuntu.com/download/desktop",
        direct_url: Some(
            "https://mirror.xenyth.net/ubuntu-releases/18.04.6/ubuntu-18.04.6-desktop-amd64.iso",
        ),
        checksum_url: Some("https://releases.ubuntu.com/bionic/SHA256SUMS"),
        filename_suffix: Some("-desktop-amd64.iso"),
        note: "Ubuntu 18.04.6 LTS Desktop (Bionic) — ESM only; legacy deployments",
    },
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
            "https://dl.fedoraproject.org/pub/fedora/linux/releases/42/KDE/x86_64/iso/Fedora-KDE-Desktop-42-1.1-x86_64-CHECKSUM",
        ),
        filename_suffix: Some("-KDE-Desktop-Live-"),
        note: "Fedora 42 KDE — Plasma desktop live spin",
    },
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
    // ── Debian ───────────────────────────────────────────────────────────────
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
    // ── openSUSE — kernel.org mirror ─────────────────────────────────────────
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
    // ── Security ─────────────────────────────────────────────────────────────
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
    // ── Pop!_OS — iso.pop-os.org ──────────────────────────────────────────────
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
        checksum_url: Some(
            "https://iso.pop-os.org/22.04/amd64/intel/46/pop-os_22.04_amd64_intel_46.iso.sha256",
        ),
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
        checksum_url: Some(
            "https://iso.pop-os.org/22.04/amd64/nvidia/46/pop-os_22.04_amd64_nvidia_46.iso.sha256",
        ),
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
        checksum_url: Some(
            "https://iso.pop-os.org/24.04/amd64/intel/9/pop-os_24.04_amd64_intel_9.iso.sha256",
        ),
        filename_suffix: Some("_amd64_intel_"),
        note: "Pop!_OS 24.04 — Intel/AMD GPU build; Ubuntu 24.04 base",
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
    fn all_presets_returns_thirty_five_items() {
        assert_eq!(all_presets().len(), 35);
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
        assert!(url.unwrap().contains("mirror.xenyth.net"));
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
        let preset = find_preset_by_str("manjaro").unwrap();
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
        // manjaro uses a discovery page — build-stamped filenames prevent a stable direct URL
        let p = find_preset_by_str("manjaro").expect("manjaro must exist");
        assert_eq!(p.strategy, AcquisitionStrategy::DiscoveryPage);
        assert!(
            resolve_url(p).unwrap().is_none(),
            "manjaro should resolve to None"
        );
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

    // ── find_preset_by_str edge cases ─────────────────────────────────────────

    #[test]
    fn find_preset_by_str_empty_returns_none() {
        assert!(
            find_preset_by_str("").is_none(),
            "empty string must return None — no preset has an empty ID"
        );
    }

    #[test]
    fn find_preset_by_str_whitespace_returns_none() {
        assert!(
            find_preset_by_str("   ").is_none(),
            "whitespace-only string must return None"
        );
    }

    #[test]
    fn find_preset_by_str_case_insensitive_matches() {
        // The function documents case-insensitive lookup; all casing variants
        // of a valid ID must resolve to the same preset.
        let lower = find_preset_by_str("ubuntu-server-lts");
        let upper = find_preset_by_str("UBUNTU-SERVER-LTS");
        let mixed = find_preset_by_str("Ubuntu-Server-LTS");
        assert!(lower.is_some(), "lowercase must match");
        assert!(upper.is_some(), "uppercase must match");
        assert!(mixed.is_some(), "mixed-case must match");
        assert_eq!(lower.unwrap().id, upper.unwrap().id);
        assert_eq!(lower.unwrap().id, mixed.unwrap().id);
    }

    #[test]
    fn find_preset_by_str_partial_id_returns_none() {
        // "ubuntu" alone must not match "ubuntu-server-lts".
        assert!(
            find_preset_by_str("ubuntu").is_none(),
            "partial ID must not match — exact equality required"
        );
    }

    // ── Catalog invariants ────────────────────────────────────────────────────

    #[test]
    fn all_presets_have_non_empty_names_and_distros() {
        for preset in all_presets() {
            assert!(
                !preset.name.is_empty(),
                "preset {} has empty name",
                preset.id.as_str()
            );
            assert!(
                !preset.distro.is_empty(),
                "preset {} has empty distro",
                preset.id.as_str()
            );
            assert!(
                !preset.edition.is_empty(),
                "preset {} has empty edition",
                preset.id.as_str()
            );
        }
    }

    #[test]
    fn all_direct_url_presets_have_direct_url_set() {
        for preset in all_presets() {
            if preset.strategy == AcquisitionStrategy::DirectUrl {
                assert!(
                    preset.direct_url.is_some(),
                    "DirectUrl preset {} must have direct_url set",
                    preset.id.as_str()
                );
            }
        }
    }

    #[test]
    fn discovery_page_and_user_provided_presets_have_no_direct_url() {
        for preset in all_presets() {
            if matches!(
                preset.strategy,
                AcquisitionStrategy::DiscoveryPage | AcquisitionStrategy::UserProvided
            ) {
                assert!(
                    preset.direct_url.is_none(),
                    "Non-DirectUrl preset {} must not have direct_url set",
                    preset.id.as_str()
                );
            }
        }
    }

    #[test]
    fn all_preset_ids_are_unique() {
        let ids: Vec<&str> = all_presets().iter().map(|p| p.id.as_str()).collect();
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(seen.insert(*id), "duplicate preset id: {id}");
        }
    }

    #[test]
    fn all_presets_have_official_page_starting_with_https() {
        for preset in all_presets() {
            assert!(
                preset.official_page.starts_with("https://"),
                "preset {} official_page must use HTTPS, got: {}",
                preset.id.as_str(),
                preset.official_page
            );
        }
    }

    #[test]
    fn resolve_url_user_provided_always_returns_none() {
        // rhel-custom is UserProvided — user must supply their own ISO path.
        let p = find_preset_by_str("rhel-custom").expect("rhel-custom must exist");
        assert_eq!(p.strategy, AcquisitionStrategy::UserProvided);
        assert!(
            resolve_url(p).unwrap().is_none(),
            "UserProvided must always resolve to None"
        );
    }

    #[test]
    fn resolve_url_direct_url_returns_https_url() {
        // ubuntu-server-lts is DirectUrl — resolve_url must return the CDN URL.
        let p = find_preset_by_str("ubuntu-server-lts").expect("ubuntu-server-lts must exist");
        assert_eq!(p.strategy, AcquisitionStrategy::DirectUrl);
        let url = resolve_url(p)
            .expect("resolve must not error")
            .expect("must return Some URL");
        assert!(
            url.starts_with("https://"),
            "resolved URL must be HTTPS, got: {url}"
        );
    }
}
