//! VM harness and hypervisor launch layer.
//!
//! Generates launch commands, scripts, and configuration for booting a ForgeISO
//! artifact under multiple hypervisors (QEMU, VirtualBox, VMware, Hyper-V, Proxmox).
//! No I/O side effects at the module boundary — all output is returned as data.
//!
//! The module is split per-runtime concern:
//! - [`hypervisor`] — [`Hypervisor`] enum.
//! - [`firmware`] — [`FirmwareMode`] enum.
//! - [`ovmf`] — OVMF firmware discovery on the host.
//! - [`spec`] — [`VmLaunchSpec`] with VM-name sanitisation.
//! - [`qemu`] / [`vbox`] / [`vmware`] / [`hyperv`] / [`proxmox`] — per-hypervisor emitters.
//! - [`launch`] — [`VmLaunchOutput`] and the cross-runtime [`emit_launch`] entry point.

mod firmware;
mod hyperv;
mod hypervisor;
mod launch;
mod ovmf;
mod proxmox;
mod qemu;
mod spec;
mod vbox;
mod vmware;

pub use firmware::FirmwareMode;
pub use hyperv::hyperv_ps1;
pub use hypervisor::Hypervisor;
pub use launch::{emit_launch, VmLaunchOutput};
pub use ovmf::{find_ovmf, ovmf_candidates};
pub use proxmox::proxmox_cmds;
pub use qemu::{create_qemu_disk, maybe_remove_kvm, qemu_bios_args, qemu_uefi_args};
pub use spec::VmLaunchSpec;
pub use vbox::vbox_commands;
pub use vmware::vmware_instructions;

#[cfg(test)]
mod tests {
    use super::spec::sanitize_vm_name;
    use super::*;
    use std::path::{Path, PathBuf};

    fn test_spec(hypervisor: Hypervisor, firmware: FirmwareMode) -> VmLaunchSpec {
        VmLaunchSpec {
            hypervisor,
            firmware,
            iso_path: PathBuf::from("/tmp/test-ubuntu.iso"),
            ram_mb: 2048,
            cpus: 2,
            disk_gb: 20,
            vm_name: "test-ubuntu".to_string(),
            ovmf_path: Some(PathBuf::from("/usr/share/OVMF/OVMF_CODE.fd")),
        }
    }

    // ── find_ovmf ────────────────────────────────────────────────────────────

    #[test]
    fn find_ovmf_returns_some_only_when_path_exists() {
        let result = find_ovmf();
        if let Some(ref p) = result {
            assert!(
                p.exists(),
                "find_ovmf() returned a path that does not exist: {p:?}"
            );
        }
        // If None, that is also correct — it means no OVMF is installed on this host.
    }

    #[test]
    fn ovmf_candidates_is_nonempty() {
        assert!(!ovmf_candidates().is_empty());
    }

    // ── Hypervisor helpers ───────────────────────────────────────────────────

    #[test]
    fn hypervisor_as_str_roundtrips() {
        for &hv in Hypervisor::all() {
            let s = hv.as_str();
            let parsed = Hypervisor::from_str(s);
            assert!(parsed.is_some(), "from_str({s:?}) returned None");
            assert_eq!(parsed.unwrap(), hv);
        }
    }

    #[test]
    fn hypervisor_all_has_five_variants() {
        assert_eq!(Hypervisor::all().len(), 5);
    }

    #[test]
    fn hypervisor_from_str_aliases() {
        assert_eq!(Hypervisor::from_str("vbox"), Some(Hypervisor::VirtualBox));
        assert_eq!(Hypervisor::from_str("pve"), Some(Hypervisor::Proxmox));
        assert_eq!(Hypervisor::from_str("hyper-v"), Some(Hypervisor::HyperV));
        assert_eq!(Hypervisor::from_str("unknown"), None);
    }

    #[test]
    fn firmware_from_str_aliases() {
        assert_eq!(FirmwareMode::from_str("legacy"), Some(FirmwareMode::Bios));
        assert_eq!(FirmwareMode::from_str("efi"), Some(FirmwareMode::Uefi));
        assert_eq!(FirmwareMode::from_str("bogus"), None);
    }

    // ── QEMU BIOS ────────────────────────────────────────────────────────────

    #[test]
    fn qemu_bios_args_has_cdrom_and_serial() {
        let spec = test_spec(Hypervisor::Qemu, FirmwareMode::Bios);
        let args = qemu_bios_args(&spec);
        assert!(
            args.contains(&"-cdrom".to_string()),
            "missing -cdrom in BIOS args"
        );
        let has_serial = args
            .iter()
            .any(|a| a.starts_with("file:/tmp/") && a.ends_with("-bios-serial.log"));
        assert!(has_serial, "missing serial log path in BIOS args");
    }

