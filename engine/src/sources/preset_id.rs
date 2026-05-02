use serde::{Deserialize, Serialize};

/// A well-known distro edition that ForgeISO knows how to find.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PresetId {
    /// Ubuntu 24.04 LTS Noble server edition via Xenyth CDN mirror.
    UbuntuServerLts,
    /// Ubuntu 24.04 LTS Noble desktop edition via Xenyth CDN mirror.
    UbuntuDesktopLts,
    /// Ubuntu 25.10 Questing (non-LTS) server edition via Xenyth CDN mirror.
    UbuntuServer2510,
    /// Ubuntu 25.10 Questing (non-LTS) desktop edition via Xenyth CDN mirror.
    UbuntuDesktop2510,
    /// Ubuntu 22.04 Jammy LTS server edition via Xenyth CDN mirror.
    UbuntuServerJammy,
    /// Ubuntu 22.04 Jammy LTS desktop edition via Xenyth CDN mirror.
    UbuntuDesktopJammy,
    /// Ubuntu 20.04 Focal LTS server edition via Xenyth CDN mirror.
    UbuntuServerFocal,
    /// Ubuntu 20.04 Focal LTS desktop edition via Xenyth CDN mirror.
    UbuntuDesktopFocal,
    /// Ubuntu 18.04 Bionic LTS server edition via Xenyth CDN mirror.
    UbuntuServerBionic,
    /// Ubuntu 18.04 Bionic LTS desktop edition via Xenyth CDN mirror.
    UbuntuDesktopBionic,
    /// Linux Mint 22.3 Zena with Cinnamon desktop via kernel.org mirror.
    LinuxMintCinnamon,
    /// Linux Mint 22.3 Zena with MATE desktop via kernel.org mirror.
    LinuxMintMate,
    /// Linux Mint 22.3 Zena with Xfce desktop via kernel.org mirror.
    LinuxMintXfce,
    /// Fedora 42 server edition via dl.fedoraproject.org.
    FedoraServer,
    /// Fedora 42 Workstation (GNOME) edition via dl.fedoraproject.org.
    FedoraWorkstation,
    /// Fedora 42 KDE Plasma spin via dl.fedoraproject.org.
    FedoraKde,
    /// Rocky Linux latest stable release via dl.rockylinux.org.
    RockyLinux,
    /// AlmaLinux latest stable release via repo.almalinux.org.
    AlmaLinux,
    /// CentOS Stream latest release via mirror.stream.centos.org.
    CentOsStream,
    /// RHEL custom — user must supply their own ISO path or URL.
    RhelCustom,
    /// Arch Linux latest monthly snapshot via archlinux.org mirrors.
    ArchLinux,
    /// EndeavourOS latest release via EndeavourOS CDN.
    EndeavourOs,
    /// Garuda Linux dr460nized (KDE) edition via sourceforge.net.
    GarudaDr460nized,
    /// Garuda Linux GNOME edition via sourceforge.net.
    GarudaGnome,
    /// Garuda Linux Xfce edition via sourceforge.net.
    GarudaXfce,
    /// Manjaro latest release — requires discovery page visit for stable URL.
    Manjaro,
    /// Debian latest stable netinstall image via cdimage.debian.org.
    DebianNetInst,
    /// openSUSE Leap latest full installer via kernel.org mirror.
    OpenSuseLeap,
    /// openSUSE Leap latest net installer via kernel.org mirror.
    OpenSuseLeapNet,
    /// openSUSE Tumbleweed rolling release via kernel.org mirror.
    OpenSuseTumbleweed,
    /// Kali Linux latest rolling release via cdimage.kali.org.
    KaliLinux,
    /// Kali Linux latest network installer via cdimage.kali.org.
    KaliLinuxNetinst,
    /// Pop!_OS 22.04 LTS Intel/AMD edition via iso.pop-os.org.
    PopOs22Intel,
    /// Pop!_OS 22.04 LTS Nvidia edition via iso.pop-os.org.
    PopOs22Nvidia,
    /// Pop!_OS 24.04 LTS Intel/AMD edition via iso.pop-os.org.
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
