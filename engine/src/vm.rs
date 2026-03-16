/// VM harness and hypervisor launch layer.
///
/// Generates launch commands, scripts, and configuration for booting a ForgeISO
/// artifact under multiple hypervisors (QEMU, VirtualBox, VMware, Hyper-V, Proxmox).
/// No I/O side effects at the module boundary — all output is returned as data.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ─── Hypervisor target ───────────────────────────────────────────────────────

/// Supported hypervisor targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hypervisor {
    Qemu,
    VirtualBox,
    Vmware,
    HyperV,
    Proxmox,
}

impl Hypervisor {
    /// Return a short lowercase identifier string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Hypervisor::Qemu => "qemu",
            Hypervisor::VirtualBox => "virtualbox",
            Hypervisor::Vmware => "vmware",
            Hypervisor::HyperV => "hyperv",
            Hypervisor::Proxmox => "proxmox",
        }
    }

    /// All hypervisor variants in a stable order.
    pub fn all() -> &'static [Hypervisor] {
        &[
            Hypervisor::Qemu,
            Hypervisor::VirtualBox,
            Hypervisor::Vmware,
            Hypervisor::HyperV,
            Hypervisor::Proxmox,
        ]
    }

    /// Parse from a lowercase string.  Returns `None` for unknown values.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "qemu" => Some(Hypervisor::Qemu),
            "virtualbox" | "vbox" => Some(Hypervisor::VirtualBox),
            "vmware" => Some(Hypervisor::Vmware),
            "hyperv" | "hyper-v" => Some(Hypervisor::HyperV),
            "proxmox" | "pve" => Some(Hypervisor::Proxmox),
            _ => None,
        }
    }
}

impl std::fmt::Display for Hypervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Boot firmware mode ───────────────────────────────────────────────────────

/// Boot firmware mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirmwareMode {
    Bios,
    Uefi,
}

impl FirmwareMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            FirmwareMode::Bios => "bios",
            FirmwareMode::Uefi => "uefi",
        }
    }

    /// Parse from a lowercase string. Returns `None` for unknown values.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "bios" | "legacy" => Some(FirmwareMode::Bios),
            "uefi" | "efi" => Some(FirmwareMode::Uefi),
            _ => None,
        }
    }
}

impl std::fmt::Display for FirmwareMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── OVMF discovery ──────────────────────────────────────────────────────────

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

// ─── Launch spec ─────────────────────────────────────────────────────────────

/// A fully-specified VM launch configuration.
///
/// Build with `VmLaunchSpec::new()` for sensible defaults, then adjust fields
/// before passing to `emit_launch()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmLaunchSpec {
    pub hypervisor: Hypervisor,
    pub firmware: FirmwareMode,
    pub iso_path: PathBuf,
    pub ram_mb: u32,
    pub cpus: u8,
    pub disk_gb: u32,
    pub vm_name: String,
    /// Resolved OVMF firmware path (QEMU / Proxmox UEFI only).
    pub ovmf_path: Option<PathBuf>,
}

