# Troubleshooting

## Build fails on a non-Linux host

ForgeISO only supports local build and VM test flows on Linux.

## ISO inspection is missing distro details

Install `xorriso` so ForgeISO can read files from inside the ISO instead of relying only on the primary volume label.

## Repack fails after rootfs extraction

Install both `unsquashfs` and `mksquashfs`. ForgeISO uses them for Ubuntu, Mint, and Arch root filesystem updates.

## Fedora overlay does not reach the live rootfs

Some Fedora live images use nested filesystem layouts that are not yet rewritten by the local remaster step. ForgeISO will still update top-level ISO content and report the limitation honestly.

## UEFI smoke test fails immediately

Install QEMU and an OVMF firmware package so ForgeISO can boot the ISO locally in UEFI mode.

## Mint inject — installer still shows GUI prompts

The Mint preseed path requires `auto=true priority=critical preseed/file=/cdrom/preseed.cfg` on the kernel command line. ForgeISO injects these parameters into the grub.cfg and isolinux.cfg during inject. If the ISO you're using has a non-standard boot config (e.g., patched third-party Mint remaster), the kernel params may not be applied.

Workaround: manually add `auto=true priority=critical preseed/file=/cdrom/preseed.cfg` to the boot menu entry before booting.

Note: Calamares preseed support varies by Mint release. This path is not CI-tested with a real Mint ISO.

## Arch inject — archinstall does not run at boot

The Arch inject path places the archinstall config at `/arch/boot/archinstall-config.json` in the ISO and adds `archiso_script=/arch/boot/run-archinstall.sh` to syslinux and systemd-boot entries. If the boot entries do not trigger the script:

1. Verify the `archiso_script=` parameter appears in the syslinux APPEND line or systemd-boot options line.
2. The script is placed in `arch/boot/` and is accessible from the running live system at `/run/archiso/bootmnt/arch/boot/run-archinstall.sh`.
3. You may need to run `archinstall --config /run/archiso/bootmnt/arch/boot/archinstall-config.json:` manually if the automatic trigger fails.

Note: archiso_script= support requires archiso 2023+ on the Arch live media side.

## Kickstart inject — Fedora installer cannot find ks.cfg

The ks.cfg is placed at the ISO root and the boot entry is patched with `inst.ks=cdrom:/ks.cfg`. If the installer still prompts for a Kickstart URL:

1. Ensure you are booting from the ForgeISO-output ISO, not the original.
2. Check that `inst.ks=cdrom:/ks.cfg` appears in the kernel boot parameters (visible during boot or in grub menu).
3. Some Fedora ISOs use a different boot path — check if `EFI/BOOT/grub.cfg` also needs patching (ForgeISO patches this automatically when it exists).
