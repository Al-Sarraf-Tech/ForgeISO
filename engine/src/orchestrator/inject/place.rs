//! Phase B of `inject_autoinstall`: copy distro-specific files into the
//! extracted ISO tree and patch boot configurations so the installer picks
//! up the autoinstall config at boot.

use std::path::Path;

use crate::config::InjectConfig;
use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};

use crate::orchestrator::helpers::{patch_boot_configs, patch_efi_grub_cfg};
use crate::orchestrator::ForgeIsoEngine;

/// Ubuntu — copy `nocloud/` overlay to ISO root and patch grub/isolinux/EFI
/// kernel command lines with `autoinstall ds=nocloud;s=/cdrom/nocloud/`.
pub(super) fn ubuntu(
    engine: &ForgeIsoEngine,
    cfg: &InjectConfig,
    work_dir: &Path,
    extract_dir: &Path,
) -> EngineResult<()> {
    // Cloud-init nocloud overlay.
    // Files must be at the ISO root so that when the installer
    // mounts the CD at /cdrom/ the datasource path resolves to
    // /cdrom/nocloud/ — not /cdrom/cdrom/nocloud/.
    let nocloud_dir = work_dir.join("overlay").join("nocloud");
    let iso_nocloud = extract_dir.join("nocloud");
    std::fs::create_dir_all(&iso_nocloud)?;
    for entry in std::fs::read_dir(&nocloud_dir)? {
        let entry = entry?;
        std::fs::copy(entry.path(), iso_nocloud.join(entry.file_name()))?;
    }
    engine.emit(EngineEvent::info(
        EventPhase::Inject,
        "injected cloud-init files into ISO root /nocloud/",
    ));

    // Wallpaper — also at ISO root so /cdrom/wallpaper/ resolves correctly.
    if let Some(src) = &cfg.wallpaper {
        let fname = src.file_name().ok_or_else(|| {
            EngineError::InvalidConfig(format!("wallpaper path has no filename: {}", src.display()))
        })?;
        let iso_wp = extract_dir.join("wallpaper");
        std::fs::create_dir_all(&iso_wp)?;
        std::fs::copy(work_dir.join("wallpaper").join(fname), iso_wp.join(fname))?;
    }

    // Boot patch — Ubuntu autoinstall kernel params
    let kernel_append = " autoinstall ds=nocloud;s=/cdrom/nocloud/";
    patch_boot_configs(extract_dir, kernel_append)?;

    // Also patch EFI/BOOT/grub.cfg for UEFI boot — without this,
    // modern UEFI systems see the unmodified EFI grub config and
    // boot straight into the manual interactive installer.
    patch_efi_grub_if_present(extract_dir, kernel_append)?;
    Ok(())
}

/// Mint — copy preseed.cfg to ISO root and patch boot entries with the
/// Calamares preseed kernel command line.
pub(super) fn mint(
    engine: &ForgeIsoEngine,
    work_dir: &Path,
    extract_dir: &Path,
) -> EngineResult<()> {
    // Copy preseed.cfg to ISO root (accessible as /cdrom/preseed.cfg at boot).
    std::fs::copy(
        work_dir.join("preseed.cfg"),
        extract_dir.join("preseed.cfg"),
    )?;
    engine.emit(EngineEvent::info(
        EventPhase::Inject,
        "injected preseed.cfg into ISO root",
    ));

    // Patch boot entries to trigger Calamares preseed.
    // Calamares reads preseed when booted with:
    //   auto=true priority=critical preseed/file=/cdrom/preseed.cfg
    let kernel_append = " auto=true priority=critical preseed/file=/cdrom/preseed.cfg";
    patch_boot_configs(extract_dir, kernel_append)?;

    // Also patch EFI/BOOT/grub.cfg if present (UEFI Mint media).
    // Use line-by-line patching so only kernel command lines
    // (`linuxefi` / `linux`) are modified — a global string replace
    // on "quiet splash" would also corrupt comments or menu labels
    // that happen to contain those words.
    patch_efi_grub_if_present(extract_dir, kernel_append)?;
    Ok(())
}