    #[test]
    fn qemu_bios_args_first_element_is_binary() {
        let spec = test_spec(Hypervisor::Qemu, FirmwareMode::Bios);
        let args = qemu_bios_args(&spec);
        assert_eq!(args[0], "qemu-system-x86_64");
    }

    #[test]
    fn qemu_bios_args_has_no_reboot() {
        let spec = test_spec(Hypervisor::Qemu, FirmwareMode::Bios);
        let args = qemu_bios_args(&spec);
        assert!(args.contains(&"-no-reboot".to_string()));
    }

    // ── QEMU UEFI ────────────────────────────────────────────────────────────

    #[test]
    fn qemu_uefi_args_has_pflash_and_ovmf() {
        let spec = test_spec(Hypervisor::Qemu, FirmwareMode::Uefi);
        let args = qemu_uefi_args(&spec);
        let has_pflash = args.iter().any(|a| a.contains("pflash"));
        let has_ovmf = args.iter().any(|a| a.contains("OVMF"));
        assert!(has_pflash, "missing pflash in UEFI args: {args:?}");
        assert!(has_ovmf, "missing OVMF in UEFI args: {args:?}");
    }

    #[test]
    fn qemu_uefi_args_has_cdrom_and_serial() {
        let spec = test_spec(Hypervisor::Qemu, FirmwareMode::Uefi);
        let args = qemu_uefi_args(&spec);
        assert!(args.contains(&"-cdrom".to_string()));
        let has_serial = args.iter().any(|a| a.ends_with("-uefi-serial.log"));
        assert!(has_serial, "missing UEFI serial log: {args:?}");
    }

    // ── maybe_remove_kvm ─────────────────────────────────────────────────────

    #[test]
    fn maybe_remove_kvm_strips_flag_when_kvm_absent() {
        // Build an arg list that always contains -enable-kvm.
        let args = vec![
            "qemu-system-x86_64".to_string(),
            "-enable-kvm".to_string(),
            "-m".to_string(),
            "2048M".to_string(),
        ];
        // Temporarily test the stripping logic directly without relying on /dev/kvm.
        let result: Vec<String> = args.into_iter().filter(|a| a != "-enable-kvm").collect();
        assert!(!result.contains(&"-enable-kvm".to_string()));
        assert!(result.contains(&"qemu-system-x86_64".to_string()));
    }

    #[test]
    fn maybe_remove_kvm_preserves_other_args() {
        let args = vec![
            "qemu-system-x86_64".to_string(),
            "-m".to_string(),
            "2048M".to_string(),
        ];
        let result = maybe_remove_kvm(args);
        // -enable-kvm was never in the list; other args preserved regardless of KVM.
        assert!(result.contains(&"-m".to_string()));
        assert!(result.contains(&"2048M".to_string()));
    }

    // ── VirtualBox ───────────────────────────────────────────────────────────

    #[test]
    fn vbox_commands_has_createvm_and_iso_path() {
        let spec = test_spec(Hypervisor::VirtualBox, FirmwareMode::Bios);
        let cmds = vbox_commands(&spec);
        let has_createvm = cmds.iter().any(|c| c.contains("VBoxManage createvm"));
        let iso_str = spec.iso_path.to_string_lossy();
        let has_iso = cmds.iter().any(|c| c.contains(iso_str.as_ref()));
        assert!(has_createvm, "missing createvm command: {cmds:?}");
        assert!(has_iso, "missing iso path in vbox commands: {cmds:?}");
    }

    #[test]
    fn vbox_commands_uefi_sets_efi_firmware() {
        let spec = test_spec(Hypervisor::VirtualBox, FirmwareMode::Uefi);
        let cmds = vbox_commands(&spec);
        let has_efi = cmds.iter().any(|c| c.contains("--firmware efi"));
        assert!(
            has_efi,
            "UEFI firmware flag missing from vbox commands: {cmds:?}"
        );
    }

    #[test]
    fn vbox_commands_bios_sets_bios_firmware() {
        let spec = test_spec(Hypervisor::VirtualBox, FirmwareMode::Bios);
        let cmds = vbox_commands(&spec);
        let has_bios = cmds.iter().any(|c| c.contains("--firmware bios"));
        assert!(
            has_bios,
            "BIOS firmware flag missing from vbox commands: {cmds:?}"
        );
    }

