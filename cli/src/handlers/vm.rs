use forgeiso_engine::{emit_launch, find_ovmf, FirmwareMode, Hypervisor, VmLaunchSpec};

use crate::VmCmd;

pub async fn handle(command: VmCmd) -> anyhow::Result<()> {
    match command {
        VmCmd::Emit {
            iso,
            hypervisor,
            firmware,
            ram,
            cpus,
            disk,
            name,
            json,
        } => {
            let hv = parse_hypervisor(&hypervisor)?;
            let fw = parse_firmware(&firmware)?;
            let ovmf = if matches!(fw, FirmwareMode::Uefi) {
                find_ovmf()
            } else {
                None
            };
            let vm_name = name.unwrap_or_else(|| {
                iso.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "forgeiso-vm".to_string())
            });
            let spec = VmLaunchSpec {
                hypervisor: hv,
                firmware: fw,
                iso_path: iso,
                ram_mb: ram,
                cpus,
                disk_gb: disk,
                vm_name,
                ovmf_path: ovmf,
            };
            let out = emit_launch(&spec);
            if json {
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                print_vm_output(&out);
            }
        }
    }
    Ok(())
}

fn parse_hypervisor(raw: &str) -> anyhow::Result<Hypervisor> {
    match raw.to_lowercase().as_str() {
        "qemu" => Ok(Hypervisor::Qemu),
        "virtualbox" | "vbox" => Ok(Hypervisor::VirtualBox),
        "vmware" => Ok(Hypervisor::Vmware),
        "hyperv" | "hyper-v" => Ok(Hypervisor::HyperV),
        "proxmox" => Ok(Hypervisor::Proxmox),
        other => anyhow::bail!(
            "unknown hypervisor '{other}': expected qemu, virtualbox, vmware, hyperv, proxmox"
        ),
    }
}

fn parse_firmware(raw: &str) -> anyhow::Result<FirmwareMode> {
    match raw.to_lowercase().as_str() {
        "bios" => Ok(FirmwareMode::Bios),
        "uefi" | "efi" => Ok(FirmwareMode::Uefi),
        other => anyhow::bail!("unknown firmware '{other}': expected bios or uefi"),
    }
}

fn print_vm_output(out: &forgeiso_engine::VmLaunchOutput) {
    println!("Hypervisor: {:?}", out.hypervisor);
    println!("Firmware:   {:?}", out.firmware);
    println!("ISO:        {}", out.iso_path);
    println!(
        "KVM:        {}",
        if out.kvm_available {
            "available"
        } else {
            "not available (software emulation)"
        }
    );
    if let Some(ref ovmf) = out.ovmf_used {
        println!("OVMF:       {ovmf}");
    }
    if !out.notes.is_empty() {
        println!();
        for note in &out.notes {
            eprintln!("NOTE: {note}");
        }
    }
    if !out.commands.is_empty() {
        println!();
        // QEMU args are a single argv list; join as one shell command.
        // VirtualBox/Proxmox store complete commands, one per entry.
        if matches!(out.hypervisor, Hypervisor::Qemu) {
            println!("# Run:");
            println!("{}", out.commands.join(" \\\n  "));
        } else {
            println!("# Run these commands:");
            for cmd in &out.commands {
                println!("{cmd}");
            }
        }
    }
    if let Some(ref script) = out.script {
        println!();
        println!("# Script:");
        println!("{script}");
    }
}
