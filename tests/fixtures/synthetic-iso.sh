#!/usr/bin/env bash
# tests/fixtures/synthetic-iso.sh — emit a small (<= 10 MB) synthetic source
# ISO containing just enough structure for the ForgeISO engine's
# scan + place + repack passes to succeed for a given distro family.
#
# Usage:
#   tests/fixtures/synthetic-iso.sh <family> <output.iso>
#
# Families:
#   ubuntu       — Ubuntu / Pop!_OS / Mint base (cloud-init nocloud + casper)
#   debian       — Debian / Kali (preseed + isolinux + boot/grub)
#   fedora       — Fedora / Rocky / Alma / CentOS (kickstart + boot/grub2)
#   arch         — Arch / EndeavourOS / Garuda / Manjaro (archiso layout)
#   opensuse     — openSUSE Leap / Tumbleweed (autoyast layout)
#   mint         — Linux Mint (Calamares preseed + casper)
#
# Output: a hybrid ISO9660 image with PVD, a fake initrd payload, an
# El Torito boot catalog, and the per-distro top-level directory layout
# expected by `engine/src/orchestrator/inject/place.rs` and
# `engine/src/orchestrator/build.rs`. Volume label encodes the family so
# the engine's `inspect_iso` heuristics can detect the distro.
#
# This is NOT a bootable installer ISO — it is a structural fixture only.
# Real boot/install testing requires upstream installer media.
set -Eeuo pipefail

usage() {
    cat <<'EOF' >&2
usage: synthetic-iso.sh <family> <output.iso>
families: ubuntu debian fedora arch opensuse mint
EOF
    exit 64
}