/// Fedora — copy ks.cfg to ISO root and patch boot entries with
/// `inst.ks=cdrom:/ks.cfg`.
pub(super) fn fedora(
    engine: &ForgeIsoEngine,
    work_dir: &Path,
    extract_dir: &Path,
) -> EngineResult<()> {
    // Copy ks.cfg to ISO root
    std::fs::copy(work_dir.join("ks.cfg"), extract_dir.join("ks.cfg"))?;
    engine.emit(EngineEvent::info(
        EventPhase::Inject,
        "injected ks.cfg into ISO root",
    ));

    // Patch Fedora boot entries to add inst.ks=cdrom:/ks.cfg
    let kernel_append = " inst.ks=cdrom:/ks.cfg";
    patch_boot_configs(extract_dir, kernel_append)?;

    // Also patch EFI/BOOT/grub.cfg if present (UEFI Fedora media).
    // Use line-by-line patching — `.replace("quiet", ...)` would corrupt
    // any comment or menu label containing the word "quiet", and would
    // silently miss the injection if the Fedora ISO does not include the
    // "quiet" parameter.
    patch_efi_grub_if_present(extract_dir, kernel_append)?;
    Ok(())
}

/// Arch — copy archinstall config + launcher into `arch/boot/`, then patch
/// syslinux APPEND lines, systemd-boot loader entries, and any UEFI grub.cfg
/// to invoke the launcher via `archiso_script=`.
pub(super) fn arch(
    engine: &ForgeIsoEngine,
    work_dir: &Path,
    extract_dir: &Path,
) -> EngineResult<()> {
    // Copy archinstall config + launcher into arch/boot/ inside the ISO.
    // At boot, the ISO is mounted at /run/archiso/bootmnt/, so the config
    // is accessible at /run/archiso/bootmnt/arch/boot/archinstall-config.json.
    let arch_boot = extract_dir.join("arch").join("boot");
    std::fs::create_dir_all(&arch_boot)?;
    std::fs::copy(
        work_dir.join("archinstall-config.json"),
        arch_boot.join("archinstall-config.json"),
    )?;
    std::fs::copy(
        work_dir.join("run-archinstall.sh"),
        arch_boot.join("run-archinstall.sh"),
    )?;
    engine.emit(EngineEvent::info(
        EventPhase::Inject,
        "injected archinstall config and launcher into arch/boot/",
    ));

    // Patch syslinux APPEND lines to add archiso_script= parameter.
    // archiso recognises archiso_script= as the path to execute after boot.
    // We must append to each APPEND line rather than replace all "APPEND"
    // occurrences globally, to preserve multi-entry syslinux configs.
    for syslinux_name in &["archiso_sys.conf", "archiso_sys-linux.conf"] {
        let syslinux_cfg = extract_dir.join("syslinux").join(syslinux_name);
        if syslinux_cfg.exists() {
            let content = std::fs::read_to_string(&syslinux_cfg)?;
            let patched = patch_lines_starting_with(
                &content,
                "APPEND ",
                " archiso_script=/arch/boot/run-archinstall.sh",
            );
            std::fs::write(&syslinux_cfg, patched)?;
        }
    }

    // Patch systemd-boot loader entries — append archiso_script= to options lines.
    let loader_entries = extract_dir.join("loader").join("entries");
    if loader_entries.exists() {
        for entry in std::fs::read_dir(&loader_entries)? {
            let entry = entry?;
            if entry.path().extension().and_then(|e| e.to_str()) == Some("conf") {
                let content = std::fs::read_to_string(entry.path())?;
                let patched = patch_lines_starting_with(
                    &content,
                    "options ",
                    " archiso_script=/arch/boot/run-archinstall.sh",
                );
                std::fs::write(entry.path(), patched)?;
            }
        }
    }

    // Also patch EFI/BOOT/grub.cfg if present (some Arch media include it).
    patch_efi_grub_if_present(extract_dir, " archiso_script=/arch/boot/run-archinstall.sh")?;
    Ok(())
}

