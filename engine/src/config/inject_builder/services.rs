//! Service / boot / kernel setters: systemd services, sysctl tunables,
//! GRUB tweaks, runtime escape hatches (run_commands, late commands), and
//! the per-user access config.

use super::InjectConfigBuilder;
use crate::config::components::{GrubConfig, UserConfig};

impl InjectConfigBuilder {
    #[must_use]
    pub fn enable_services(mut self, val: Vec<String>) -> Self {
        self.enable_services = Some(val);
        self
    }

    #[must_use]
    pub fn disable_services(mut self, val: Vec<String>) -> Self {
        self.disable_services = Some(val);
        self
    }

    #[must_use]
    pub fn sysctl(mut self, val: Vec<(String, String)>) -> Self {
        self.sysctl = Some(val);
        self
    }

    #[must_use]
    pub fn grub(mut self, val: GrubConfig) -> Self {
        self.grub = Some(val);
        self
    }

    #[must_use]
    pub fn run_commands(mut self, val: Vec<String>) -> Self {
        self.run_commands = Some(val);
        self
    }

    #[must_use]
    pub fn extra_late_commands(mut self, val: Vec<String>) -> Self {
        self.extra_late_commands = Some(val);
        self
    }

    #[must_use]
    pub fn user(mut self, val: UserConfig) -> Self {
        self.user = Some(val);
        self
    }
}
