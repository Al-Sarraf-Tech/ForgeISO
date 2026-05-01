//! System-level setters: locale/timezone/keyboard, target distro,
//! interactivity, wallpaper.

use std::path::PathBuf;

use super::InjectConfigBuilder;
use crate::config::Distro;

impl InjectConfigBuilder {
    #[must_use]
    pub fn timezone(mut self, val: impl Into<String>) -> Self {
        self.timezone = Some(val.into());
        self
    }

    #[must_use]
    pub fn locale(mut self, val: impl Into<String>) -> Self {
        self.locale = Some(val.into());
        self
    }

    #[must_use]
    pub fn keyboard_layout(mut self, val: impl Into<String>) -> Self {
        self.keyboard_layout = Some(val.into());
        self
    }

    #[must_use]
    pub fn no_user_interaction(mut self, val: bool) -> Self {
        self.no_user_interaction = Some(val);
        self
    }

    #[must_use]
    pub fn wallpaper(mut self, val: impl Into<PathBuf>) -> Self {
        self.wallpaper = Some(val.into());
        self
    }

    #[must_use]
    pub fn distro(mut self, val: Distro) -> Self {
        self.distro = Some(val);
        self
    }
}
