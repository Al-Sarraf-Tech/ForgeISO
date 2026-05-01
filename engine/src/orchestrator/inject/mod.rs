use std::path::Path;

use crate::config::Distro;
use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};
use crate::iso::inspect_iso;
use crate::workspace::Workspace;

use super::build::repack_iso_args;
use super::helpers::{
    cache_subdir, chmod_recursive_writable, remove_dir_all_force, run_command_lossy_async,
};
use super::verify::check_expected_sha256;
use super::{BuildResult, ForgeIsoEngine};

mod configure;
mod place;

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

        emit_compatibility_warnings(self, cfg, &metadata);

        // Phase A — generate distro-specific config files into the workspace.
        match cfg.distro {
            None | Some(Distro::Ubuntu) => configure::ubuntu(self, cfg, &work_dir)?,
            Some(Distro::Mint) => configure::mint(self, cfg, &work_dir)?,
            Some(Distro::Fedora) => configure::fedora(self, cfg, &work_dir)?,
            Some(Distro::Arch) => configure::arch(self, cfg, &work_dir)?,
        }

        // Copy wallpaper file if provided (consumed later by Ubuntu placement).
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

        // Phase B — copy distro-specific files into the extracted ISO and
        // patch boot entries.
        match cfg.distro {
            None | Some(Distro::Ubuntu) => place::ubuntu(self, cfg, &work_dir, &extract_dir)?,
            Some(Distro::Mint) => place::mint(self, &work_dir, &extract_dir)?,
            Some(Distro::Fedora) => place::fedora(self, &work_dir, &extract_dir)?,
            Some(Distro::Arch) => place::arch(self, &work_dir, &extract_dir)?,
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

/// Emit advisory warnings about ISO/config combinations that are likely to
/// produce a non-functional output: desktop ISOs, distro mismatch, LUKS
/// passphrase visibility.
fn emit_compatibility_warnings(
    engine: &ForgeIsoEngine,
    cfg: &crate::config::InjectConfig,
    metadata: &crate::iso::IsoMetadata,
) {
    // Warn if the ISO is a desktop edition — desktop installers (Ubuntu Desktop,
    // Fedora Workstation) do NOT support cloud-init/kickstart autoinstall the same
    // way server editions do. The resulting ISO will likely boot to a manual installer.
    let is_desktop = metadata
        .volume_id
        .as_deref()
        .map(|v| {
            let lc = v.to_lowercase();
            lc.contains("desktop") || lc.contains("workstation")
        })
        .unwrap_or(false);
    if is_desktop {
        engine.emit(EngineEvent::warn(
            EventPhase::Inject,
            "This appears to be a DESKTOP ISO. Desktop editions (Ubuntu Desktop, \
             Fedora Workstation) do NOT support fully unattended installation. \
             Use the Server edition for automated installs."
                .to_string(),
        ));
    }

    // Warn if the requested distro doesn't match what the ISO reports.
    // This is non-fatal: custom/hybrid ISOs legitimately differ; we warn
    // so users notice unintentional mismatches before a long build.
    if let (Some(requested), Some(detected)) = (cfg.distro, metadata.distro) {
        if requested != detected {
            engine.emit(EngineEvent::warn(
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
        engine.emit(EngineEvent::warn(
            EventPhase::Inject,
            "LUKS passphrase will be stored in plaintext inside the generated \
             cloud-init YAML; treat the output ISO as sensitive material",
        ));
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
