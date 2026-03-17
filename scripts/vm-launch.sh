#!/usr/bin/env bash
# ForgeISO VM launch helper
#
# Usage: scripts/vm-launch.sh <iso-path> [hypervisor] [firmware]
#
#   hypervisor:  qemu (default) | vbox | vmware | hyperv | proxmox
#   firmware:    bios (default) | uefi
#
# Generates or prints the commands needed to boot the ISO in the requested
# hypervisor.  For QEMU, the commands are run directly when possible.
# For other hypervisors, the commands or scripts are printed to stdout.
#
# Examples:
#   ./scripts/vm-launch.sh /tmp/custom.iso
#   ./scripts/vm-launch.sh /tmp/custom.iso qemu uefi
#   ./scripts/vm-launch.sh /tmp/custom.iso vbox bios
#   ./scripts/vm-launch.sh /tmp/custom.iso proxmox uefi

set -euo pipefail

# ─── Args ─────────────────────────────────────────────────────────────────────

ISO="${1:?Usage: vm-launch.sh <iso-path> [hypervisor] [firmware]}"
HYPERVISOR="${2:-qemu}"
FIRMWARE="${3:-bios}"

if [[ ! -f "${ISO}" ]]; then
    echo "ERROR: ISO file not found: ${ISO}" >&2
    exit 1
fi

# ─── Derived vars ─────────────────────────────────────────────────────────────

VM_NAME="$(basename "${ISO%.iso}")"
RAM_MB="2048"
CPUS="2"
DISK_GB="20"
DISK_PATH="/tmp/${VM_NAME}.qcow2"

# ─── OVMF discovery ───────────────────────────────────────────────────────────

find_ovmf() {
    local candidates=(
        "/usr/share/OVMF/OVMF_CODE.fd"
        "/usr/share/ovmf/OVMF.fd"
        "/usr/share/OVMF/x64/OVMF_CODE.fd"
        "/usr/share/edk2/x64/OVMF_CODE.fd"
        "/usr/share/edk2-ovmf/OVMF_CODE.fd"
    )
    for c in "${candidates[@]}"; do
        if [[ -f "${c}" ]]; then
            echo "${c}"
            return 0
        fi
    done
    return 1
}

OVMF_PATH=""
if [[ "${FIRMWARE}" == "uefi" ]]; then
    if OVMF_PATH="$(find_ovmf)"; then
        echo "INFO: Using OVMF firmware: ${OVMF_PATH}" >&2
    else
        OVMF_PATH="/usr/share/OVMF/OVMF_CODE.fd"
        echo "WARN: OVMF firmware not found; using default path (may fail): ${OVMF_PATH}" >&2
    fi
fi

# ─── KVM check ────────────────────────────────────────────────────────────────

KVM_ARGS="-enable-kvm"
if [[ ! -e "/dev/kvm" ]]; then
    echo "WARN: /dev/kvm not present; KVM disabled (software emulation will be slow)" >&2
    KVM_ARGS=""
fi

# ─── Hypervisor dispatch ──────────────────────────────────────────────────────

case "${HYPERVISOR}" in

# ── QEMU ──────────────────────────────────────────────────────────────────────
qemu)
    echo "INFO: Target: QEMU, firmware: ${FIRMWARE}, VM: ${VM_NAME}" >&2

    # Create disk image if it does not already exist.
    if [[ ! -f "${DISK_PATH}" ]]; then
        echo "INFO: Creating qcow2 disk image at ${DISK_PATH} (${DISK_GB}G)" >&2
        qemu-img create -f qcow2 "${DISK_PATH}" "${DISK_GB}G"
    fi

    if [[ "${FIRMWARE}" == "uefi" ]]; then
        echo "INFO: Launching QEMU UEFI boot" >&2
        # shellcheck disable=SC2086
        exec qemu-system-x86_64 \
            ${KVM_ARGS} \
            -m "${RAM_MB}M" \
            -smp "${CPUS}" \
            -drive "if=pflash,format=raw,readonly=on,file=${OVMF_PATH}" \
            -cdrom "${ISO}" \
            -boot d \
            -drive "file=${DISK_PATH},format=qcow2,if=virtio" \
            -serial "file:/tmp/${VM_NAME}-uefi-serial.log" \
            -display none \
            -no-reboot
    else
        echo "INFO: Launching QEMU BIOS boot" >&2
        # shellcheck disable=SC2086
        exec qemu-system-x86_64 \
            ${KVM_ARGS} \
            -m "${RAM_MB}M" \
            -smp "${CPUS}" \
            -cdrom "${ISO}" \
            -boot d \
            -drive "file=${DISK_PATH},format=qcow2,if=virtio" \
            -serial "file:/tmp/${VM_NAME}-bios-serial.log" \
            -display none \
            -no-reboot
    fi
    ;;

