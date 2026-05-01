use super::firmware::FirmwareMode;
use super::spec::VmLaunchSpec;

/// Generate a human-readable instruction block for VMware Workstation / Player.
///
/// `vmrun` may not be installed on all systems, so this returns documentation
/// alongside the programmatic option.
pub fn vmware_instructions(spec: &VmLaunchSpec) -> String {
    let iso = spec.iso_path.display();
    let fw = match spec.firmware {
        FirmwareMode::Bios => "bios",
        FirmwareMode::Uefi => "efi64",
    };
    let name = &spec.vm_name;
    format!(
        r#"# VMware Workstation / Player — ForgeISO boot test
# ─────────────────────────────────────────────────

# Option 1: vmrun (VMware Workstation Pro)
#   vmrun -T ws start /path/to/{name}.vmx

# Option 2: Manual setup
#   1. Create a new VM (Linux 64-bit)
#   2. Set firmware to: {fw}
#   3. Attach ISO: {iso}
#   4. Set RAM: {ram}MB, CPUs: {cpus}
#   5. Boot and observe serial output

# Option 3: OVF/OVA path
#   Use 'vmware-vdiskmanager' or the GUI to import if converting a disk image.

# VMX firmware setting:
#   firmware = "{fw}"

# If secure boot causes failures, add to .vmx:
#   uefi.allowAuthBypass = "TRUE"
"#,
        name = name,
        iso = iso,
        fw = fw,
        ram = spec.ram_mb,
        cpus = spec.cpus
    )
}
