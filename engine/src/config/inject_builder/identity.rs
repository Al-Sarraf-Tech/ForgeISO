//! Identity setters: autoinstall, output labels/checksums, hostname/user.

use std::path::PathBuf;

use super::InjectConfigBuilder;

impl InjectConfigBuilder {
    /// Override autoinstall YAML generation with a pre-existing YAML file at the given path.
    #[must_use]
    pub fn autoinstall_yaml(mut self, val: impl Into<PathBuf>) -> Self {
        self.autoinstall_yaml = Some(val.into());
        self
    }

    /// Set the ISO volume label (≤32 ASCII characters, no control characters).
    #[must_use]
    pub fn output_label(mut self, val: impl Into<String>) -> Self {
        self.output_label = Some(val.into());
        self
    }

    /// Require the source ISO to match this hex-encoded SHA-256 digest before injection proceeds.
    #[must_use]
    pub fn expected_sha256(mut self, val: impl Into<String>) -> Self {
        self.expected_sha256 = Some(val.into());
        self
    }

    /// Set the system hostname written to `/etc/hostname` and the cloud-init `local-hostname`.
    #[must_use]
    pub fn hostname(mut self, val: impl Into<String>) -> Self {
        self.hostname = Some(val.into());
        self
    }

    /// Set the primary user account name created during unattended installation.
    #[must_use]
    pub fn username(mut self, val: impl Into<String>) -> Self {
        self.username = Some(val.into());
        self
    }

    /// Set the plaintext password for the primary user; the engine hashes it to `$6$` format before writing.
    #[must_use]
    pub fn password(mut self, val: impl Into<String>) -> Self {
        self.password = Some(val.into());
        self
    }

    /// Set the full real name (GECOS field) for the primary user account.
    #[must_use]
    pub fn realname(mut self, val: impl Into<String>) -> Self {
        self.realname = Some(val.into());
        self
    }
}