# ── VirtualBox ────────────────────────────────────────────────────────────────
vbox | virtualbox)
    FW_ARG="bios"
    [[ "${FIRMWARE}" == "uefi" ]] && FW_ARG="efi"

    echo "# VirtualBox commands for: ${VM_NAME}"
    echo "# Run these in order on the VirtualBox host."
    echo ""
    echo "VBoxManage createvm --name '${VM_NAME}' --ostype Linux_64 --register"
    echo "VBoxManage modifyvm '${VM_NAME}' --memory ${RAM_MB} --cpus ${CPUS} --firmware ${FW_ARG} --audio none"
    echo "VBoxManage createhd --filename '/tmp/${VM_NAME}.vdi' --size $(( DISK_GB * 1024 ))"
    echo "VBoxManage storagectl '${VM_NAME}' --name 'SATA' --add sata --controller IntelAhci"
    echo "VBoxManage storageattach '${VM_NAME}' --storagectl 'SATA' --port 0 --device 0 --type hdd --medium '/tmp/${VM_NAME}.vdi'"
    echo "VBoxManage storageattach '${VM_NAME}' --storagectl 'SATA' --port 1 --device 0 --type dvddrive --medium '${ISO}'"
    echo "VBoxManage startvm '${VM_NAME}' --type headless"
    echo ""
    echo "# When done:"
    echo "# VBoxManage unregistervm '${VM_NAME}' --delete"
    ;;

# ── VMware ────────────────────────────────────────────────────────────────────
vmware)
    FW_ARG="bios"
    [[ "${FIRMWARE}" == "uefi" ]] && FW_ARG="efi64"

    cat <<EOF
# VMware Workstation / Player — ForgeISO boot test
# ─────────────────────────────────────────────────

# Option 1: vmrun (VMware Workstation Pro)
#   vmrun -T ws start /path/to/${VM_NAME}.vmx

# Option 2: Manual setup
#   1. Create a new VM (Linux 64-bit)
#   2. Set firmware to: ${FW_ARG}
#   3. Attach ISO: ${ISO}
#   4. Set RAM: ${RAM_MB}MB, CPUs: ${CPUS}
#   5. Boot and observe serial output

# VMX firmware setting:
#   firmware = "${FW_ARG}"

# If secure boot causes failures, add to .vmx:
#   uefi.allowAuthBypass = "TRUE"
EOF
    ;;

# ── Hyper-V ───────────────────────────────────────────────────────────────────
hyperv | hyper-v)
    GEN=1
    BOOT_ORDER='Set-VMBios -VMName $VMName -StartupOrder @('"'"'CD'"'"', '"'"'IDE'"'"', '"'"'LegacyNetworkAdapter'"'"', '"'"'Floppy'"'"')'
    if [[ "${FIRMWARE}" == "uefi" ]]; then
        GEN=2
        BOOT_ORDER='Set-VMFirmware -VMName $VMName -FirstBootDevice (Get-VMDvdDrive -VMName $VMName)'
    fi

    cat <<EOF
# Hyper-V boot test — Run in Windows PowerShell (Administrator)
# ────────────────────────────────────────────────────────────────

\$VMName = "${VM_NAME}"
\$IsoPath = "${ISO}"

New-VM -Name \$VMName -Generation ${GEN} -MemoryStartupBytes ${RAM_MB}MB -Path "\$env:TEMP"
Set-VMProcessor -VMName \$VMName -Count ${CPUS}
Add-VMDvdDrive -VMName \$VMName -Path \$IsoPath
${BOOT_ORDER}
# Boot the VM
Start-VM -Name \$VMName
Write-Host "VM \$VMName started. Connect with: vmconnect localhost \$VMName"

# When done:
# Stop-VM -Name \$VMName -Force
# Remove-VM -Name \$VMName -Force
EOF
    ;;

# ── Proxmox ───────────────────────────────────────────────────────────────────
proxmox | pve)
    BIOS_ARG="seabios"
    [[ "${FIRMWARE}" == "uefi" ]] && BIOS_ARG="ovmf"
    VMID="9000"
    ISO_NAME="$(basename "${ISO}")"

    echo "# Proxmox VE — run on PVE node shell"
    echo "# Copy ISO first: scp ${ISO} pve-host:/var/lib/vz/template/iso/"
    echo ""
    echo "qm create ${VMID} --name '${VM_NAME}' --memory ${RAM_MB} --cores ${CPUS} \\"
    echo "   --bios ${BIOS_ARG} --cdrom local:iso/${ISO_NAME} --boot order=ide2 \\"
    echo "   --ide2 local:iso/${ISO_NAME},media=cdrom \\"
    echo "   --scsihw virtio-scsi-pci --virtio0 local-lvm:${DISK_GB},size=${DISK_GB}G"

    if [[ "${FIRMWARE}" == "uefi" ]]; then
        echo "qm set ${VMID} --efidisk0 local-lvm:0,efitype=4m,pre-enrolled-keys=0"
    fi

    echo "qm start ${VMID}"
    echo ""
    echo "# Watch serial: qm terminal ${VMID}"
    echo "# When done:    qm stop ${VMID} && qm destroy ${VMID}"
    ;;

*)
    echo "ERROR: Unknown hypervisor '${HYPERVISOR}'" >&2
    echo "Valid values: qemu, vbox, vmware, hyperv, proxmox" >&2
    exit 1
    ;;
esac
