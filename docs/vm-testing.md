# VM Testing Guide

This document covers how to boot a ForgeISO artifact in each supported
hypervisor for functional validation, both BIOS and UEFI paths.

---

## Supported hypervisors

| Hypervisor | Host OS | BIOS | UEFI | Method |
|---|---|---|---|---|
| QEMU | Linux, Windows (WSL) | yes | yes | Commands (run directly) |
| VirtualBox | Linux, Windows | yes | yes | `VBoxManage` commands |
| VMware Workstation/Player | Linux, Windows | yes | yes | Instructions / vmrun |
| Hyper-V | Windows 10/11, Windows Server | yes (Gen1) | yes (Gen2) | PowerShell script |
| Proxmox VE | Proxmox node shell | yes | yes | `qm` commands |

---

## Quick start

Use `scripts/vm-launch.sh` to generate or run the appropriate commands:

```bash
# QEMU BIOS (default) — runs the VM directly
./scripts/vm-launch.sh /path/to/output.iso

# QEMU UEFI
./scripts/vm-launch.sh /path/to/output.iso qemu uefi

# VirtualBox BIOS — prints VBoxManage commands to stdout
./scripts/vm-launch.sh /path/to/output.iso vbox bios

# VMware instructions
./scripts/vm-launch.sh /path/to/output.iso vmware uefi

# Hyper-V PowerShell script
./scripts/vm-launch.sh /path/to/output.iso hyperv uefi

# Proxmox qm commands
./scripts/vm-launch.sh /path/to/output.iso proxmox bios
```

---

## BIOS vs UEFI testing

- **BIOS / Legacy** boots use the MBR boot record. Equivalent to VirtualBox
  Generation 1 and Hyper-V Generation 1. QEMU uses plain `-boot d`.
- **UEFI** boots require OVMF firmware (Linux) or native EFI (VirtualBox,
  VMware, Hyper-V Gen2). Ubuntu autoinstall ISOs built with ForgeISO include
  GRUB EFI stubs and are compatible with both modes.

Always test both paths for production artifacts.

---

## QEMU

### Prerequisites

- `qemu-system-x86_64` — from `qemu-system-x86` (Fedora/RHEL: `qemu-kvm`)
- `qemu-img` — from the same package
- For UEFI: `edk2-ovmf` (Fedora/RHEL) or `ovmf` (Debian/Ubuntu)

```bash
# Fedora
sudo dnf install -y qemu-kvm edk2-ovmf

# Ubuntu/Debian
sudo apt-get install -y qemu-system-x86 ovmf
```

### KVM presence check

KVM hardware acceleration requires `/dev/kvm` to be available. Without it,
QEMU falls back to software emulation, which is 10-50x slower.

```bash
ls -l /dev/kvm          # must exist
groups                  # your user must be in the 'kvm' group
# Fedora: sudo usermod -aG kvm $USER && newgrp kvm
```

`scripts/vm-launch.sh` and the `forgeiso-engine` VM module automatically
detect `/dev/kvm` and drop `-enable-kvm` when it is absent, with a warning.

### OVMF path

`find_ovmf()` in `engine/src/vm.rs` checks these paths in order:

```
/usr/share/OVMF/OVMF_CODE.fd          # Fedora/RHEL default
/usr/share/ovmf/OVMF.fd               # Ubuntu alternative
/usr/share/OVMF/x64/OVMF_CODE.fd
/usr/share/edk2/x64/OVMF_CODE.fd
/usr/share/edk2-ovmf/OVMF_CODE.fd
```

If none exist, the engine warns and falls back to the Fedora default path.
Install the appropriate package and retry.

### BIOS boot (QEMU)

```bash
qemu-img create -f qcow2 /tmp/test.qcow2 20G
qemu-system-x86_64 \
  -enable-kvm \
  -m 2048M -smp 2 \
  -cdrom /path/to/output.iso \
  -boot d \
  -drive file=/tmp/test.qcow2,format=qcow2,if=virtio \
  -serial file:/tmp/bios-serial.log \
  -display none \
  -no-reboot
```

