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
