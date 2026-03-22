use std::path::Path;

use crate::config::BuildConfig;
use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};
use crate::iso::{inspect_iso, IsoMetadata};
use crate::report::BuildReport;
use crate::workspace::Workspace;

use super::helpers::{
    chmod_recursive_writable, copy_dir_contents, is_squashfs_path, remove_dir_all_force,
    require_tools, run_command_capture_async, run_command_lossy, run_command_lossy_async,
    sanitize_filename,
};
use super::verify::check_expected_sha256;
use super::{BuildResult, ForgeIsoEngine};

impl ForgeIsoEngine {
    pub async fn build_from_file(
        &self,
        config_path: &Path,
        out_dir: &Path,
    ) -> EngineResult<BuildResult> {
        let cfg = BuildConfig::from_path(config_path)?;
        self.build(&cfg, out_dir).await
    }

    pub async fn build(&self, cfg: &BuildConfig, out_dir: &Path) -> EngineResult<BuildResult> {
        cfg.validate()?;
        super::helpers::ensure_linux_host()?;

        self.emit(EngineEvent::info(
            EventPhase::Configure,
            format!("starting local ISO build for '{}'", cfg.name),
        ));

        let workspace = Workspace::create(out_dir, &cfg.name)?;
        let resolved = self.resolve_source(&cfg.source, &workspace.input).await?;
        if let Some(expected) = &cfg.expected_sha256 {
            self.emit(EngineEvent::info(
                EventPhase::Verify,
                "verifying expected SHA-256 of source ISO",
            ));
            check_expected_sha256(&resolved.source_path, expected)?;
        }
        let iso = inspect_iso(
            &resolved.source_path,
            resolved.source_kind,
            resolved.source_value.clone(),
        )?;

        self.emit(EngineEvent::info(
            EventPhase::Build,
            format!("using source ISO {}", iso.source_path.display()),
        ));

        require_tools(&["xorriso"])?;
        let extract_dir = workspace.work.join("iso-tree");
        std::fs::create_dir_all(&extract_dir)?;
        // xorriso may exit non-zero on some ISOs (e.g. for non-fatal permission
        // quirks on certain files) even when extraction fully succeeded.  Use the
        // lossy runner so we still get stdout/stderr for diagnostics, and only
        // fail on an explicit non-zero status — matching the pattern used in
        // inject_autoinstall for the same operation.
        let extract_out = run_command_lossy_async(
            "xorriso",
            &[
                "-osirrox".to_string(),
                "on".to_string(),
                "-indev".to_string(),
                iso.source_path.display().to_string(),
                "-extract".to_string(),
                "/".to_string(),
                extract_dir.display().to_string(),
            ],
            None,
        )
        .await?;
        if extract_out.status != 0 {
            return Err(EngineError::Runtime(format!(
                "xorriso extract failed (status {}): {}",
                extract_out.status, extract_out.stderr
            )));
        }
        // xorriso extracts files with read-only permissions; make writable
        // so we can modify the tree and clean up afterwards.
        chmod_recursive_writable(&extract_dir);

        let mut warnings = iso.warnings.clone();
        let mut rootfs_dir = None;
        if let Some(rootfs_rel) = iso.rootfs_path.as_deref() {
            let rootfs_image = extract_dir.join(rootfs_rel);
            if rootfs_image.exists() && is_squashfs_path(rootfs_rel) {
                require_tools(&["unsquashfs", "mksquashfs"])?;
                let unpack_dir = workspace.work.join("rootfs");
                std::fs::create_dir_all(&unpack_dir)?;
                run_command_lossy_async(
                    "unsquashfs",
                    &[
                        "-f".to_string(),
                        "-no-xattrs".to_string(),
                        "-d".to_string(),
                        unpack_dir.display().to_string(),
                        rootfs_image.display().to_string(),
                    ],
                    None,
                )
                .await?;
                if let Some(overlay) = cfg.overlay_dir.as_deref() {
                    copy_dir_contents(overlay, &unpack_dir)?;
                }
                write_rootfs_manifest(&unpack_dir, cfg, &iso)?;
                std::fs::remove_file(&rootfs_image)?;
                run_command_capture_async(
                    "mksquashfs",
                    &[
                        unpack_dir.display().to_string(),
                        rootfs_image.display().to_string(),
                        "-comp".to_string(),
                        "xz".to_string(),
                        "-noappend".to_string(),
                        "-no-xattrs".to_string(),
                    ],
                    None,
                )
                .await?;
                rootfs_dir = Some(unpack_dir);
            } else if rootfs_image.exists() {
                warnings.push(format!(
                    "Root filesystem image '{}' is not yet rewriteable offline; only top-level ISO files will be updated",
                    rootfs_rel
                ));
            }
        } else {
            warnings.push("No known root filesystem image was detected inside the ISO".to_string());
        }

        if rootfs_dir.is_none() {
            if let Some(overlay) = cfg.overlay_dir.as_deref() {
                copy_dir_contents(overlay, &extract_dir)?;
            }
        }
        write_iso_manifest(&extract_dir, cfg, &iso)?;

        let output_iso = out_dir.join(format!("{}.iso", sanitize_filename(&cfg.name)));
        let repack_args = repack_iso_args(
            &iso.source_path,
            &extract_dir,
            &output_iso,
            cfg.output_label.as_deref(),
        )?;
        run_command_capture_async("xorriso", &repack_args, None).await?;

        let mut report = BuildReport::new(cfg, &iso);
        report.metadata.warnings.extend(warnings);
        report
            .metadata
            .tool_versions
            .insert("engine".to_string(), env!("CARGO_PKG_VERSION").to_string());
        report
            .metadata
            .tool_versions
            .insert("host_os".to_string(), std::env::consts::OS.to_string());
        report.artifacts.push(output_iso.display().to_string());

        let report_json = out_dir.join("build-report.json");
        let report_html = out_dir.join("build-report.html");
        report.write_json(&report_json)?;
        report.write_html(&report_html)?;

        self.emit(EngineEvent::info(
            EventPhase::Complete,
            format!("build completed: {}", output_iso.display()),
        ));

        let workspace_root = workspace.root.clone();
        if !cfg.keep_workdir {
            if let Err(e) = remove_dir_all_force(&workspace.root) {
                self.emit(EngineEvent::warn(
                    EventPhase::Complete,
                    format!(
                        "failed to clean up workspace {}: {e}",
                        workspace.root.display()
                    ),
                ));
            }
        }

        Ok(BuildResult {
            workspace_root,
            output_dir: out_dir.to_path_buf(),
            report_json,
            report_html,
            artifacts: vec![output_iso],
            source_iso: resolved.source_path,
            iso,
        })
    }
}

