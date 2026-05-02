//! System-level setters: locale/timezone/keyboard, target distro,
//! interactivity, wallpaper.

use std::path::PathBuf;

use super::InjectConfigBuilder;
use crate::config::Distro;

impl InjectConfigBuilder {
    /// Set the IANA timezone identifier injected into the installer (e.g. `"America/New_York"`).
    #[must_use]
    pub fn timezone(mut self, val: impl Into<String>) -> Self {
        self.timezone = Some(val.into());
        self
    }

    /// Set the locale string injected into the installer (e.g. `"en_US.UTF-8"`).
    #[must_use]
    pub fn locale(mut self, val: impl Into<String>) -> Self {
        self.locale = Some(val.into());
        self
    }

    /// Set the XKB keyboard layout code injected into the installer (e.g. `"us"`, `"gb"`).
    #[must_use]
    pub fn keyboard_layout(mut self, val: impl Into<String>) -> Self {
        self.keyboard_layout = Some(val.into());
        self
    }

    /// When `true`, suppress all interactive installer prompts for a fully unattended install.
    #[must_use]
    pub fn no_user_interaction(mut self, val: bool) -> Self {
        self.no_user_interaction = Some(val);
        self
    }

    /// Set the path to an image file that will be copied to the ISO and set as the default GNOME wallpaper.
    #[must_use]
    pub fn wallpaper(mut self, val: impl Into<PathBuf>) -> Self {
        self.wallpaper = Some(val.into());
        self
    }

    /// Set the target distro family, selecting which installer config format is generated.
    #[must_use]
    pub fn distro(mut self, val: Distro) -> Self {
        self.distro = Some(val);
        self
    }
}