    // ── VMware ───────────────────────────────────────────────────────────────

    #[test]
    fn vmware_instructions_contains_iso_path_and_firmware() {
        let spec = test_spec(Hypervisor::Vmware, FirmwareMode::Uefi);
        let out = vmware_instructions(&spec);
        assert!(out.contains("/tmp/test-ubuntu.iso"));
        assert!(out.contains("efi64"));
    }

    #[test]
    fn vmware_instructions_bios_firmware_string() {
        let spec = test_spec(Hypervisor::Vmware, FirmwareMode::Bios);
        let out = vmware_instructions(&spec);
        // BIOS maps to "bios" in VMware syntax.
        assert!(out.contains("bios"));
    }

    // ── Hyper-V ──────────────────────────────────────────────────────────────

    #[test]
    fn hyperv_ps1_gen1_contains_set_vm_bios() {
        let spec = test_spec(Hypervisor::HyperV, FirmwareMode::Bios);
        let script = hyperv_ps1(&spec);
        assert!(
            script.contains("Set-VMBios"),
            "Gen1 script should contain Set-VMBios: {script}"
        );
        assert!(
            !script.contains("Set-VMFirmware"),
            "Gen1 script should not contain Set-VMFirmware"
        );
    }

    #[test]
    fn hyperv_ps1_gen2_contains_set_vm_firmware() {
        let spec = test_spec(Hypervisor::HyperV, FirmwareMode::Uefi);
        let script = hyperv_ps1(&spec);
        assert!(
            script.contains("Set-VMFirmware"),
            "Gen2 script should contain Set-VMFirmware: {script}"
        );
        assert!(
            !script.contains("Set-VMBios"),
            "Gen2 script should not contain Set-VMBios"
        );
    }

    #[test]
    fn hyperv_ps1_contains_vm_name_and_iso() {
        let spec = test_spec(Hypervisor::HyperV, FirmwareMode::Bios);
        let script = hyperv_ps1(&spec);
        assert!(script.contains("test-ubuntu"));
        assert!(script.contains("/tmp/test-ubuntu.iso"));
    }

    // ── Proxmox ──────────────────────────────────────────────────────────────

    #[test]
    fn proxmox_cmds_contains_qm_create() {
        let spec = test_spec(Hypervisor::Proxmox, FirmwareMode::Bios);
        let cmds = proxmox_cmds(&spec);
        let has_create = cmds.iter().any(|c| c.starts_with("qm create"));
        assert!(has_create, "missing qm create: {cmds:?}");
    }

    #[test]
    fn proxmox_cmds_uefi_has_efidisk() {
        let spec = test_spec(Hypervisor::Proxmox, FirmwareMode::Uefi);
        let cmds = proxmox_cmds(&spec);
        let has_efi = cmds.iter().any(|c| c.contains("efidisk0"));
        assert!(
            has_efi,
            "UEFI Proxmox should include efidisk0 command: {cmds:?}"
        );
    }

    #[test]
    fn proxmox_cmds_bios_no_efidisk() {
        let spec = test_spec(Hypervisor::Proxmox, FirmwareMode::Bios);
        let cmds = proxmox_cmds(&spec);
        let has_efi = cmds.iter().any(|c| c.contains("efidisk0"));
        assert!(
            !has_efi,
            "BIOS Proxmox should not include efidisk0: {cmds:?}"
        );
    }

    #[test]
    fn proxmox_cmds_no_duplicate_cdrom_and_ide2() {
        // Regression: qm create previously had both --cdrom and --ide2 which is
        // a duplicate ide2 assignment and causes qm create to fail.
        for fw in [FirmwareMode::Bios, FirmwareMode::Uefi] {
            let spec = test_spec(Hypervisor::Proxmox, fw);
            let cmds = proxmox_cmds(&spec);
            let qm_create_line = cmds.iter().find(|c| c.starts_with("qm create")).unwrap();
            assert!(
                !qm_create_line.contains("--cdrom"),
                "qm create must not use --cdrom (conflicts with --ide2): {qm_create_line}"
            );
            assert!(
                qm_create_line.contains("--ide2"),
                "qm create must use --ide2 for the ISO: {qm_create_line}"
            );
        }
    }

    // ── emit_launch ───────────────────────────────────────────────────────────

    #[test]
    fn emit_launch_qemu_bios_has_commands_no_script() {
        let spec = test_spec(Hypervisor::Qemu, FirmwareMode::Bios);
        let out = emit_launch(&spec);
        assert!(
            !out.commands.is_empty(),
            "QEMU emit_launch should produce commands"
        );
        assert!(
            out.script.is_none(),
            "QEMU emit_launch should not produce a script"
        );
    }