pub(super) fn repack_iso_args(
    source_iso: &Path,
    extract_dir: &Path,
    output_iso: &Path,
    output_label: Option<&str>,
) -> EngineResult<Vec<String>> {
    // xorriso -report_el_torito exits non-zero on ISOs with no El Torito boot
    // catalog (a valid state for non-bootable ISOs).  Use run_command_lossy so
    // we always capture whatever stdout is available, then parse it; if xorriso
    // produced no output the result is an empty boot_args and the ISO is
    // repacked without boot flags — which is correct for non-bootable sources.
    let report = run_command_lossy(
        "xorriso",
        &[
            "-indev".to_string(),
            source_iso.display().to_string(),
            "-report_el_torito".to_string(),
            "as_mkisofs".to_string(),
        ],
        None,
    )?;

    let mut boot_args = parse_mkisofs_report(&report.stdout)?;
    if output_label.is_some() {
        boot_args = strip_volume_args(&boot_args);
    }

    let mut args = vec![
        "-as".to_string(),
        "mkisofs".to_string(),
        "-o".to_string(),
        output_iso.display().to_string(),
    ];
    args.extend(boot_args);
    if let Some(label) = output_label {
        args.push("-V".to_string());
        args.push(label.to_string());
    }
    args.push(extract_dir.display().to_string());
    Ok(args)
}

fn parse_mkisofs_report(report: &str) -> EngineResult<Vec<String>> {
    let mut args = Vec::new();
    for line in report
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let parsed = shell_words::split(line).map_err(|error| {
            EngineError::Runtime(format!(
                "failed to parse xorriso mkisofs report line '{line}': {error}"
            ))
        })?;
        args.extend(parsed);
    }
    Ok(args)
}

fn strip_volume_args(args: &[String]) -> Vec<String> {
    let mut filtered = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "-V" || arg == "-volid" {
            // Skip the flag and its value (if present).
            index += if index + 1 < args.len() { 2 } else { 1 };
            continue;
        }
        filtered.push(arg.clone());
        index += 1;
    }
    filtered
}

fn write_iso_manifest(
    extract_dir: &Path,
    cfg: &BuildConfig,
    iso: &IsoMetadata,
) -> EngineResult<()> {
    let manifest = serde_json::json!({
        "name": cfg.name,
        "profile": cfg.profile,
        "source": cfg.source.display_value(),
        "inspected": iso,
        "generated_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(
        extract_dir.join("forgeiso-build.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

fn write_rootfs_manifest(
    rootfs_dir: &Path,
    cfg: &BuildConfig,
    iso: &IsoMetadata,
) -> EngineResult<()> {
    let etc = rootfs_dir.join("etc");
    std::fs::create_dir_all(&etc)?;
    let manifest = serde_json::json!({
        "name": cfg.name,
        "profile": cfg.profile,
        "source": cfg.source.display_value(),
        "sha256": iso.sha256,
        "generated_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(
        etc.join("forgeiso-build.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_xorriso_mkisofs_report() {
        let report = "\
-V 'ISOIMAGE'\n\
--grub2-mbr --interval:local_fs:0s-15s:zero_mbrpt,zero_gpt,zero_apm:'/tmp/source.iso'\n\
-efi-boot-part --efi-boot-image\n\
-c '/boot.catalog'\n";

        let args = parse_mkisofs_report(report).expect("report should parse");

        assert_eq!(args[0], "-V");
        assert_eq!(args[1], "ISOIMAGE");
        assert_eq!(args[2], "--grub2-mbr");
        assert_eq!(
            args[3],
            "--interval:local_fs:0s-15s:zero_mbrpt,zero_gpt,zero_apm:/tmp/source.iso"
        );
        assert_eq!(args[4], "-efi-boot-part");
        assert_eq!(args[5], "--efi-boot-image");
        assert_eq!(args[6], "-c");
        assert_eq!(args[7], "/boot.catalog");
    }

    #[test]
    fn parse_mkisofs_report_empty_input_returns_empty_args() {
        // xorriso exits non-zero (and produces no stdout) for ISOs with no El
        // Torito boot records.  parse_mkisofs_report("") must return an empty
        // Vec — not an error — so repack proceeds without boot flags.
        let args = parse_mkisofs_report("").expect("empty report must not error");
        assert!(
            args.is_empty(),
            "empty xorriso output must produce empty boot_args, got: {args:?}"
        );
    }

    #[test]
    fn strips_existing_volume_flag_before_override() {
        let args = vec![
            "-V".to_string(),
            "OLDLABEL".to_string(),
            "--grub2-mbr".to_string(),
            "payload".to_string(),
        ];

        let stripped = strip_volume_args(&args);

        assert_eq!(
            stripped,
            vec!["--grub2-mbr".to_string(), "payload".to_string()]
        );
    }
}
