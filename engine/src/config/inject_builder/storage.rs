//! Storage-related setters: layout, LUKS encryption, custom mounts, swap.

use super::InjectConfigBuilder;
use crate::config::components::SwapConfig;

impl InjectConfigBuilder {
    /// Set the disk partitioning scheme: `"lvm"` (default), `"direct"`, or `"zfs"`.
    #[must_use]
    pub fn storage_layout(mut self, val: impl Into<String>) -> Self {
        self.storage_layout = Some(val.into());
        self
    }

    /// Enable or disable LUKS full-disk encryption for the storage layout.
    #[must_use]
    pub fn encrypt(mut self, val: bool) -> Self {
        self.encrypt = Some(val);
        self
    }

    /// Set the LUKS passphrase embedded in the autoinstall storage config. Treat resulting ISOs as sensitive.
    #[must_use]
    pub fn encrypt_passphrase(mut self, val: impl Into<String>) -> Self {
        self.encrypt_passphrase = Some(val.into());
        self
    }

    /// Set additional `/etc/fstab` entries; each entry triggers `mkdir -p <mountpoint>` before the line is written.
    #[must_use]
    pub fn mounts(mut self, val: Vec<String>) -> Self {
        self.mounts = Some(val);
        self
    }

    /// Configure the swap file: size, optional filename, and optional swappiness tunable.
    #[must_use]
    pub fn swap(mut self, val: SwapConfig) -> Self {
        self.swap = Some(val);
        self
    }
}
