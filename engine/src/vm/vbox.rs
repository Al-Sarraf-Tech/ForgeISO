use super::firmware::FirmwareMode;
use super::spec::VmLaunchSpec;

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
