use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};
use crate::scanner::run_scans;

use super::helpers::{ensure_linux_host, ovmf_path, require_tools};
use super::{ForgeIsoEngine, ScanResult, TestResult};

impl ForgeIsoEngine {
    /// Run security scans (SBOM, vulnerability, secrets) on `artifact` using the
    /// policy in `policy_file`, writing a `scan-report.json` to `out_dir`.
    ///
    /// When `policy_file` is `None`, the default [`crate::config::ScanPolicy`] is used.
    /// Returns an error if the policy file cannot be read or any scan fails fatally.
    pub async fn scan(
        &self,
        artifact: &Path,
        policy_file: Option<&Path>,
        out_dir: &Path,
    ) -> EngineResult<ScanResult> {
        // Top-level span for the scan operation. Wraps policy load + scan
        // execution + report serialization for OTLP grouping.
        let _scan_span =
            tracing::info_span!("scan_phase", artifact = %artifact.display()).entered();

        let policy = if let Some(path) = policy_file {
            let raw = std::fs::read_to_string(path)?;
            serde_yaml::from_str(&raw)?
        } else {
            crate::config::ScanPolicy::default()
        };

        self.emit(EngineEvent::info(
            EventPhase::Scan,
            format!("running local scans for {}", artifact.display()),
        ));
        let summary = run_scans(artifact, out_dir, &policy).await?;
        let report_json = out_dir.join("scan-report.json");
        std::fs::write(&report_json, serde_json::to_vec_pretty(&summary)?)?;
        self.emit(EngineEvent::info(EventPhase::Complete, "scan completed"));
        Ok(ScanResult {
            report: summary,
            report_json,
        })
    }

    /// Boot-smoke-test `iso` with QEMU: run BIOS and/or UEFI smoke tests,
    /// capturing serial logs under `out_dir`.
    ///
    /// Each boot attempt runs for up to 30 seconds; QEMU is killed and the
    /// test is marked as passed unless a known boot-failure string appears in the
    /// serial log. Returns an error when QEMU or OVMF firmware is missing, or
    /// when the ISO file does not exist.
    pub async fn test_iso(
        &self,
        iso: &Path,
        bios: bool,
        uefi: bool,
        out_dir: &Path,
    ) -> EngineResult<TestResult> {
        ensure_linux_host()?;
        require_tools(&["qemu-system-x86_64"])?;
        if !iso.exists() {
            return Err(EngineError::NotFound(format!(
                "ISO does not exist: {}",
                iso.display()
            )));
        }

        std::fs::create_dir_all(out_dir)?;
        let mut logs = Vec::new();
        let mut passed = true;

        if bios {
            let log = out_dir.join("bios-serial.log");
            run_qemu_smoke(iso, None, &log).await?;
            logs.push(log);
        }

        if uefi {
            let firmware = ovmf_path()?;
            let log = out_dir.join("uefi-serial.log");
            run_qemu_smoke(iso, Some(&firmware), &log).await?;
            logs.push(log);
        }

        for log in &logs {
            if std::fs::metadata(log).map(|meta| meta.len()).unwrap_or(0) == 0 {
                passed = false;
                continue;
            }

            let body = std::fs::read_to_string(log)
                .unwrap_or_default()
                .to_lowercase();
            if body.contains("no bootable option or device")
                || body.contains("failed to load boot")
                || body.contains("kernel panic")
                || body.contains("boot failed")
                || body.contains("no bootable device")
            {
                passed = false;
            }
        }

        self.emit(EngineEvent::info(
            EventPhase::Complete,
            format!("test run completed (passed={passed})"),
        ));

        Ok(TestResult {
            bios,
            uefi,
            logs,
            passed,
        })
    }
}

async fn run_qemu_smoke(iso: &Path, firmware: Option<&Path>, log_path: &Path) -> EngineResult<()> {
    let mut args = vec![
        "-m".to_string(),
        "2048".to_string(),
        "-boot".to_string(),
        "d".to_string(),
        "-cdrom".to_string(),
        iso.display().to_string(),
        "-display".to_string(),
        "none".to_string(),
        "-serial".to_string(),
        format!("file:{}", log_path.display()),
        "-monitor".to_string(),
        "none".to_string(),
        "-no-reboot".to_string(),
    ];
    if let Some(path) = firmware {
        args.push("-bios".to_string());
        args.push(path.display().to_string());
    }

    let mut child = Command::new("qemu-system-x86_64")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| EngineError::Runtime(format!("failed to start qemu-system-x86_64: {e}")))?;

    match tokio::time::timeout(Duration::from_secs(30), child.wait()).await {
        Ok(status) => {
            let status =
                status.map_err(|e| EngineError::Runtime(format!("qemu wait failed: {e}")))?;
            if !status.success() {
                return Err(EngineError::Runtime(format!(
                    "qemu exited before smoke timeout with status {:?}",
                    status.code()
                )));
            }
        }
        Err(_) => {
            child.kill().await.ok();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scan_with_default_policy_writes_report_json() {
        let target = tempfile::tempdir().expect("target");
        std::fs::write(target.path().join("hello.txt"), b"world").expect("write");
        let out = tempfile::tempdir().expect("out");
        let engine = ForgeIsoEngine::new();

        let result = engine
            .scan(target.path(), None, out.path())
            .await
            .expect("scan must succeed in default policy mode");
        assert!(
            result.report_json.exists(),
            "scan-report.json must be written"
        );
        // Default policy enables sbom + secrets, both hermetic.
        assert!(result.report.sbom_spdx.is_some());
    }

    #[tokio::test]
    async fn scan_with_inline_policy_file_overrides_defaults() {
        let target = tempfile::tempdir().expect("target");
        let out = tempfile::tempdir().expect("out");
        let policy_dir = tempfile::tempdir().expect("policy");
        let policy_file = policy_dir.path().join("policy.yaml");
        // Disable everything → no reports, no SBOM
        std::fs::write(
            &policy_file,
            "enable_sbom: false\n\
             enable_trivy: false\n\
             enable_syft_grype: false\n\
             enable_open_scap: false\n\
             enable_secrets_scan: false\n\
             strict_secrets: false\n",
        )
        .expect("write");
        let engine = ForgeIsoEngine::new();

        let result = engine
            .scan(target.path(), Some(&policy_file), out.path())
            .await
            .expect("scan with inline policy");
        assert!(result.report.reports.is_empty());
        assert!(result.report.sbom_spdx.is_none());
    }

    #[tokio::test]
    async fn test_iso_returns_error_for_missing_file() {
        // qemu-system-x86_64 must be present for this branch to fire; if not,
        // we still get an error (MissingTool) — both are acceptable failures.
        let out = tempfile::tempdir().expect("out");
        let engine = ForgeIsoEngine::new();
        let result = engine
            .test_iso(
                std::path::Path::new("/nonexistent/path.iso"),
                true,
                false,
                out.path(),
            )
            .await;
        assert!(result.is_err(), "must return some error for missing ISO");
    }
}