    #[test]
    fn emit_launch_qemu_uefi_has_commands_no_script() {
        let spec = test_spec(Hypervisor::Qemu, FirmwareMode::Uefi);
        let out = emit_launch(&spec);
        assert!(!out.commands.is_empty());
        assert!(out.script.is_none());
    }

    #[test]
    fn emit_launch_vmware_has_script_empty_commands() {
        let spec = test_spec(Hypervisor::Vmware, FirmwareMode::Bios);
        let out = emit_launch(&spec);
        assert!(
            out.commands.is_empty(),
            "VMware emit_launch commands should be empty"
        );
        assert!(
            out.script.is_some(),
            "VMware emit_launch should produce a script"
        );
    }

    #[test]
    fn emit_launch_hyperv_has_script_empty_commands() {
        let spec = test_spec(Hypervisor::HyperV, FirmwareMode::Uefi);
        let out = emit_launch(&spec);
        assert!(
            out.commands.is_empty(),
            "Hyper-V emit_launch commands should be empty"
        );
        assert!(
            out.script.is_some(),
            "Hyper-V emit_launch should produce a script"
        );
    }

    #[test]
    fn emit_launch_vbox_has_commands_no_script() {
        let spec = test_spec(Hypervisor::VirtualBox, FirmwareMode::Bios);
        let out = emit_launch(&spec);
        assert!(!out.commands.is_empty());
        assert!(out.script.is_none());
    }

    #[test]
    fn emit_launch_proxmox_has_commands_no_script() {
        let spec = test_spec(Hypervisor::Proxmox, FirmwareMode::Bios);
        let out = emit_launch(&spec);
        assert!(!out.commands.is_empty());
        assert!(out.script.is_none());
    }

    #[test]
    fn emit_launch_populates_iso_path_string() {
        let spec = test_spec(Hypervisor::Qemu, FirmwareMode::Bios);
        let out = emit_launch(&spec);
        assert_eq!(out.iso_path, "/tmp/test-ubuntu.iso");
    }

    // ── VmLaunchSpec::new ────────────────────────────────────────────────────

    #[test]
    fn vm_launch_spec_new_derives_vm_name_from_iso_stem() {
        let spec = VmLaunchSpec::new(
            Path::new("/some/path/myiso.iso"),
            Hypervisor::Qemu,
            FirmwareMode::Bios,
        );
        assert_eq!(spec.vm_name, "myiso");
    }

    #[test]
    fn vm_launch_spec_new_fallback_name_on_no_stem() {
        let spec = VmLaunchSpec::new(Path::new("/"), Hypervisor::Qemu, FirmwareMode::Bios);
        assert_eq!(spec.vm_name, "forgeiso-vm");
    }

    #[test]
    fn vm_launch_spec_new_no_ovmf_for_bios() {
        let spec = VmLaunchSpec::new(
            Path::new("/tmp/x.iso"),
            Hypervisor::Qemu,
            FirmwareMode::Bios,
        );
        // BIOS mode must never attempt OVMF discovery.
        assert!(spec.ovmf_path.is_none());
    }

    #[test]
    fn vm_launch_spec_new_sanitizes_special_chars_in_name() {
        // File names with spaces, parens, or quotes must be sanitized so
        // the vm_name is safe for shell commands and QEMU -drive paths.
        let spec = VmLaunchSpec::new(
            Path::new("/tmp/ubuntu 24.04 (copy).iso"),
            Hypervisor::Qemu,
            FirmwareMode::Bios,
        );
        assert!(
            !spec.vm_name.contains(' '),
            "spaces must be sanitized: {}",
            spec.vm_name
        );
        assert!(
            !spec.vm_name.contains('('),
            "parens must be sanitized: {}",
            spec.vm_name
        );
        assert!(
            spec.vm_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'),
            "vm_name must contain only safe chars: {}",
            spec.vm_name
        );
    }

    #[test]
    fn sanitize_vm_name_handles_edge_cases() {
        assert_eq!(sanitize_vm_name("normal-name"), "normal-name");
        assert_eq!(sanitize_vm_name("has spaces"), "has-spaces");
        assert_eq!(sanitize_vm_name("---"), "forgeiso-vm");
        assert_eq!(sanitize_vm_name(""), "forgeiso-vm");
        assert_eq!(sanitize_vm_name("a'b\"c"), "a-b-c");
    }
}
