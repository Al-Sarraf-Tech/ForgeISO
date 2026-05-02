//! Curated [`InjectConfig`] presets per use-case.
//!
//! Each [`ProfileKind`] populates a fresh [`InjectConfig`] with sensible,
//! production-ready defaults for a specific deployment scenario (server,
//! hardened server, developer workstation, kiosk appliance, or minimal
//! cloud bootstrap). Profiles are distro-aware: package names, repo lines,
//! and service identifiers are adjusted for `ubuntu`, `mint`, `fedora`,
//! `rhel-family`, `arch`, `debian`, `opensuse`, and `popos`.
//!
//! Profiles are *purely additive* — they take an existing [`InjectConfig`]
//! and overwrite only the fields they care about, so users can layer
//! their own settings on top via the builder API afterwards.
//!
//! # Example
//!
//! ```
//! use forgeiso_engine::config::InjectConfig;
//! use forgeiso_engine::profiles::{Profile, ProfileKind};
//!
//! let base = InjectConfig::default();
//! let cfg = Profile::new(ProfileKind::ServerDefault).populate("ubuntu", base);
//! assert!(cfg.firewall.enabled);
//! assert!(cfg.extra_packages.contains(&"vim".to_string()));
//! ```

use crate::config::InjectConfig;

mod desktop_developer;
mod kiosk;
mod minimal_cloud;
mod server_default;
mod server_hardened;

#[cfg(test)]
mod tests;

/// One of the curated deployment profiles ForgeISO ships out of the box.
///
/// These are intentionally namespaced under `profiles::` rather than
/// re-using the existing top-level `ProfileKind` (which is a Minimal /
/// Desktop edition selector for source presets, not an operational
/// configuration preset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileKind {
    /// Sensible production defaults: SSH on, firewall on with common ports
    /// (22/80/443), NTP, baseline packages (vim, curl, git, htop), wheel/sudo
    /// group membership. No keys baked in — user adds their own.
    ServerDefault,
    /// Security-first: firewall default-deny with only 22/80/443 allowed,
    /// SSH password auth OFF, root login OFF, audit + fail2ban packages,
    /// strict sysctl, no container runtime by default.
    ServerHardened,
    /// Developer workstation: full toolchain (gcc/make/python3/nodejs/rust),
    /// docker enabled, common GUI desktop bits, codecs, no firewall blocking
    /// dev ports.
    DesktopDeveloper,
    /// Single-app kiosk appliance: auto-login, autostart browser, screen
    /// blank disabled, minimal package set.
    Kiosk,
    /// Tiny cloud-init friendly base: no extra packages, SSH password auth
    /// ON for cloud bootstrap, no GUI.
    MinimalCloud,
}

impl ProfileKind {
    /// Stable kebab-case identifier for serialisation and CLI surfaces.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ServerDefault => "server-default",
            Self::ServerHardened => "server-hardened",
            Self::DesktopDeveloper => "desktop-developer",
            Self::Kiosk => "kiosk",
            Self::MinimalCloud => "minimal-cloud",
        }
    }

    /// One-line human-readable summary for CLI / UI display.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::ServerDefault => "Production server defaults (SSH, firewall, baseline packages)",
            Self::ServerHardened => {
                "Hardened server (deny-by-default firewall, no password SSH, audit)"
            }
            Self::DesktopDeveloper => "Developer workstation (toolchain, docker, GUI extras)",
            Self::Kiosk => "Single-app appliance (auto-login, autostart browser)",
            Self::MinimalCloud => "Minimal cloud base (no extras, password SSH for first boot)",
        }
    }

    /// Iterate over every profile variant.
    #[must_use]
    pub fn all() -> &'static [ProfileKind] {
        &[
            Self::ServerDefault,
            Self::ServerHardened,
            Self::DesktopDeveloper,
            Self::Kiosk,
            Self::MinimalCloud,
        ]
    }
}

/// A profile bound to a specific [`ProfileKind`]. Wraps the populate
/// dispatch so callers can pass `Profile` around without needing to
/// match on the enum themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Profile {
    kind: ProfileKind,
}

impl Profile {
    /// Create a profile from its kind.
    #[must_use]
    pub const fn new(kind: ProfileKind) -> Self {
        Self { kind }
    }

    /// The underlying kind.
    #[must_use]
    pub const fn kind(self) -> ProfileKind {
        self.kind
    }

