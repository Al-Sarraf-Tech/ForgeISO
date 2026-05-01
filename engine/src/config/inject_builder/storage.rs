//! Storage-related setters: layout, LUKS encryption, custom mounts, swap.

use super::InjectConfigBuilder;
use crate::config::components::SwapConfig;

impl InjectConfigBuilder {
    #[must_use]
    pub fn storage_layout(mut self, val: impl Into<String>) -> Self {
        self.storage_layout = Some(val.into());
        self
    }

    #[must_use]
    pub fn encrypt(mut self, val: bool) -> Self {
        self.encrypt = Some(val);
        self
    }

    #[must_use]
    pub fn encrypt_passphrase(mut self, val: impl Into<String>) -> Self {
        self.encrypt_passphrase = Some(val.into());
        self
    }

    #[must_use]
    pub fn mounts(mut self, val: Vec<String>) -> Self {
        self.mounts = Some(val);
        self
    }

    #[must_use]
    pub fn swap(mut self, val: SwapConfig) -> Self {
        self.swap = Some(val);
        self
    }
}