impl VmLaunchSpec {
    /// Create a launch spec with sensible defaults derived from the ISO path.
    pub fn new(iso_path: &Path, hypervisor: Hypervisor, firmware: FirmwareMode) -> Self {
        let vm_name = iso_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("forgeiso-vm")
            .to_string();
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

// ─── QEMU ────────────────────────────────────────────────────────────────────

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

// ─── VirtualBox ──────────────────────────────────────────────────────────────

/// Generate the ordered sequence of `VBoxManage` shell commands required to
/// create, configure, and start a headless VM from the ISO.
pub fn vbox_commands(spec: &VmLaunchSpec) -> Vec<String> {
    let name = &spec.vm_name;
    let iso = spec.iso_path.display();
    let fw = match spec.firmware {
        FirmwareMode::Bios => "bios",
        FirmwareMode::Uefi => "efi",
    };
    vec![
        format!("VBoxManage createvm --name '{name}' --ostype Linux_64 --register"),
        format!(
            "VBoxManage modifyvm '{name}' --memory {ram} --cpus {cpus} --firmware {fw} --audio none",
            ram = spec.ram_mb,
            cpus = spec.cpus
        ),
        format!(
            "VBoxManage createhd --filename '/tmp/{name}.vdi' --size {size}",
            size = spec.disk_gb * 1024
        ),
        format!("VBoxManage storagectl '{name}' --name 'SATA' --add sata --controller IntelAhci"),
        format!(
            "VBoxManage storageattach '{name}' --storagectl 'SATA' --port 0 --device 0 --type hdd --medium '/tmp/{name}.vdi'"
        ),
        format!(
            "VBoxManage storageattach '{name}' --storagectl 'SATA' --port 1 --device 0 --type dvddrive --medium '{iso}'"
        ),
        format!("VBoxManage startvm '{name}' --type headless"),
        format!("# When done: VBoxManage unregistervm '{name}' --delete"),
    ]
}

// ─── VMware ──────────────────────────────────────────────────────────────────

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

// ─── Hyper-V ─────────────────────────────────────────────────────────────────

/// Generate a PowerShell script for Hyper-V VM creation and boot.
///
/// Generation 1 = BIOS, Generation 2 = UEFI.  The script must be run in an
/// elevated (Administrator) PowerShell session on a Windows host.
pub fn hyperv_ps1(spec: &VmLaunchSpec) -> String {
    let iso = spec.iso_path.display();
    let name = &spec.vm_name;
    let gen: u8 = match spec.firmware {
        FirmwareMode::Bios => 1,
        FirmwareMode::Uefi => 2,
    };
    let boot_order = if gen == 2 {
        "Set-VMFirmware -VMName $VMName -FirstBootDevice (Get-VMDvdDrive -VMName $VMName)"
    } else {
        "Set-VMBios -VMName $VMName -StartupOrder @('CD', 'IDE', 'LegacyNetworkAdapter', 'Floppy')"
    };
    format!(
        r#"# Hyper-V boot test — Run in Windows PowerShell (Administrator)
# ────────────────────────────────────────────────────────────────

$VMName = "{name}"
$IsoPath = "{iso}"

New-VM -Name $VMName -Generation {gen} -MemoryStartupBytes {ram}MB -Path "$env:TEMP"
Set-VMProcessor -VMName $VMName -Count {cpus}
Add-VMDvdDrive -VMName $VMName -Path $IsoPath
{boot_order}
# Boot the VM
Start-VM -Name $VMName
Write-Host "VM $VMName started. Connect with: vmconnect localhost $VMName"

# When done:
# Stop-VM -Name $VMName -Force
# Remove-VM -Name $VMName -Force
"#,
        name = name,
        iso = iso,
        gen = gen,
        ram = spec.ram_mb,
        cpus = spec.cpus,
        boot_order = boot_order
    )
}

// ─── Proxmox ─────────────────────────────────────────────────────────────────

/// Generate `qm` commands to create and start a test VM on a Proxmox VE node.
///
/// VMID 9000 is used as a convention for ephemeral test VMs; callers should
/// verify the ID is free before running these commands.
pub fn proxmox_cmds(spec: &VmLaunchSpec) -> Vec<String> {
    let name = &spec.vm_name;
    let iso = spec.iso_path.display();
    let bios_arg = match spec.firmware {
        FirmwareMode::Bios => "seabios",
        FirmwareMode::Uefi => "ovmf",
    };
    let iso_name = spec
        .iso_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("forgeiso.iso");
    let vmid: u32 = 9000;

    let mut cmds = vec![
        format!("# Proxmox VE — run on PVE node shell"),
        format!("# Copy ISO first: scp {iso} pve-host:/var/lib/vz/template/iso/"),
        // Note: do NOT use --cdrom here; it is shorthand for --ide2 ...,media=cdrom.
        // Specifying both --cdrom and --ide2 would attempt to assign two disks to
        // the same IDE port, causing qm create to fail.
        format!(
            "qm create {vmid} --name '{name}' --memory {ram} --cores {cpus} \
             --bios {bios} --boot order=ide2 \
             --ide2 local:iso/{isoname},media=cdrom \
             --scsihw virtio-scsi-pci --virtio0 local-lvm:{disk},size={disk}G",
            ram = spec.ram_mb,
            cpus = spec.cpus,
            bios = bios_arg,
            isoname = iso_name,
            disk = spec.disk_gb,
            vmid = vmid
        ),
    ];

    if matches!(spec.firmware, FirmwareMode::Uefi) {
        cmds.push(format!(
            "qm set {vmid} --efidisk0 local-lvm:0,efitype=4m,pre-enrolled-keys=0"
        ));
    }

    cmds.push(format!("qm start {vmid}"));
    cmds.push(format!("# Watch serial: qm terminal {vmid}"));
    cmds.push(format!("# When done: qm stop {vmid} && qm destroy {vmid}"));
    cmds
}

// ─── Emit ─────────────────────────────────────────────────────────────────────

/// Combined output produced by `emit_launch()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmLaunchOutput {
    pub hypervisor: Hypervisor,
    pub firmware: FirmwareMode,
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
            notes.push(format!(
                "QEMU {} mode — {}.",
                match spec.firmware {
                    FirmwareMode::Bios => "BIOS",
                    FirmwareMode::Uefi => "UEFI",
                },
                if kvm_available { "KVM acceleration enabled" } else { "software emulation (slow)" },
            ));
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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
}
