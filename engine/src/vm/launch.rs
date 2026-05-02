use std::path::Path;

use serde::{Deserialize, Serialize};

use super::firmware::FirmwareMode;
use super::hyperv::hyperv_ps1;
use super::hypervisor::Hypervisor;
use super::proxmox::proxmox_cmds;
use super::qemu::{maybe_remove_kvm, qemu_bios_args, qemu_uefi_args};
use super::spec::VmLaunchSpec;
use super::vbox::vbox_commands;
use super::vmware::vmware_instructions;

/// Combined output produced by `emit_launch()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmLaunchOutput {
    /// Hypervisor this output targets.
    pub hypervisor: Hypervisor,
    /// Firmware mode (BIOS or UEFI) this output was generated for.
    pub firmware: FirmwareMode,
    /// Absolute path of the source ISO as a string, for embedding in scripts.
    pub iso_path: String,
    /// Shell commands (QEMU, VirtualBox, Proxmox).
    pub commands: Vec<String>,
    /// Script content (VMware, Hyper-V).
    pub script: Option<String>,
    /// Whether `/dev/kvm` is present on the current host.
    pub kvm_available: bool,
    /// OVMF path used (UEFI boots only).
    pub ovmf_used: Option<String>,
    /// Human-readable notes (warnings, tips).
    pub notes: Vec<String>,
}

/// Generate all launch information for a given `VmLaunchSpec`.
///
/// This is the primary entry point for consumers of this module.
pub fn emit_launch(spec: &VmLaunchSpec) -> VmLaunchOutput {
    let kvm_available = Path::new("/dev/kvm").exists();
    let mut notes = Vec::new();

    let (commands, script) = match spec.hypervisor {
        Hypervisor::Qemu => {
            let base_args = match spec.firmware {
                FirmwareMode::Bios => qemu_bios_args(spec),
                FirmwareMode::Uefi => qemu_uefi_args(spec),
            };
            let args = maybe_remove_kvm(base_args);
            notes.push("Requires QEMU (qemu-system-x86_64) installed on the host.".to_string());
            if !kvm_available {
                notes.push(
                    "KVM is not available; running in software emulation (slow).".to_string(),
                );
            }
            if matches!(spec.firmware, FirmwareMode::Uefi) && spec.ovmf_path.is_none() {
                notes.push(
                    "OVMF firmware not found on this host; \
                     install edk2-ovmf (Fedora/RHEL) or ovmf (Debian/Ubuntu)."
                        .to_string(),
                );
            }
            (args, None)
        }
        Hypervisor::VirtualBox => {
            notes.push("Requires VirtualBox 6.1+ installed on the host.".to_string());
            (vbox_commands(spec), None)
        }
        Hypervisor::Vmware => {
            notes.push(
                "vmrun is only available with VMware Workstation Pro; \
                 Player users must use the GUI."
                    .to_string(),
            );
            (vec![], Some(vmware_instructions(spec)))
        }
        Hypervisor::HyperV => {
            notes.push(
                "Script must be run in an elevated PowerShell session on a Windows host."
                    .to_string(),
            );
            (vec![], Some(hyperv_ps1(spec)))
        }
        Hypervisor::Proxmox => {
            notes.push(
                "VMID 9000 is used by convention; verify it is free before running.".to_string(),
            );
            (proxmox_cmds(spec), None)
        }
    };

    VmLaunchOutput {
        hypervisor: spec.hypervisor,
        firmware: spec.firmware,
        iso_path: spec.iso_path.display().to_string(),
        commands,
        script,
        kvm_available,
        ovmf_used: spec.ovmf_path.as_ref().map(|p| p.display().to_string()),
        notes,
    }
}