### UEFI boot (QEMU)

```bash
qemu-img create -f qcow2 /tmp/test.qcow2 20G
qemu-system-x86_64 \
  -enable-kvm \
  -m 2048M -smp 2 \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE.fd \
  -cdrom /path/to/output.iso \
  -boot d \
  -drive file=/tmp/test.qcow2,format=qcow2,if=virtio \
  -serial file:/tmp/uefi-serial.log \
  -display none \
  -no-reboot
```

### Viewing serial output

```bash
tail -f /tmp/bios-serial.log
tail -f /tmp/uefi-serial.log
```

---

## VirtualBox

### Prerequisites

- VirtualBox 6.1 or newer
- `VBoxManage` must be in `PATH`
- For UEFI, VirtualBox uses its built-in EFI implementation — no extra package

```bash
# Fedora (RPM from Oracle repository)
sudo dnf install VirtualBox-7.0
# or use the official installer from virtualbox.org
```

### Usage

`scripts/vm-launch.sh <iso> vbox [bios|uefi]` prints the required
`VBoxManage` commands. Run them on the host where VirtualBox is installed.

```bash
./scripts/vm-launch.sh output.iso vbox bios | bash
```

Or copy-paste individual commands for review before execution.

### Cleanup

```bash
VBoxManage unregistervm '<vm-name>' --delete
```

---

## VMware Workstation / Player

### Prerequisites

- VMware Workstation 17+ or VMware Player 17+
- **vmrun** is only available with Workstation Pro; Player users must use the GUI

### BIOS boot

1. Create a new VM: Linux > Other Linux 5.x or later (64-bit)
2. Set firmware to: **BIOS**
3. Attach ISO as CD/DVD (Connected at power on)
4. Set RAM to 2048 MB, CPUs to 2

### UEFI boot

1. Create a new VM as above
2. Set firmware to: **UEFI** (in VM Settings > Options > Advanced)
3. Add to the `.vmx` file:
   ```
   firmware = "efi64"
   ```
4. Attach ISO; boot the VM

### Secure boot

If secure boot causes boot failures, add to the `.vmx` file:

```
uefi.allowAuthBypass = "TRUE"
```

### vmrun (Workstation Pro only)

```bash
vmrun -T ws start /path/to/vm.vmx
```

### Instructions script

```bash
./scripts/vm-launch.sh output.iso vmware bios
./scripts/vm-launch.sh output.iso vmware uefi
```

---

## Hyper-V

### Prerequisites

- Windows 10/11 Pro, Enterprise, or Server with Hyper-V role enabled
- PowerShell 5.1 or 7+ running as Administrator
- The ISO must be accessible from the Windows host (use a UNC path or copy it)

### Enable Hyper-V

```powershell
Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All
# Reboot required
```

### Generation 1 (BIOS)

Generation 1 VMs use legacy BIOS. Compatible with all Ubuntu ISOs.

```bash
./scripts/vm-launch.sh output.iso hyperv bios
```

Copy the printed PowerShell script and run it in an elevated session.

### Generation 2 (UEFI)

Generation 2 VMs use UEFI with Secure Boot support. Ubuntu ISOs signed for
Secure Boot work natively; unsigned ISOs may require disabling Secure Boot:

```powershell
Set-VMFirmware -VMName '<name>' -EnableSecureBoot Off
```

```bash
./scripts/vm-launch.sh output.iso hyperv uefi
```

### Connecting to the VM

```powershell
vmconnect localhost '<vm-name>'
```

### Cleanup

```powershell
Stop-VM -Name '<vm-name>' -Force
Remove-VM -Name '<vm-name>' -Force
```

---

## Proxmox VE

### Prerequisites

- Access to a Proxmox VE node shell (SSH or console)
- ISO transferred to `/var/lib/vz/template/iso/` on the node
- VMID 9000 must be free (used by convention for test VMs)

### Transfer the ISO

```bash
scp /path/to/output.iso root@pve-host:/var/lib/vz/template/iso/
```

