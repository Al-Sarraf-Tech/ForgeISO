//! Step 1 (Source) helpers on [`App`].
//!
//! Resolves the effective ISO source (manual override beats preset) and the
//! engine `Distro` derived from the user-set distro string.

use forgeiso_engine::{all_presets, Distro};

use super::App;

impl App {
    pub(crate) fn effective_source(&self) -> String {
        if !self.manual_source.trim().is_empty() {
            return self.manual_source.trim().to_string();
        }
        if let Some(idx) = self.preset_selected {
            let presets = all_presets();
            if let Some(p) = presets.get(idx) {
                if let Some(url) = p.direct_url {
                    return url.to_string();
                }
            }
        }
        String::new()
    }

    pub(crate) fn resolve_distro(&self) -> Option<Distro> {
        match self.distro.trim().to_lowercase().as_str() {
            "fedora" | "rhel" | "rocky" | "alma" | "centos" => Some(Distro::Fedora),
            "mint" => Some(Distro::Mint),
            "arch" => Some(Distro::Arch),
            "ubuntu" | "" => None,
            _ => None,
        }
    }
}
