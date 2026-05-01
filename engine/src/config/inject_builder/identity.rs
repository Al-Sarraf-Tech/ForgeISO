//! Identity setters: autoinstall, output labels/checksums, hostname/user.

use std::path::PathBuf;

use super::InjectConfigBuilder;

impl InjectConfigBuilder {
    #[must_use]
    pub fn autoinstall_yaml(mut self, val: impl Into<PathBuf>) -> Self {
        self.autoinstall_yaml = Some(val.into());
        self
    }

    #[must_use]
    pub fn output_label(mut self, val: impl Into<String>) -> Self {
        self.output_label = Some(val.into());
        self
    }

    #[must_use]
    pub fn expected_sha256(mut self, val: impl Into<String>) -> Self {
        self.expected_sha256 = Some(val.into());
        self
    }

    #[must_use]
    pub fn hostname(mut self, val: impl Into<String>) -> Self {
        self.hostname = Some(val.into());
        self
    }

    #[must_use]
    pub fn username(mut self, val: impl Into<String>) -> Self {
        self.username = Some(val.into());
        self
    }

    #[must_use]
    pub fn password(mut self, val: impl Into<String>) -> Self {
        self.password = Some(val.into());
        self
    }

    #[must_use]
    pub fn realname(mut self, val: impl Into<String>) -> Self {
        self.realname = Some(val.into());
        self
    }
}
