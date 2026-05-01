use super::firmware::FirmwareMode;
use super::spec::VmLaunchSpec;

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
