//! Package-management setters: APT/DNF/Pacman repos and mirrors,
//! extra packages, container runtime selection.

use super::InjectConfigBuilder;
use crate::config::components::ContainerConfig;

impl InjectConfigBuilder {
    #[must_use]
    pub fn extra_packages(mut self, val: Vec<String>) -> Self {
        self.extra_packages = Some(val);
        self
    }

    #[must_use]
    pub fn apt_mirror(mut self, val: impl Into<String>) -> Self {
        self.apt_mirror = Some(val.into());
        self
    }

    #[must_use]
    pub fn apt_repos(mut self, val: Vec<String>) -> Self {
        self.apt_repos = Some(val);
        self
    }

    #[must_use]
    pub fn dnf_repos(mut self, val: Vec<String>) -> Self {
        self.dnf_repos = Some(val);
        self
    }

    #[must_use]
    pub fn dnf_mirror(mut self, val: impl Into<String>) -> Self {
        self.dnf_mirror = Some(val.into());
        self
    }

    #[must_use]
    pub fn pacman_repos(mut self, val: Vec<String>) -> Self {
        self.pacman_repos = Some(val);
        self
    }

    #[must_use]
    pub fn pacman_mirror(mut self, val: impl Into<String>) -> Self {
        self.pacman_mirror = Some(val.into());
        self
    }

    #[must_use]
    pub fn containers(mut self, val: ContainerConfig) -> Self {
        self.containers = Some(val);
        self
    }
}
