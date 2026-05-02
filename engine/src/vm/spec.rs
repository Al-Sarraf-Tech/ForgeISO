use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::firmware::FirmwareMode;
use super::hypervisor::Hypervisor;
use super::ovmf::find_ovmf;

/// A fully-specified VM launch configuration.
///
/// Build with `VmLaunchSpec::new()` for sensible defaults, then adjust fields
/// before passing to `emit_launch()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmLaunchSpec {
    /// Hypervisor to target when generating launch commands or scripts.
    pub hypervisor: Hypervisor,
    /// Firmware mode (BIOS or UEFI) for this VM.
    pub firmware: FirmwareMode,
    /// Absolute path to the ISO to boot.
    pub iso_path: PathBuf,
    /// RAM allocation in mebibytes (default 2048 MiB).
    pub ram_mb: u32,
    /// Number of virtual CPUs (default 2).
    pub cpus: u8,
    /// Size of the ephemeral scratch disk in gibibytes (default 20 GiB).
    pub disk_gb: u32,
    /// Sanitized VM name used in hypervisor APIs and shell paths.
    pub vm_name: String,
    /// Resolved OVMF firmware path (QEMU / Proxmox UEFI only).
    pub ovmf_path: Option<PathBuf>,
}

impl VmLaunchSpec {
    /// Create a launch spec with sensible defaults derived from the ISO path.
    ///
    /// The VM name is sanitized to contain only `[a-zA-Z0-9._-]` so it is safe
    /// to embed in shell commands, QEMU `-drive file=` paths, and hypervisor APIs
    /// without quoting or escaping issues.
    pub fn new(iso_path: &Path, hypervisor: Hypervisor, firmware: FirmwareMode) -> Self {
        let raw_name = iso_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("forgeiso-vm");
        let vm_name = sanitize_vm_name(raw_name);
        let ovmf_path = if matches!(firmware, FirmwareMode::Uefi) {
            find_ovmf()
        } else {
            None
        };
        Self {
            hypervisor,
            firmware,
            iso_path: iso_path.to_path_buf(),
            ram_mb: 2048,
            cpus: 2,
            disk_gb: 20,
            vm_name,
            ovmf_path,
        }
    }
}

/// Sanitize a VM name to contain only shell/path-safe characters.
/// Replaces anything outside `[a-zA-Z0-9._-]` with `-` and trims leading/trailing dashes.
/// Falls back to `"forgeiso-vm"` if the result is empty.
pub(super) fn sanitize_vm_name(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "forgeiso-vm".to_string()
    } else {
        trimmed.to_string()
    }
}
