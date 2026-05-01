use std::path::{Path, PathBuf};

/// Well-known OVMF firmware paths, searched in order.
static OVMF_CANDIDATES: &[&str] = &[
    "/usr/share/OVMF/OVMF_CODE.fd",
    "/usr/share/ovmf/OVMF.fd",
    "/usr/share/OVMF/x64/OVMF_CODE.fd",
    "/usr/share/edk2/x64/OVMF_CODE.fd",
    "/usr/share/edk2-ovmf/OVMF_CODE.fd",
];

/// Find the system OVMF firmware file by checking common distro paths.
/// Returns the first existing path, or `None` if none are found.
pub fn find_ovmf() -> Option<PathBuf> {
    for candidate in OVMF_CANDIDATES {
        let p = Path::new(candidate);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}

/// Return all candidate OVMF paths (for documentation / diagnostics).
pub fn ovmf_candidates() -> &'static [&'static str] {
    OVMF_CANDIDATES
}