### Usage

```bash
./scripts/vm-launch.sh output.iso proxmox bios
./scripts/vm-launch.sh output.iso proxmox uefi
```

Copy-paste the printed `qm` commands on the Proxmox node shell.

### Check VMID availability

```bash
qm list | grep 9000
```

If 9000 is already in use, edit the `VMID` variable in the script or pass the
commands a free ID manually.

### UEFI (OVMF on Proxmox)

The engine uses `--bios ovmf` and `--efidisk0` for UEFI VMs. Proxmox's OVMF
integration is handled through the `local-lvm` storage; the EFI disk is
created automatically.

### Watch the serial console

```bash
qm terminal 9000
# Press Ctrl+O to detach
```

### Cleanup

```bash
qm stop 9000
qm destroy 9000
```

---

## Secure boot notes

- **QEMU**: Secure Boot requires OVMF with enrolled keys (`OVMF_CODE.secboot.fd`
  on some distros). ForgeISO smoke tests do not require secure boot.
- **VirtualBox**: EFI firmware supports Secure Boot on VirtualBox 7.0+;
  disabled by default.
- **VMware**: Set `uefi.allowAuthBypass = "TRUE"` in `.vmx` to bypass Secure
  Boot for test builds.
- **Hyper-V Gen2**: Secure Boot is **enabled by default**. Disable it for
  unsigned ISOs:
  ```powershell
  Set-VMFirmware -VMName '<name>' -EnableSecureBoot Off
  ```
- **Proxmox**: Set `pre-enrolled-keys=0` on the `--efidisk0` argument (done
  automatically by `scripts/vm-launch.sh` and the engine).

---

## Troubleshooting common boot failures

| Symptom | Likely cause | Fix |
|---|---|---|
| Black screen / no output | Wrong firmware mode | Switch BIOS/UEFI; check serial log |
| `BOOTMGR is missing` | Boot order wrong | Set CD/DVD as first boot device |
| `No bootable device` | ISO not attached or corrupt | Verify ISO integrity with `sha256sum` |
| Hangs at GRUB menu | GRUB config missing `linux`/`initrd` | Re-inject autoinstall with ForgeISO |
| Cloud-init not running | No `autoinstall` seed on ISO | Verify `autoinstall.yaml` was injected |
| Secure Boot failure | Unsigned shim | Disable Secure Boot (see notes above) |
| KVM error: permission denied | User not in `kvm` group | `sudo usermod -aG kvm $USER && newgrp kvm` |
| OVMF not found | Package not installed | Install `edk2-ovmf` or `ovmf` |
| Extremely slow boot | No KVM (/dev/kvm absent) | Enable nested virt or run on bare metal |
| `qemu-img: command not found` | qemu-img not installed | Install `qemu-img` / `qemu-kvm` package |

### Reading the serial log

The most reliable way to diagnose boot failures is the serial console log:

```bash
# BIOS
tail -200 /tmp/<vm-name>-bios-serial.log

# UEFI
tail -200 /tmp/<vm-name>-uefi-serial.log
```

GRUB, the kernel, and cloud-init all write to the serial console when the ISO
is built with `console=ttyS0` in the kernel command line.

### Checking the ISO structure

```bash
# List root of ISO
isoinfo -i /path/to/output.iso -l | head -40

# Verify SHA-256
sha256sum /path/to/output.iso
```

---

## Engine API reference

The `forgeiso-engine` crate exposes the VM harness through `engine/src/vm.rs`:

```rust
use forgeiso_engine::{
    emit_launch, find_ovmf, VmLaunchSpec, Hypervisor, FirmwareMode,
};

let spec = VmLaunchSpec::new(
    Path::new("/path/to/output.iso"),
    Hypervisor::Qemu,
    FirmwareMode::Uefi,
);
let out = emit_launch(&spec);
// out.commands — shell arguments
// out.script   — script text (VMware, Hyper-V)
// out.notes    — warnings and tips
// out.kvm_available — whether /dev/kvm is present
```
