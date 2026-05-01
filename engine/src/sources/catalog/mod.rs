//! Built-in ISO preset catalog, split into per-distro-family submodules.
//!
//! Each submodule exports a `PRESETS` slice. The top-level [`ALL_PRESETS`]
//! lazily concatenates every per-distro slice into a single contiguous
//! `&'static [IsoPreset]` so that downstream callers can keep using a static
//! borrow without caring about how the catalog is organised internally.
//!
//! The current ordering (Ubuntu → Mint → Fedora → RHEL family → Arch
//! → Debian → openSUSE → Pop!_OS) is preserved exactly to avoid behaviour
//! changes for any callers that observe ordering (e.g. CLI list output).

use std::sync::LazyLock;

use super::preset::IsoPreset;

mod arch;
mod debian;
mod fedora;
mod mint;
mod opensuse;
mod popos;
mod rhel_family;
mod ubuntu;

/// All built-in presets, concatenated in stable order across distro families.
///
/// Cloning each [`IsoPreset`] here is cheap: every field is either a
/// `&'static str`, an `Option<&'static str>`, or a small enum. The clones
/// live in a `Vec` initialised once via [`LazyLock`], and external callers
/// see a `&'static [IsoPreset]` indistinguishable from a plain `static`
/// array.
pub(super) static ALL_PRESETS: LazyLock<Vec<IsoPreset>> = LazyLock::new(|| {
    let mut all: Vec<IsoPreset> = Vec::with_capacity(
        ubuntu::PRESETS.len()
            + mint::PRESETS.len()
            + fedora::PRESETS.len()
            + rhel_family::PRESETS.len()
            + arch::PRESETS.len()
            + debian::PRESETS.len()
            + opensuse::PRESETS.len()
            + popos::PRESETS.len(),
    );
    all.extend_from_slice(ubuntu::PRESETS);
    all.extend_from_slice(mint::PRESETS);
    all.extend_from_slice(fedora::PRESETS);
    all.extend_from_slice(rhel_family::PRESETS);
    all.extend_from_slice(arch::PRESETS);
    all.extend_from_slice(debian::PRESETS);
    all.extend_from_slice(opensuse::PRESETS);
    all.extend_from_slice(popos::PRESETS);
    all
});
