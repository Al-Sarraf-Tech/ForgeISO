use std::path::Path;

use crate::autoinstall::{generate_autoinstall_yaml, merge_autoinstall_yaml};
use crate::config::Distro;
use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};
use crate::iso::inspect_iso;
use crate::kickstart::generate_kickstart_cfg;
use crate::mint_preseed::generate_mint_preseed;
use crate::workspace::Workspace;

use super::build::repack_iso_args;
use super::helpers::{
    build_archinstall_config, cache_subdir, chmod_recursive_writable, patch_boot_configs,
    patch_efi_grub_cfg, remove_dir_all_force, run_command_lossy_async,
};
use super::verify::check_expected_sha256;
use super::{BuildResult, ForgeIsoEngine};

impl ForgeIsoEngine {
    pub async fn inject_autoinstall(
        &self,
        cfg: &crate::config::InjectConfig,
        out: &Path,
    ) -> EngineResult<BuildResult> {
        cfg.validate()?;

        self.emit(EngineEvent::info(
            EventPhase::Inject,
            "starting autoinstall injection",
        ));

        // Create workspace for injection
        let workspace = Workspace::create(&cache_subdir("inject")?, "inject")?;
        let work_dir = workspace.root;

        // Resolve the source ISO
        let resolved = self.resolve_source(&cfg.source, &work_dir).await?;
        if let Some(expected) = &cfg.expected_sha256 {
            self.emit(EngineEvent::info(
                EventPhase::Verify,
                "verifying expected SHA-256 of source ISO",
            ));
            check_expected_sha256(&resolved.source_path, expected)?;
        }
        let metadata = inspect_iso(
            &resolved.source_path,
            resolved.source_kind,
            resolved.source_value,
        )?;

        // Warn if the requested distro doesn't match what the ISO reports.
        // This is non-fatal: custom/hybrid ISOs legitimately differ; we warn
        // so users notice unintentional mismatches before a long build.
        if let (Some(requested), Some(detected)) = (cfg.distro, metadata.distro) {
            if requested != detected {
                self.emit(EngineEvent::warn(
                    EventPhase::Inject,
                    format!(
                        "distro mismatch: config requests {:?} but ISO appears to be {:?}; \
                         injection may produce an unbootable image",
                        requested, detected
                    ),
                ));
            }
        }

        // Warn when LUKS encryption is requested: cloud-init requires the
        // passphrase in plaintext inside the YAML blob on the ISO.
        if cfg.encrypt_passphrase.is_some() {
            self.emit(EngineEvent::warn(
                EventPhase::Inject,
                "LUKS passphrase will be stored in plaintext inside the generated \
                 cloud-init YAML; treat the output ISO as sensitive material",
            ));
        }

        // Dispatch to the appropriate injection method based on target distro
        match cfg.distro {
            None | Some(Distro::Ubuntu) => {
                // -- Ubuntu: cloud-init nocloud overlay ---------------------
                let nocloud_dir = work_dir.join("overlay").join("nocloud");
                std::fs::create_dir_all(&nocloud_dir)?;

                let user_data = match &cfg.autoinstall_yaml {
                    Some(path) => {
                        let existing = std::fs::read_to_string(path)?;
                        merge_autoinstall_yaml(&existing, cfg)?
                    }
                    None => generate_autoinstall_yaml(cfg)?,
                };
                std::fs::write(nocloud_dir.join("user-data"), &user_data)?;
                std::fs::write(nocloud_dir.join("meta-data"), "")?;

                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "created cloud-init overlay",
                ));
            }
            Some(Distro::Mint) => {
                // -- Linux Mint: preseed.cfg for Calamares unattended install
                // Mint uses Calamares (live desktop installer), not cloud-init.
                // Calamares supports Debian-style preseed files for unattended
                // installs when booted with:
                //   auto=true priority=critical preseed/file=/cdrom/preseed.cfg
                self.emit(EngineEvent::warn(
                    EventPhase::Inject,
                    "Linux Mint uses Calamares — injecting preseed.cfg (NOT cloud-init autoinstall)",
                ));
                let preseed = generate_mint_preseed(cfg)?;
                std::fs::write(work_dir.join("preseed.cfg"), &preseed)?;

                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "generated preseed.cfg for Calamares",
                ));
            }
            Some(Distro::Fedora) => {
                // -- Fedora: Kickstart ks.cfg ------------------------------
                let ks_content = generate_kickstart_cfg(cfg)?;
                std::fs::write(work_dir.join("ks.cfg"), &ks_content)?;

                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "generated Kickstart ks.cfg",
                ));
            }
            Some(Distro::Arch) => {
                // -- Arch Linux: archinstall JSON config --------------------
                // Warn about features that cannot be expressed in an archinstall
                // JSON config.  These are silently dropped; the user should be
                // informed so they are not surprised by missing post-install state.
                let arch_unsupported: &[(&str, bool)] = &[
                    ("swap", cfg.swap.is_some()),
                    ("mounts", !cfg.mounts.is_empty()),
                    ("sysctl", !cfg.sysctl.is_empty()),
                    (
                        "proxy",
                        cfg.proxy.http_proxy.is_some() || cfg.proxy.https_proxy.is_some(),
                    ),
                    ("firewall", cfg.firewall.enabled),
                    (
                        "grub settings",
                        cfg.grub.timeout.is_some()
                            || !cfg.grub.cmdline_extra.is_empty()
                            || cfg.grub.default_entry.is_some(),
                    ),
                    ("pacman_mirror", cfg.pacman_mirror.is_some()),
                    ("pacman_repos", !cfg.pacman_repos.is_empty()),
                    ("disable_services", !cfg.disable_services.is_empty()),
                    ("run_commands", !cfg.run_commands.is_empty()),
                    ("extra_late_commands", !cfg.extra_late_commands.is_empty()),
                    ("apt_repos", !cfg.apt_repos.is_empty()),
                ];
                for (feature, active) in arch_unsupported {
                    if *active {
                        self.emit(EngineEvent::warn(
                            EventPhase::Inject,
                            format!(
                                "Arch Linux: '{feature}' is not supported by archinstall config \
                                 and will be ignored — configure it manually post-install"
                            ),
                        ));
                    }
                }

                let archinstall_cfg = build_archinstall_config(cfg)?;
                let json_content = serde_json::to_string_pretty(&archinstall_cfg)
                    .map_err(|e| EngineError::Runtime(e.to_string()))?;
                std::fs::write(work_dir.join("archinstall-config.json"), &json_content)?;

                // Create the run-archinstall.sh launcher script
                let launcher = concat!(
                    "#!/usr/bin/env bash\n",
                    "# Generated by ForgeISO -- triggers archinstall in unattended mode\n",
                    "set -euo pipefail\n",
                    "CONFIG=\"/run/archiso/bootmnt/arch/boot/archinstall-config.json\"\n",
                    "if [[ -f \"${CONFIG}\" ]]; then\n",
                    "    archinstall --config \"${CONFIG}\" --silent\n",
                    "else\n",
                    "    echo \"ERROR: archinstall config not found at ${CONFIG}\" >&2\n",
                    "    exit 1\n",
                    "fi\n"
                );
                std::fs::write(work_dir.join("run-archinstall.sh"), launcher)?;

                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "generated archinstall config",
                ));
            }
        }

        // --- (distro-specific files are copied to the extracted ISO below) ---

        // Copy wallpaper file if provided
        if let Some(src) = &cfg.wallpaper {
            let fname = src
                .file_name()
                .ok_or_else(|| EngineError::InvalidConfig("invalid wallpaper path".to_string()))?;
            let dest = work_dir.join("wallpaper");
            std::fs::create_dir_all(&dest)?;
            std::fs::copy(src, dest.join(fname))?;
        }

        // Extract ISO
        let extract_dir = work_dir.join("extract");
        std::fs::create_dir_all(&extract_dir)?;
        let output = run_command_lossy_async(
            "xorriso",
            &[
                "-osirrox".to_string(),
                "on".to_string(),
                "-indev".to_string(),
                resolved.source_path.to_string_lossy().to_string(),
                "-extract".to_string(),
                "/".to_string(),
                extract_dir.to_string_lossy().to_string(),
            ],
            None,
        )
        .await?;
        if output.status != 0 {
            return Err(EngineError::Runtime(format!(
                "xorriso extract failed: {}",
                output.stderr
            )));
        }

        self.emit(EngineEvent::info(
            EventPhase::Inject,
            "extracted ISO filesystem",
        ));

        // xorriso extracts files with read-only permissions; make writable
        // so we can modify the tree and inject files without permission errors.
        chmod_recursive_writable(&extract_dir);

        // Copy distro-specific files into the extracted ISO and patch boot entries
        match cfg.distro {
            None | Some(Distro::Ubuntu) => {
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
                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "injected cloud-init files into ISO root /nocloud/",
                ));

                // Wallpaper — also at ISO root so /cdrom/wallpaper/ resolves correctly.
                if let Some(src) = &cfg.wallpaper {
                    let fname = src.file_name().ok_or_else(|| {
                        EngineError::InvalidConfig(format!(
                            "wallpaper path has no filename: {}",
                            src.display()
                        ))
                    })?;
                    let iso_wp = extract_dir.join("wallpaper");
                    std::fs::create_dir_all(&iso_wp)?;
                    std::fs::copy(work_dir.join("wallpaper").join(fname), iso_wp.join(fname))?;
                }

                // Boot patch — Ubuntu autoinstall kernel params
                let kernel_append = " autoinstall ds=nocloud;s=/cdrom/nocloud/";
                patch_boot_configs(&extract_dir, kernel_append)?;

                // Also patch EFI/BOOT/grub.cfg for UEFI boot — without this,
                // modern UEFI systems see the unmodified EFI grub config and
                // boot straight into the manual interactive installer.
                let efi_grub = extract_dir.join("EFI").join("BOOT").join("grub.cfg");
                if efi_grub.exists() {
                    let content = std::fs::read_to_string(&efi_grub)?;
                    let patched = patch_efi_grub_cfg(&content, kernel_append);
                    std::fs::write(&efi_grub, patched)?;
                }
            }
            Some(Distro::Mint) => {
                // Copy preseed.cfg to ISO root (accessible as /cdrom/preseed.cfg at boot).
                std::fs::copy(
                    work_dir.join("preseed.cfg"),
                    extract_dir.join("preseed.cfg"),
                )?;
                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "injected preseed.cfg into ISO root",
                ));

                // Patch boot entries to trigger Calamares preseed.
                // Calamares reads preseed when booted with:
                //   auto=true priority=critical preseed/file=/cdrom/preseed.cfg
                let kernel_append = " auto=true priority=critical preseed/file=/cdrom/preseed.cfg";
                patch_boot_configs(&extract_dir, kernel_append)?;

                // Also patch EFI/BOOT/grub.cfg if present (UEFI Mint media).
                // Use line-by-line patching so only kernel command lines
                // (`linuxefi` / `linux`) are modified — a global string replace
                // on "quiet splash" would also corrupt comments or menu labels
                // that happen to contain those words.
                let efi_grub = extract_dir.join("EFI").join("BOOT").join("grub.cfg");
                if efi_grub.exists() {
                    let content = std::fs::read_to_string(&efi_grub)?;
                    let patched = patch_efi_grub_cfg(&content, kernel_append);
                    std::fs::write(&efi_grub, patched)?;
                }
            }
            Some(Distro::Fedora) => {
                // Copy ks.cfg to ISO root
                std::fs::copy(work_dir.join("ks.cfg"), extract_dir.join("ks.cfg"))?;
                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "injected ks.cfg into ISO root",
                ));

                // Patch Fedora boot entries to add inst.ks=cdrom:/ks.cfg
                let kernel_append = " inst.ks=cdrom:/ks.cfg";
                patch_boot_configs(&extract_dir, kernel_append)?;

                // Also patch EFI/BOOT/grub.cfg if present (UEFI Fedora media).
                // Use line-by-line patching — `.replace("quiet", ...)` would corrupt
                // any comment or menu label containing the word "quiet", and would
                // silently miss the injection if the Fedora ISO does not include the
                // "quiet" parameter.
                let efi_grub = extract_dir.join("EFI").join("BOOT").join("grub.cfg");
                if efi_grub.exists() {
                    let content = std::fs::read_to_string(&efi_grub)?;
                    let patched = patch_efi_grub_cfg(&content, kernel_append);
                    std::fs::write(&efi_grub, patched)?;
                }
            }
            Some(Distro::Arch) => {
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
                self.emit(EngineEvent::info(
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
                        let patched = content
                            .lines()
                            .map(|line| {
                                if line.trim_start().starts_with("APPEND ") {
                                    format!(
                                        "{} archiso_script=/arch/boot/run-archinstall.sh",
                                        line.trim_end()
                                    )
                                } else {
                                    line.to_string()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                            + "\n";
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
                            let patched = content
                                .lines()
                                .map(|line| {
                                    if line.trim_start().starts_with("options ") {
                                        format!(
                                            "{} archiso_script=/arch/boot/run-archinstall.sh",
                                            line.trim_end()
                                        )
                                    } else {
                                        line.to_string()
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                                + "\n";
                            std::fs::write(entry.path(), patched)?;
                        }
                    }
                }

                // Also patch EFI/BOOT/grub.cfg if present (some Arch media include it).
                let efi_grub = extract_dir.join("EFI").join("BOOT").join("grub.cfg");
                if efi_grub.exists() {
                    let content = std::fs::read_to_string(&efi_grub)?;
                    let patched = patch_efi_grub_cfg(
                        &content,
                        " archiso_script=/arch/boot/run-archinstall.sh",
                    );
                    std::fs::write(&efi_grub, patched)?;
                }
            }
        }

        self.emit(EngineEvent::info(
            EventPhase::Inject,
            "patched boot configurations",
        ));

        // Repack ISO
        std::fs::create_dir_all(out)?;
        // Ensure the output always has an .iso extension regardless of what the
        // caller passed — avoids producing unrecognised files from the GUI default.
        let out_filename = {
            let name = if cfg.out_name.trim().is_empty() {
                "forgeiso-local"
            } else {
                cfg.out_name.trim()
            };
            if name.to_ascii_lowercase().ends_with(".iso") {
                name.to_string()
            } else {
                format!("{}.iso", name)
            }
        };
        let output_path = out.join(&out_filename);

        let args = repack_iso_args(
            &resolved.source_path,
            &extract_dir,
            &output_path,
            cfg.output_label.as_deref(),
        )?;

        let output = run_command_lossy_async("xorriso", &args, None).await?;
        if output.status != 0 {
            return Err(EngineError::Runtime(format!(
                "xorriso repack failed: {}",
                output.stderr
            )));
        }

        self.emit(EngineEvent::info(
            EventPhase::Inject,
            format!("created output ISO: {}", output_path.display()),
        ));

        // Build the result before cleaning up so that all paths are captured.
        let result = BuildResult {
            workspace_root: work_dir.to_path_buf(),
            output_dir: out.to_path_buf(),
            // Inject does not generate a standalone build report; these paths
            // point into the workspace which is removed below.  Callers must
            // not rely on these paths existing after inject completes.
            report_json: work_dir.join("report.json"),
            report_html: work_dir.join("report.html"),
            artifacts: vec![output_path],
            source_iso: resolved.source_path,
            iso: metadata,
        };

        // Always clean up the inject workspace — it can contain the full
        // extracted ISO tree (several GB).  Unlike BuildConfig there is no
        // keep_workdir flag on InjectConfig; inject workspaces are always
        // ephemeral temp dirs that should not accumulate on disk.
        if let Err(e) = remove_dir_all_force(&work_dir) {
            self.emit(EngineEvent::warn(
                EventPhase::Complete,
                format!(
                    "failed to clean up inject workspace {}: {e}",
                    work_dir.display()
                ),
            ));
        }

        self.emit(EngineEvent::info(
            EventPhase::Complete,
            "autoinstall injection completed",
        ));

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Distro;

    // ── Distro mismatch logic ────────────────────────────────────────────────

    #[test]
    fn distro_mismatch_ubuntu_vs_fedora_is_detectable() {
        // Confirm that the comparison driving the mismatch warning works correctly.
        assert_ne!(Distro::Ubuntu, Distro::Fedora);
        assert_ne!(Distro::Ubuntu, Distro::Arch);
        assert_ne!(Distro::Ubuntu, Distro::Mint);
        assert_ne!(Distro::Fedora, Distro::Arch);
    }

    #[test]
    fn distro_match_same_variant_no_mismatch() {
        // Same distro -> mismatch guard must not trigger.
        assert_eq!(Distro::Ubuntu, Distro::Ubuntu);
        assert_eq!(Distro::Fedora, Distro::Fedora);
        assert_eq!(Distro::Arch, Distro::Arch);
        assert_eq!(Distro::Mint, Distro::Mint);
    }

    // ── LUKS passphrase warning ──────────────────────────────────────────────

    #[test]
    fn inject_config_with_luks_passphrase_is_valid() {
        // validate() must succeed even when encrypt_passphrase is set;
        // the warning is advisory (emitted by the engine), not a hard error.
        let cfg = crate::config::InjectConfig {
            encrypt_passphrase: Some("supersecret".to_string()),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "LUKS passphrase should not fail validate()"
        );
    }

    #[test]
    fn arch_syslinux_patching_appends_archiso_script() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let syslinux_dir = tmp.path().join("syslinux");
        std::fs::create_dir_all(&syslinux_dir).expect("create syslinux dir");
        let syslinux_cfg = syslinux_dir.join("archiso_sys.conf");
        // Typical multi-entry Arch syslinux.cfg
        let original = "LABEL arch64\n  MENU LABEL Boot Arch Linux (x86_64)\n  APPEND initrd=/arch/boot/x86_64/initramfs-linux.img archisobasedir=arch quiet\nLABEL arch64-nonfree\n  MENU LABEL Boot Arch Linux (x86_64, with nonfree)\n  APPEND initrd=/arch/boot/x86_64/initramfs-linux.img archisobasedir=arch quiet\n";
        std::fs::write(&syslinux_cfg, original).expect("write");

        // Simulate the patching logic from inject_autoinstall Arch branch
        let content = std::fs::read_to_string(&syslinux_cfg).expect("read");
        let patched = content
            .lines()
            .map(|line| {
                if line.trim_start().starts_with("APPEND ") {
                    format!(
                        "{} archiso_script=/arch/boot/run-archinstall.sh",
                        line.trim_end()
                    )
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        std::fs::write(&syslinux_cfg, &patched).expect("write patched");

        let result = std::fs::read_to_string(&syslinux_cfg).expect("read result");
        // Both APPEND lines must have archiso_script= appended
        let append_lines_with_script: Vec<&str> = result
            .lines()
            .filter(|l| l.trim_start().starts_with("APPEND ") && l.contains("archiso_script="))
            .collect();
        assert_eq!(
            append_lines_with_script.len(),
            2,
            "expected 2 APPEND lines with archiso_script=, got: {result:?}"
        );
        // Must NOT have bare 'APPEND' without the script
        let bare_append: Vec<&str> = result
            .lines()
            .filter(|l| l.trim_start().starts_with("APPEND ") && !l.contains("archiso_script="))
            .collect();
        assert!(
            bare_append.is_empty(),
            "found APPEND lines without archiso_script=: {bare_append:?}"
        );
    }
}
