use serde::{Deserialize, Serialize};

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