/// Append `kernel_append` to every line whose trim_start prefix matches
/// `prefix`.  Output always terminates with a single trailing newline.
/// Used by syslinux APPEND and systemd-boot options patching.
fn patch_lines_starting_with(content: &str, prefix: &str, append: &str) -> String {
    content
        .lines()
        .map(|line| {
            if line.trim_start().starts_with(prefix) {
                format!("{}{}", line.trim_end(), append)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// If `EFI/BOOT/grub.cfg` exists under `extract_dir`, patch it via
/// `patch_efi_grub_cfg`.  No-op when the file is absent.
fn patch_efi_grub_if_present(extract_dir: &Path, kernel_append: &str) -> EngineResult<()> {
    let efi_grub = extract_dir.join("EFI").join("BOOT").join("grub.cfg");
    if efi_grub.exists() {
        let content = std::fs::read_to_string(&efi_grub)?;
        let patched = patch_efi_grub_cfg(&content, kernel_append);
        std::fs::write(&efi_grub, patched)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InjectConfig;

    #[test]
    fn patch_lines_starting_with_appends_only_to_matching_lines() {
        let input = "  APPEND foo bar\nLABEL x\n  APPEND baz\n";
        let out = patch_lines_starting_with(input, "APPEND ", " EXTRA");
        assert!(
            out.contains("APPEND foo bar EXTRA"),
            "first APPEND must be patched: {out}"
        );
        assert!(
            out.contains("APPEND baz EXTRA"),
            "second APPEND must be patched: {out}"
        );
        assert!(
            out.contains("LABEL x"),
            "non-matching line must be unchanged: {out}"
        );
    }

    #[test]
    fn patch_lines_starting_with_terminates_with_single_newline() {
        let out = patch_lines_starting_with("only a single line\n", "APPEND ", " X");
        assert!(out.ends_with('\n'), "output must end with newline");
        // No double-newline at end
        assert!(
            !out.ends_with("\n\n"),
            "must not produce trailing blank line"
        );
    }

    #[test]
    fn patch_lines_starting_with_does_nothing_when_prefix_not_present() {
        let input = "no matching lines here\n";
        let out = patch_lines_starting_with(input, "APPEND ", " X");
        // Lines retained, with single trailing newline.
        assert_eq!(out, "no matching lines here\n");
    }

    #[test]
    fn patch_efi_grub_if_present_skips_when_file_absent() {
        let dir = tempfile::tempdir().expect("dir");
        // No EFI/BOOT/grub.cfg at all
        patch_efi_grub_if_present(dir.path(), " inst.ks=cdrom:/ks.cfg")
            .expect("must succeed when file absent");
    }

    #[test]
    fn patch_efi_grub_if_present_patches_when_file_exists() {
        let dir = tempfile::tempdir().expect("dir");
        let efi = dir.path().join("EFI").join("BOOT");
        std::fs::create_dir_all(&efi).expect("mkdir");
        std::fs::write(
            efi.join("grub.cfg"),
            "menuentry 'Fedora' {\n  linuxefi /vmlinuz quiet\n}\n",
        )
        .expect("write");
        patch_efi_grub_if_present(dir.path(), " inst.ks=cdrom:/ks.cfg").expect("patch");
        let body = std::fs::read_to_string(efi.join("grub.cfg")).expect("read");
        assert!(
            body.contains("linuxefi /vmlinuz quiet inst.ks=cdrom:/ks.cfg"),
            "linuxefi line was not patched: {body}"
        );
    }

    #[test]
    fn ubuntu_place_creates_nocloud_directory_and_patches_grub() {
        let work = tempfile::tempdir().expect("work");
        let extract = tempfile::tempdir().expect("extract");
        // Pre-populate nocloud overlay (configure phase would have done this)
        let nocloud = work.path().join("overlay").join("nocloud");
        std::fs::create_dir_all(&nocloud).expect("mkdir");
        std::fs::write(nocloud.join("user-data"), b"#cloud-config\n").expect("write user-data");
        std::fs::write(nocloud.join("meta-data"), b"").expect("write meta-data");
        // Pre-populate boot/grub/grub.cfg so patching has something to do
        let grub = extract.path().join("boot").join("grub");
        std::fs::create_dir_all(&grub).expect("mkdir");
        std::fs::write(grub.join("grub.cfg"), "linux\t/casper/vmlinuz quiet ---\n")
            .expect("write grub.cfg");

        let engine = ForgeIsoEngine::new();
        let cfg = InjectConfig {
            hostname: Some("h".to_string()),
            ..Default::default()
        };
        ubuntu(&engine, &cfg, work.path(), extract.path()).expect("place ubuntu");

        // nocloud/ must be created at extract root
        assert!(
            extract.path().join("nocloud").join("user-data").exists(),
            "nocloud/user-data must be copied"
        );
        assert!(extract.path().join("nocloud").join("meta-data").exists());

        // grub.cfg must be patched
        let body = std::fs::read_to_string(grub.join("grub.cfg")).expect("read grub.cfg");
        assert!(
            body.contains("autoinstall ds=nocloud"),
            "grub.cfg must be patched: {body}"
        );
    }

    #[test]
    fn fedora_place_copies_kickstart_and_patches_grub() {
        let work = tempfile::tempdir().expect("work");
        let extract = tempfile::tempdir().expect("extract");
        std::fs::write(work.path().join("ks.cfg"), b"# kickstart\n").expect("write ks.cfg");
        let grub = extract.path().join("boot").join("grub");
        std::fs::create_dir_all(&grub).expect("mkdir");
        std::fs::write(
            grub.join("grub.cfg"),
            "linux\t/boot/vmlinuz inst.stage2=hd:LABEL=Fedora\n",
        )
        .expect("write grub.cfg");

        let engine = ForgeIsoEngine::new();
        fedora(&engine, work.path(), extract.path()).expect("place fedora");

        assert!(extract.path().join("ks.cfg").exists());
        let body = std::fs::read_to_string(grub.join("grub.cfg")).expect("read");
        assert!(
            body.contains("inst.ks=cdrom:/ks.cfg"),
            "fedora grub.cfg must be patched: {body}"
        );
    }

    #[test]
    fn mint_place_copies_preseed_and_patches_grub() {
        let work = tempfile::tempdir().expect("work");
        let extract = tempfile::tempdir().expect("extract");
        std::fs::write(work.path().join("preseed.cfg"), b"# preseed\n").expect("write");
        let grub = extract.path().join("boot").join("grub");
        std::fs::create_dir_all(&grub).expect("mkdir");
        std::fs::write(
            grub.join("grub.cfg"),
            "linux\t/casper/vmlinuz quiet splash ---\n",
        )
        .expect("write grub.cfg");

        let engine = ForgeIsoEngine::new();
        mint(&engine, work.path(), extract.path()).expect("place mint");

        assert!(extract.path().join("preseed.cfg").exists());
        let body = std::fs::read_to_string(grub.join("grub.cfg")).expect("read");
        assert!(
            body.contains("preseed/file=/cdrom/preseed.cfg"),
            "mint grub.cfg must be patched: {body}"
        );
    }

    #[test]
    fn arch_place_copies_files_and_patches_syslinux() {
        let work = tempfile::tempdir().expect("work");
        let extract = tempfile::tempdir().expect("extract");
        // Put archinstall config + launcher in workspace
        std::fs::write(work.path().join("archinstall-config.json"), b"{}\n").expect("write json");
        std::fs::write(
            work.path().join("run-archinstall.sh"),
            b"#!/bin/sh\necho hi\n",
        )
        .expect("write launcher");
        // Pre-populate syslinux config the patcher should find
        let syslinux = extract.path().join("syslinux");
        std::fs::create_dir_all(&syslinux).expect("mkdir syslinux");
        std::fs::write(
            syslinux.join("archiso_sys.conf"),
            "LABEL arch64\n  APPEND initrd=/arch/boot/x86_64/initramfs-linux.img\n",
        )
        .expect("write syslinux conf");

        let engine = ForgeIsoEngine::new();
        arch(&engine, work.path(), extract.path()).expect("place arch");

        // archinstall files copied into arch/boot/
        let arch_boot = extract.path().join("arch").join("boot");
        assert!(arch_boot.join("archinstall-config.json").exists());
        assert!(arch_boot.join("run-archinstall.sh").exists());

        // syslinux APPEND patched
        let body = std::fs::read_to_string(syslinux.join("archiso_sys.conf")).expect("read");
        assert!(
            body.contains("archiso_script=/arch/boot/run-archinstall.sh"),
            "syslinux APPEND must contain archiso_script: {body}"
        );
    }
}
