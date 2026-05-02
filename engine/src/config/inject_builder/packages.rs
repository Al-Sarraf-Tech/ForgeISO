//! Package-management setters: APT/DNF/Pacman repos and mirrors,
//! extra packages, container runtime selection.

use super::InjectConfigBuilder;
use crate::config::components::ContainerConfig;

impl InjectConfigBuilder {
    /// Set the list of additional packages installed via the distro's package manager.
    #[must_use]
    pub fn extra_packages(mut self, val: Vec<String>) -> Self {
        self.extra_packages = Some(val);
        self
    }

    /// Override the APT mirror URL used during installation (Ubuntu/Debian/Mint only).
    #[must_use]
    pub fn apt_mirror(mut self, val: impl Into<String>) -> Self {
        self.apt_mirror = Some(val.into());
        self
    }

    /// Set extra APT repository entries: PPA shorthand (`ppa:user/repo`) or full `deb …` lines.
    #[must_use]
    pub fn apt_repos(mut self, val: Vec<String>) -> Self {
        self.apt_repos = Some(val);
        self
    }

    /// Set DNF repository entries for Fedora/RHEL: URL strings or full `.repo` stanzas.
    #[must_use]
    pub fn dnf_repos(mut self, val: Vec<String>) -> Self {
        self.dnf_repos = Some(val);
        self
    }

    /// Override the primary DNF mirror base URL for `fedora.repo` and `fedora-updates.repo`.
    #[must_use]
    pub fn dnf_mirror(mut self, val: impl Into<String>) -> Self {
        self.dnf_mirror = Some(val.into());
        self
    }

    /// Set Pacman repository mirror lines for Arch Linux (each entry is a `Server = https://...` line).
    #[must_use]
    pub fn pacman_repos(mut self, val: Vec<String>) -> Self {
        self.pacman_repos = Some(val);
        self
    }

    /// Set the primary Pacman mirror URL written as the first `Server=` entry in the mirrorlist.
    #[must_use]
    pub fn pacman_mirror(mut self, val: impl Into<String>) -> Self {
        self.pacman_mirror = Some(val.into());
        self
    }

    /// Configure Docker CE and/or Podman installation and optional Docker group membership.
    #[must_use]
    pub fn containers(mut self, val: ContainerConfig) -> Self {
        self.containers = Some(val);
        self
    }
}
