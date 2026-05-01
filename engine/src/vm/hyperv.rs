use super::firmware::FirmwareMode;
use super::spec::VmLaunchSpec;

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
