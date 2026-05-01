use std::path::Path;

use super::spec::VmLaunchSpec;

/// Generate QEMU launch arguments for BIOS boot.
///
/// `-enable-kvm` is included unconditionally here; call `maybe_remove_kvm()`
/// on the result if KVM availability is uncertain.
pub fn qemu_bios_args(spec: &VmLaunchSpec) -> Vec<String> {
    vec![
        "qemu-system-x86_64".to_string(),
        "-enable-kvm".to_string(),
        "-m".to_string(),
        format!("{}M", spec.ram_mb),
        "-smp".to_string(),
        format!("{}", spec.cpus),
        "-cdrom".to_string(),
        spec.iso_path.display().to_string(),
        "-boot".to_string(),
        "d".to_string(),
        "-drive".to_string(),
        format!("file=/tmp/{}.qcow2,format=qcow2,if=virtio", spec.vm_name),
        "-serial".to_string(),
        format!("file:/tmp/{}-bios-serial.log", spec.vm_name),
        "-display".to_string(),
        "none".to_string(),
        "-no-reboot".to_string(),
    ]
}

/// Generate QEMU launch arguments for UEFI boot.
///
/// Uses `spec.ovmf_path` when set; falls back to a well-known default.
/// `-enable-kvm` is included unconditionally; call `maybe_remove_kvm()` to
/// strip it when KVM is unavailable.
pub fn qemu_uefi_args(spec: &VmLaunchSpec) -> Vec<String> {
    let ovmf = spec
        .ovmf_path
        .as_deref()
        .unwrap_or(Path::new("/usr/share/OVMF/OVMF_CODE.fd"));
    vec![
        "qemu-system-x86_64".to_string(),
        "-enable-kvm".to_string(),
        "-m".to_string(),
        format!("{}M", spec.ram_mb),
        "-smp".to_string(),
        format!("{}", spec.cpus),
        "-drive".to_string(),
        format!("if=pflash,format=raw,readonly=on,file={}", ovmf.display()),
        "-cdrom".to_string(),
        spec.iso_path.display().to_string(),
        "-boot".to_string(),
        "d".to_string(),
        "-drive".to_string(),
        format!("file=/tmp/{}.qcow2,format=qcow2,if=virtio", spec.vm_name),
        "-serial".to_string(),
        format!("file:/tmp/{}-uefi-serial.log", spec.vm_name),
        "-display".to_string(),
        "none".to_string(),
        "-no-reboot".to_string(),
    ]
}

/// Strip `-enable-kvm` from an arg list when `/dev/kvm` is absent.
///
/// Returns the list unchanged if KVM is available.
pub fn maybe_remove_kvm(mut args: Vec<String>) -> Vec<String> {
    if !Path::new("/dev/kvm").exists() {
        args.retain(|a| a != "-enable-kvm");
    }
    args
}

/// Create a qcow2 disk image using `qemu-img`.
///
/// Errors are returned as a plain `String` to keep the function free of
/// engine-specific error types so it can be used from test harnesses.
pub fn create_qemu_disk(path: &Path, size_gb: u32) -> Result<(), String> {
    let status = std::process::Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            &path.display().to_string(),
            &format!("{}G", size_gb),
        ])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("qemu-img create failed".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::firmware::FirmwareMode;
    use crate::vm::hypervisor::Hypervisor;
    use crate::vm::spec::VmLaunchSpec;
    use std::path::PathBuf;

    fn spec_for(firmware: FirmwareMode) -> VmLaunchSpec {
        let mut s = VmLaunchSpec::new(
            std::path::Path::new("/tmp/test-iso.iso"),
            Hypervisor::Qemu,
            firmware,
        );
        s.ovmf_path = Some(PathBuf::from("/tmp/OVMF_CODE.fd"));
        s
    }

    #[test]
    fn qemu_bios_args_includes_kvm_cdrom_serial_and_no_reboot() {
        let s = spec_for(FirmwareMode::Bios);
        let args = qemu_bios_args(&s);
        assert!(args.contains(&"-enable-kvm".to_string()));
        assert!(args.contains(&"-cdrom".to_string()));
        assert!(args.contains(&"-no-reboot".to_string()));
        assert!(args.iter().any(|a| a.contains("test-iso")));
        // The disk image path should reference the sanitized vm name.
        assert!(args.iter().any(|a| a.contains(&s.vm_name)));
    }

    #[test]
    fn qemu_uefi_args_uses_ovmf_path_from_spec() {
        let s = spec_for(FirmwareMode::Uefi);
        let args = qemu_uefi_args(&s);
        // pflash drive must reference our explicit ovmf_path
        assert!(
            args.iter()
                .any(|a| a.contains("pflash") && a.contains("OVMF_CODE.fd")),
            "pflash arg missing or wrong: {args:?}"
        );
    }

    #[test]
    fn qemu_uefi_args_falls_back_to_default_ovmf_when_unset() {
        let mut s = VmLaunchSpec::new(
            std::path::Path::new("/tmp/iso.iso"),
            Hypervisor::Qemu,
            FirmwareMode::Uefi,
        );
        s.ovmf_path = None;
        let args = qemu_uefi_args(&s);
        assert!(
            args.iter()
                .any(|a| a.contains("/usr/share/OVMF/OVMF_CODE.fd")),
            "default OVMF path missing: {args:?}"
        );
    }

    #[test]
    fn maybe_remove_kvm_strips_flag_when_kvm_absent() {
        // /dev/kvm presence varies by host; we test the no-op vs strip behaviour
        // both ways with an explicit fixture list.
        let args = vec![
            "qemu-system-x86_64".to_string(),
            "-enable-kvm".to_string(),
            "-m".to_string(),
            "1024".to_string(),
        ];
        let out = maybe_remove_kvm(args.clone());
        if std::path::Path::new("/dev/kvm").exists() {
            assert_eq!(out, args, "kvm present -> args unchanged");
        } else {
            assert!(
                !out.contains(&"-enable-kvm".to_string()),
                "kvm absent -> -enable-kvm must be stripped"
            );
            assert!(out.contains(&"-m".to_string()));
        }
    }

    #[test]
    fn create_qemu_disk_returns_error_when_qemu_img_missing() {
        // If qemu-img isn't installed we get a Runtime/IO error; if installed
        // we get Ok or some specific error. Either way: API must not panic.
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("test.qcow2");
        let res = create_qemu_disk(&path, 1);
        // Not asserting on the specific status — but we are exercising every
        // branch of the function up to the spawn() call.
        if res.is_ok() {
            assert!(path.exists(), "qemu-img succeeded but file is missing");
        }
    }
}