    /// Populate `base` with the profile's curated defaults.
    ///
    /// `distro` accepts either a preset id (`"ubuntu-server-lts"`,
    /// `"fedora-server"`, `"arch-linux"`, ...) or a family shorthand
    /// (`"ubuntu"`, `"fedora"`, `"rhel-family"`, `"arch"`, `"mint"`,
    /// `"debian"`, `"opensuse"`, `"popos"`). Unknown strings fall back
    /// to the Ubuntu/apt code path, which is the historical default.
    #[must_use]
    pub fn populate(self, distro: &str, base: InjectConfig) -> InjectConfig {
        match self.kind {
            ProfileKind::ServerDefault => server_default::populate(distro, base),
            ProfileKind::ServerHardened => server_hardened::populate(distro, base),
            ProfileKind::DesktopDeveloper => desktop_developer::populate(distro, base),
            ProfileKind::Kiosk => kiosk::populate(distro, base),
            ProfileKind::MinimalCloud => minimal_cloud::populate(distro, base),
        }
    }
}

/// Catalog query: which profiles do we recommend for a given preset id
/// or distro family? Returned in priority order (best match first).
///
/// Unknown distros yield `[ServerDefault]` as a safe default.
pub struct ProfileCatalog;

impl ProfileCatalog {
    /// Look up recommendations by preset id (`"ubuntu-server-lts"`)
    /// or family (`"ubuntu"`).
    #[must_use]
    pub fn recommended_for(distro: &str) -> Vec<ProfileKind> {
        let normalised = distro.trim().to_ascii_lowercase();
        match normalised.as_str() {
            // Ubuntu server family — production defaults.
            "ubuntu-server-lts"
            | "ubuntu-server-jammy"
            | "ubuntu-server-focal"
            | "ubuntu-server-bionic"
            | "ubuntu-server-2510" => {
                vec![ProfileKind::ServerDefault, ProfileKind::MinimalCloud]
            }
            // Fedora server — leans hardened.
            "fedora-server" => vec![ProfileKind::ServerHardened, ProfileKind::ServerDefault],
            // RHEL family — hardened by convention.
            "rocky-linux" | "almalinux" | "centos-stream" | "rhel-custom" | "rhel-family" => {
                vec![ProfileKind::ServerHardened, ProfileKind::ServerDefault]
            }
            // Arch — assumed developer workstation.
            "arch-linux" | "endeavouros" | "manjaro" | "garuda-dr460nized" | "garuda-gnome"
            | "garuda-xfce" | "arch" => {
                vec![ProfileKind::DesktopDeveloper]
            }
            // Mint — desktop-oriented.
            "linux-mint-cinnamon" | "linux-mint-mate" | "linux-mint-xfce" | "mint" => {
                vec![ProfileKind::DesktopDeveloper]
            }
            // Ubuntu desktop / Pop!_OS — developer-friendly desktop.
            "ubuntu-desktop-lts"
            | "ubuntu-desktop-jammy"
            | "ubuntu-desktop-focal"
            | "ubuntu-desktop-bionic"
            | "ubuntu-desktop-2510"
            | "pop-os-22-intel"
            | "pop-os-22-nvidia"
            | "pop-os-24-intel"
            | "popos" => vec![ProfileKind::DesktopDeveloper],
            // Fedora workstation/KDE — desktop developer.
            "fedora-workstation" | "fedora-kde" | "fedora" => {
                vec![ProfileKind::DesktopDeveloper, ProfileKind::ServerDefault]
            }
            // Generic ubuntu / debian / opensuse — server default.
            "ubuntu"
            | "debian"
            | "debian-netinst"
            | "opensuse"
            | "opensuse-leap"
            | "opensuse-leap-net"
            | "opensuse-tumbleweed" => {
                vec![ProfileKind::ServerDefault]
            }
            // Kali — security-leaning desktop.
            "kali-linux" | "kali-linux-netinst" => vec![ProfileKind::DesktopDeveloper],
            // Anything else — safe fallback.
            _ => vec![ProfileKind::ServerDefault],
        }
    }
}

/// Internal helper: which package manager family does `distro` belong to?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DistroFamily {
    Apt,
    Dnf,
    DnfWithEpel,
    Pacman,
    Zypper,
}

impl DistroFamily {
    pub(crate) fn classify(distro: &str) -> Self {
        let n = distro.trim().to_ascii_lowercase();
        match n.as_str() {
            "fedora" | "fedora-server" | "fedora-workstation" | "fedora-kde" => Self::Dnf,
            "rhel-family" | "rocky-linux" | "almalinux" | "centos-stream" | "rhel-custom" => {
                Self::DnfWithEpel
            }
            "arch" | "arch-linux" | "endeavouros" | "manjaro" | "garuda-dr460nized"
            | "garuda-gnome" | "garuda-xfce" => Self::Pacman,
            "opensuse" | "opensuse-leap" | "opensuse-leap-net" | "opensuse-tumbleweed" => {
                Self::Zypper
            }
            // Ubuntu / Mint / Debian / Pop!_OS / Kali / unknown -> apt.
            _ => Self::Apt,
        }
    }
}