if [[ $# -ne 2 ]]; then
    usage
fi

FAMILY="$1"
OUT_ISO="$2"

if ! command -v xorriso >/dev/null 2>&1; then
    echo "fixture: missing xorriso" >&2
    exit 69
fi

WORK="$(mktemp -d -t forgeiso-fixture-XXXXXXXX)"
trap 'rm -rf -- "$WORK"' EXIT

TREE="$WORK/tree"
mkdir -p "$TREE"

# ---- common boot stubs (El Torito needs *some* boot image) -----------------
mkdir -p "$TREE/boot"
# Tiny synthetic isolinux/syslinux boot image (BIOS boot — 2 KB pad).
dd if=/dev/zero of="$TREE/boot/bootcat.bin" bs=2048 count=1 status=none
dd if=/dev/zero of="$TREE/boot/eltorito.img" bs=2048 count=2 status=none
# Tiny synthetic kernel + initrd payloads.
printf 'SYNTHETIC-KERNEL-PAYLOAD\n' > "$TREE/boot/vmlinuz"
dd if=/dev/zero of="$TREE/boot/initrd.img" bs=1024 count=64 status=none

# ---- per-family layout -----------------------------------------------------
populate_ubuntu() {
    mkdir -p "$TREE/.disk" "$TREE/boot/grub" "$TREE/casper" "$TREE/isolinux" \
             "$TREE/EFI/BOOT" "$TREE/preseed" "$TREE/install"
    cat >"$TREE/.disk/info" <<EOF
ForgeISO Synthetic Ubuntu 24.04 LTS "Noble Numbat" - Server amd64
EOF
    printf 'full_cd/single\n' >"$TREE/.disk/cd_type"
    printf 'live\n'           >"$TREE/.disk/casper-uuid-generic"
    cat >"$TREE/boot/grub/grub.cfg" <<'EOF'
set timeout=5
set default=0
menuentry "Ubuntu Server (synthetic)" {
    linux /casper/vmlinuz quiet ---
    initrd /casper/initrd
}
EOF
    cat >"$TREE/EFI/BOOT/grub.cfg" <<'EOF'
set timeout=5
menuentry "Ubuntu Server (UEFI synthetic)" {
    linuxefi /casper/vmlinuz quiet ---
    initrdefi /casper/initrd
}
EOF
    cat >"$TREE/isolinux/isolinux.cfg" <<'EOF'
default install
label install
    kernel /casper/vmlinuz
    append initrd=/casper/initrd quiet ---
EOF
    printf 'SYNTHETIC-CASPER-KERNEL\n' >"$TREE/casper/vmlinuz"
    dd if=/dev/zero of="$TREE/casper/initrd" bs=1024 count=64 status=none
    # No squashfs — engine only warns when rootfs path is missing, which is
    # acceptable for a fixture.
    VOL_ID="Ubuntu-Server 24.04 LTS amd64"
}

populate_mint() {
    populate_ubuntu
    # Mint reuses the Ubuntu casper layout but adds a Calamares marker so
    # `inspect_iso` can pick out a Mint volume label.
    mkdir -p "$TREE/.disk"
    cat >"$TREE/.disk/info" <<EOF
ForgeISO Synthetic Linux Mint 22.3 Cinnamon - Release amd64
EOF
    VOL_ID="Linux Mint 22.3 Cinnamon 64-bit"
}

populate_debian() {
    mkdir -p "$TREE/.disk" "$TREE/boot/grub" "$TREE/install.amd" "$TREE/isolinux" \
             "$TREE/EFI/BOOT" "$TREE/preseed" "$TREE/dists/stable/main"
    cat >"$TREE/.disk/info" <<EOF
ForgeISO Synthetic Debian GNU/Linux 13 "Trixie" - Official amd64 NETINST
EOF
    cat >"$TREE/boot/grub/grub.cfg" <<'EOF'
set timeout=5
menuentry "Install Debian (synthetic)" {
    linux /install.amd/vmlinuz quiet ---
    initrd /install.amd/initrd.gz
}
EOF
    cat >"$TREE/EFI/BOOT/grub.cfg" <<'EOF'
set timeout=5
menuentry "Install Debian (UEFI synthetic)" {
    linuxefi /install.amd/vmlinuz quiet ---
    initrdefi /install.amd/initrd.gz
}
EOF
    cat >"$TREE/isolinux/isolinux.cfg" <<'EOF'
default install
label install
    kernel /install.amd/vmlinuz
    append initrd=/install.amd/initrd.gz quiet ---
EOF
    printf 'SYNTHETIC-DEBIAN-KERNEL\n' >"$TREE/install.amd/vmlinuz"
    dd if=/dev/zero of="$TREE/install.amd/initrd.gz" bs=1024 count=64 status=none
    VOL_ID="Debian 13 amd64 n"
}

populate_fedora() {
    mkdir -p "$TREE/.discinfo" "$TREE/boot/grub2" "$TREE/EFI/BOOT" \
             "$TREE/isolinux" "$TREE/images/pxeboot" "$TREE/LiveOS"
    # .discinfo is a file in real Fedora media; keep it as a file but allow
    # callers to detect it.
    rm -rf "$TREE/.discinfo"
    cat >"$TREE/.discinfo" <<EOF
1734567890.123456
42
x86_64
ALL
EOF
    cat >"$TREE/boot/grub2/grub.cfg" <<'EOF'
set timeout=5
menuentry "Install Fedora 42 Server (synthetic)" {
    linux /images/pxeboot/vmlinuz inst.stage2=hd:LABEL=Fedora-S-dvd-x86_64-42 quiet
    initrd /images/pxeboot/initrd.img
}
EOF
    cat >"$TREE/EFI/BOOT/grub.cfg" <<'EOF'
set timeout=5
menuentry "Install Fedora 42 Server (UEFI synthetic)" {
    linuxefi /images/pxeboot/vmlinuz inst.stage2=hd:LABEL=Fedora-S-dvd-x86_64-42 quiet
    initrdefi /images/pxeboot/initrd.img
}
EOF
    cat >"$TREE/isolinux/isolinux.cfg" <<'EOF'
default install
label install
    kernel /images/pxeboot/vmlinuz
    append initrd=/images/pxeboot/initrd.img inst.stage2=hd:LABEL=Fedora-S-dvd-x86_64-42 quiet
EOF
    printf 'SYNTHETIC-FEDORA-KERNEL\n' >"$TREE/images/pxeboot/vmlinuz"
    dd if=/dev/zero of="$TREE/images/pxeboot/initrd.img" bs=1024 count=64 status=none
    VOL_ID="Fedora-S-dvd-x86_64-42"
}

populate_arch() {
    mkdir -p "$TREE/arch/boot/x86_64" "$TREE/arch/pkglist.x86_64" \
             "$TREE/loader/entries" "$TREE/syslinux" "$TREE/EFI/BOOT"
    rm -rf "$TREE/arch/pkglist.x86_64"
    : >"$TREE/arch/pkglist.x86_64.txt"
    printf 'SYNTHETIC-ARCH-KERNEL\n' >"$TREE/arch/boot/x86_64/vmlinuz-linux"
    dd if=/dev/zero of="$TREE/arch/boot/x86_64/initramfs-linux.img" bs=1024 count=64 status=none
    cat >"$TREE/syslinux/archiso_sys.conf" <<'EOF'
LABEL arch64
    MENU LABEL Boot Arch Linux (synthetic, x86_64)
    LINUX /arch/boot/x86_64/vmlinuz-linux
    INITRD /arch/boot/x86_64/initramfs-linux.img
    APPEND archisobasedir=arch quiet
LABEL arch64-nonfree
    MENU LABEL Boot Arch Linux (synthetic, nonfree)
    LINUX /arch/boot/x86_64/vmlinuz-linux
    INITRD /arch/boot/x86_64/initramfs-linux.img
    APPEND archisobasedir=arch quiet
EOF
    cat >"$TREE/loader/entries/archiso-x86_64-linux.conf" <<'EOF'
title    Arch Linux install medium (synthetic)
linux    /arch/boot/x86_64/vmlinuz-linux
initrd   /arch/boot/x86_64/initramfs-linux.img
options  archisobasedir=arch quiet
EOF
    cat >"$TREE/EFI/BOOT/grub.cfg" <<'EOF'
set timeout=5
menuentry "Arch Linux install (UEFI synthetic)" {
    linuxefi /arch/boot/x86_64/vmlinuz-linux archisobasedir=arch quiet
    initrdefi /arch/boot/x86_64/initramfs-linux.img
}
EOF
    VOL_ID="ARCH_SYNTH"
}

populate_opensuse() {
    mkdir -p "$TREE/boot/x86_64/loader" "$TREE/EFI/BOOT" "$TREE/suse/setup/descr" "$TREE/media.1"
    : >"$TREE/media.1/products"
    cat >"$TREE/boot/x86_64/loader/grub.cfg" <<'EOF'
set timeout=5
menuentry "Install openSUSE Leap (synthetic)" {
    linux /boot/x86_64/loader/linux quiet
    initrd /boot/x86_64/loader/initrd
}
EOF
    cat >"$TREE/EFI/BOOT/grub.cfg" <<'EOF'
set timeout=5
menuentry "Install openSUSE Leap (UEFI synthetic)" {
    linuxefi /boot/x86_64/loader/linux quiet
    initrdefi /boot/x86_64/loader/initrd
}
EOF
    printf 'SYNTHETIC-OPENSUSE-KERNEL\n' >"$TREE/boot/x86_64/loader/linux"
    dd if=/dev/zero of="$TREE/boot/x86_64/loader/initrd" bs=1024 count=64 status=none
    VOL_ID="openSUSE-Leap-15.6-DVD-x86_64"
}

case "$FAMILY" in
    ubuntu)   populate_ubuntu ;;
    debian)   populate_debian ;;
    fedora)   populate_fedora ;;
    arch)     populate_arch ;;
    opensuse) populate_opensuse ;;
    mint)     populate_mint ;;
    *)        usage ;;
esac

# ---- pack into ISO9660 + El Torito ----------------------------------------
# Use xorriso's mkisofs front-end so the output ISO carries an El Torito boot
# catalog that the engine's `xorriso -report_el_torito as_mkisofs` pass can
# parse during repack. Without -b the engine repack would still succeed (it
# falls back to no-boot args) but having a boot catalog exercises the more
# common code path used by real upstream installer media.
mkdir -p "$(dirname -- "$OUT_ISO")"
xorriso -as mkisofs \
    -V "$VOL_ID" \
    -J -joliet-long -r \
    -b boot/eltorito.img \
    -c boot/bootcat.bin \
    -no-emul-boot \
    -boot-load-size 4 \
    -boot-info-table \
    -o "$OUT_ISO" \
    "$TREE" >/dev/null 2>&1

# Sanity check: ISO must exist and be under 10 MB.
size=$(stat -c%s -- "$OUT_ISO")
if (( size > 10 * 1024 * 1024 )); then
    echo "fixture: $OUT_ISO is $size bytes (>10MB cap)" >&2
    exit 70
fi

printf 'family=%s\nout=%s\nsize=%s\nvolid=%s\n' \
    "$FAMILY" "$OUT_ISO" "$size" "$VOL_ID"
