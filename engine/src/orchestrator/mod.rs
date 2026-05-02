//! [`ForgeIsoEngine`] — the central orchestrator that ties every other
//! engine module together.
//!
//! Front-ends create a single [`ForgeIsoEngine`] and hold it for the
//! lifetime of the session. It is `Clone` (cheap, internally
//! `Arc`-backed) so handlers can be parked on multiple async tasks.
//!
//! The orchestrator owns four pipelines:
//!
//! 1. [`build`] — extract source ISO → inject autoinstall / kickstart /
//!    preseed → repack with `xorriso` and `mksquashfs`. The
//!    cancellable variant ([`ForgeIsoEngine::build_cancellable`]) plumbs
//!    a [`tokio_util::sync::CancellationToken`] through every shell-out.
//! 2. [`verify`] — SHA-256 check, ISO 9660 compliance audit,
//!    boot-record verification.
//! 3. [`scan_test`] — security scanning + boot smoke tests.
//! 4. [`diff`] — structural diff between two ISOs (debugging /
//!    regression isolation).
//!
//! Per-tool [`circuit_breaker::CircuitBreaker`] guards prevent runaway
//! retries when an external tool (e.g. `mksquashfs`) starts failing —
//! see
//! [`ADR 0008`](https://github.com/Al-Sarraf-Tech/ForgeISO/blob/main/docs/adr/0008-reliability-contract-desktop-tool.md)
//! and
//! [`ADR 0012`](https://github.com/Al-Sarraf-Tech/ForgeISO/blob/main/docs/adr/0012-cancellation-and-circuit-breakers.md).

/// ISO construction pipeline: extract source ISO, inject configs, repack with xorriso/mksquashfs.
pub mod build;
pub mod circuit_breaker;
mod diff;
mod doctor;
/// Shared utility functions used by orchestrator sub-modules.
pub mod helpers;
mod inject;
mod report;
mod scan_test;
/// SHA-256 verification, ISO-9660 compliance checking, and expected-hash gating.
pub mod verify;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;

use crate::config::IsoSource;
use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};
use crate::iso::{inspect_iso, IsoMetadata, ResolvedIso, SourceKind};
use crate::report::TestSummary;

use helpers::download_filename;

// ── Public result types ──────────────────────────────────────────────────────

/// Structured output of [`ForgeIsoEngine::doctor`]: host environment summary
/// and per-distro tooling readiness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    /// Operating system name as reported by `std::env::consts::OS` (e.g. `"linux"`).
    pub host_os: String,
    /// CPU architecture as reported by `std::env::consts::ARCH` (e.g. `"x86_64"`).
    pub host_arch: String,
    /// True when the host OS is Linux; false means build/test flows are unsupported.
    pub linux_supported: bool,
    /// Map of well-known tool names to their presence (`true` = found on `PATH`).
    pub tooling: BTreeMap<String, bool>,
    /// Advisory messages describing missing tools or unsupported host configurations.
    pub warnings: Vec<String>,
    /// RFC 3339 timestamp when the doctor check was performed.
    pub timestamp: String,
    /// Per-distro inject readiness — keys: ubuntu, fedora, mint, arch, scan, test.
    pub distro_readiness: BTreeMap<String, bool>,
}

/// Materialised result of a [`ForgeIsoEngine::build`] or
/// [`ForgeIsoEngine::inject_autoinstall`] invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    /// Root of the temporary workspace directory created for this build.
    pub workspace_root: PathBuf,
    /// Directory where the final ISO and report files are written.
    pub output_dir: PathBuf,
    /// Path to the JSON build report written to `output_dir`.
    pub report_json: PathBuf,
    /// Path to the HTML build report written to `output_dir`.
    pub report_html: PathBuf,
    /// All output artifacts produced by the build (typically the repacked `.iso`).
    pub artifacts: Vec<PathBuf>,
    /// Metadata of the final output ISO (distro, release, SHA-256, etc.).
    pub iso: IsoMetadata,
    /// Resolved local path of the *input* ISO used for this operation.
    /// Always a local filesystem path (URLs are resolved/downloaded before use).
    pub source_iso: PathBuf,
}

/// Result of a [`ForgeIsoEngine::scan`] invocation: the scan summary and the
/// path of the JSON report written to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    /// Aggregated scan findings including SBOM, vulnerability, and secrets reports.
    pub report: crate::scanner::ScanSummary,
    /// Path to the `scan-report.json` file written in the caller-supplied output directory.
    pub report_json: PathBuf,
}

/// Result of a [`ForgeIsoEngine::test_iso`] boot smoke-test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// True when the BIOS smoke test was requested and completed successfully.
    pub bios: bool,
    /// True when the UEFI smoke test was requested and completed successfully.
    pub uefi: bool,
    /// Paths of the serial-log files captured from each QEMU boot attempt.
    pub logs: Vec<PathBuf>,
    /// Overall pass/fail — false when any log is empty or contains known boot-failure strings.
    pub passed: bool,
}

/// Result of a [`ForgeIsoEngine::verify`] checksum verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    /// Basename of the ISO file that was verified.
    pub filename: String,
    /// Expected SHA-256 hex digest from the upstream `SHA256SUMS` file, or an
    /// explanatory message when no upstream source was available.
    pub expected: String,
    /// Actual SHA-256 hex digest computed over the local file.
    pub actual: String,
    /// True when `expected` and `actual` are identical hex digests.
    pub matched: bool,
}

/// A single file that differs between the base and target ISOs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    /// Absolute path of the file within the ISO filesystem (e.g. `/EFI/boot/bootx64.efi`).
    pub path: String,
    /// Size of the file in the base ISO, in bytes.
    pub base_size: u64,
    /// Size of the file in the target ISO, in bytes.
    pub target_size: u64,
}

/// Structural diff between two ISOs produced by [`ForgeIsoEngine::diff_isos`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsoDiff {
    /// Paths present in the target ISO but absent from the base ISO.
    pub added: Vec<String>,
    /// Paths present in the base ISO but absent from the target ISO.
    pub removed: Vec<String>,
    /// Files present in both ISOs whose sizes differ.
    pub modified: Vec<DiffEntry>,
    /// Number of files with identical paths and sizes in both ISOs.
    pub unchanged: usize,
}

/// ISO-9660 compliance check result.
/// `compliant` is true only when the CD001 primary volume descriptor signature
/// is confirmed at the standard sector-16 offset. El Torito boot presence is
/// checked via xorriso when available.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iso9660Compliance {
    /// True if the CD001 ISO-9660 signature was found at sector 16.
    pub compliant: bool,
    /// Primary volume descriptor volume ID label (may be None if empty).
    pub volume_id: Option<String>,
    /// File size in bytes.
    pub size_bytes: u64,
    /// El Torito BIOS boot entry detected (requires xorriso).
    pub boot_bios: bool,
    /// El Torito UEFI boot entry detected (requires xorriso).
    pub boot_uefi: bool,
    /// Any El Torito boot catalog present.
    pub el_torito_present: bool,
    /// Method used: "iso9660_header" or "iso9660_header+xorriso".
    pub check_method: String,
    /// Error message if the check failed (compliant will be false).
    pub error: Option<String>,
}

/// Captured output from a subprocess invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    /// Name of the program that was executed (e.g. `"xorriso"`).
    pub program: String,
    /// Exit status code; 0 indicates success.
    pub status: i32,
    /// Captured standard output decoded as lossy UTF-8.
    pub stdout: String,
    /// Captured standard error decoded as lossy UTF-8.
    pub stderr: String,
}

// ── ForgeIsoEngine struct + core methods ─────────────────────────────────────

/// Central orchestrator that ties every engine pipeline together.
///
/// Front-ends construct a single `ForgeIsoEngine` and hold it for the
/// lifetime of the session. The type is `Clone` (cheap — the internal event
/// bus is `Arc`-backed) so handlers can be distributed across async tasks.
#[derive(Clone)]
pub struct ForgeIsoEngine {
    events: broadcast::Sender<EngineEvent>,
}

impl Default for ForgeIsoEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeIsoEngine {
    /// Construct a new engine with an internal event broadcast channel
    /// of capacity 2048.
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(2048);
        Self { events }
    }

    /// Subscribe to the engine event stream; returns a new [`broadcast::Receiver`]
    /// that receives all events emitted from this point forward.
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    pub(crate) fn emit(&self, event: EngineEvent) {
        let _ = self.events.send(event);
    }

    /// Resolve and inspect an ISO source, returning its metadata.
    ///
    /// `source` may be a local filesystem path or an HTTP(S) URL. When a URL
    /// is supplied and `cache_dir` is `None`, the default XDG cache directory
    /// is used. Returns an error if the source cannot be found or read.
    pub async fn inspect_source(
        &self,
        source: &str,
        cache_dir: Option<&Path>,
    ) -> EngineResult<IsoMetadata> {
        self.emit(EngineEvent::info(
            EventPhase::Inspect,
            format!("resolving ISO source {source}"),
        ));
        let owned_cache_root;
        let cache_root = if let Some(cache_dir) = cache_dir {
            cache_dir
        } else {
            owned_cache_root = default_cache_root()?;
            owned_cache_root.as_path()
        };
        let resolved = self
            .resolve_source(&IsoSource::from_raw(source.to_string()), cache_root)
            .await?;
        let metadata = inspect_iso(
            &resolved.source_path,
            resolved.source_kind,
            resolved.source_value,
        )?;
        self.emit(EngineEvent::info(
            EventPhase::Inspect,
            format!(
                "inspection complete: distro={} release={} arch={}",
                metadata
                    .distro
                    .map(|value| format!("{:?}", value))
                    .unwrap_or_else(|| "unknown".to_string()),
                metadata.release.as_deref().unwrap_or("unknown"),
                metadata.architecture.as_deref().unwrap_or("unknown")
            ),
        ));
        self.emit(EngineEvent::info(
            EventPhase::Complete,
            "source inspection completed",
        ));
        Ok(metadata)
    }

    pub(crate) async fn resolve_source(
        &self,
        source: &IsoSource,
        cache_root: &Path,
    ) -> EngineResult<ResolvedIso> {
        match source {
            IsoSource::Path(path) => {
                if !path.exists() {
                    return Err(EngineError::NotFound(format!(
                        "source ISO does not exist: {}",
                        path.display()
                    )));
                }
                Ok(ResolvedIso {
                    source_path: path.to_path_buf(),
                    source_kind: SourceKind::LocalPath,
                    source_value: path.display().to_string(),
                    _download_dir: None,
                })
            }
            IsoSource::Url(url) => {
                std::fs::create_dir_all(cache_root)?;
                let target = cache_root.join(download_filename(url));

                // Cache-hit: skip re-downloading if the file already exists.
                // Warn when the cached file is older than 7 days — the distro
                // may have released a security update since it was cached.
                if target.exists() {
                    const CACHE_TTL_DAYS: u64 = 7;
                    let age_days = std::fs::metadata(&target)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .map(|d| d.as_secs() / 86_400)
                        .unwrap_or(0);
                    if age_days >= CACHE_TTL_DAYS {
                        self.emit(EngineEvent::warn(
                            EventPhase::Download,
                            format!(
                                "cached ISO is {age_days} days old (>{CACHE_TTL_DAYS}d); \
                                 the distro may have released security updates. \
                                 Delete {} to force a fresh download.",
                                target.display()
                            ),
                        ));
                    } else {
                        self.emit(EngineEvent::info(
                            EventPhase::Download,
                            format!("using cached ISO ({age_days}d old): {}", target.display()),
                        ));
                    }
                    return Ok(ResolvedIso {
                        source_path: target.clone(),
                        source_kind: SourceKind::DownloadedUrl,
                        source_value: url.clone(),
                        _download_dir: Some(target),
                    });
                }

                self.emit(EngineEvent::info(
                    EventPhase::Download,
                    format!("downloading source ISO from {url}"),
                ));
                self.download_to_path(url, &target).await?;
                Ok(ResolvedIso {
                    source_path: target.clone(),
                    source_kind: SourceKind::DownloadedUrl,
                    source_value: url.clone(),
                    _download_dir: Some(target),
                })
            }
        }
    }

    async fn download_to_path(&self, url: &str, output: &Path) -> EngineResult<()> {
        const MAX_ATTEMPTS: u32 = 3;
        let mut last_err: Option<EngineError> = None;
        for attempt in 0..MAX_ATTEMPTS {
            if attempt > 0 {
                let delay_secs = 1u64 << (attempt - 1); // 1s, 2s
                self.emit(EngineEvent::warn(
                    EventPhase::Download,
                    format!(
                        "download attempt {} failed; retrying in {}s — {}",
                        attempt, delay_secs, url
                    ),
                ));
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            }
            match self.download_attempt(url, output).await {
                Ok(()) => return Ok(()),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            EngineError::Network(format!(
                "download failed after {MAX_ATTEMPTS} attempts: {url}"
            ))
        }))
    }

    async fn download_attempt(&self, url: &str, output: &Path) -> EngineResult<()> {
        let response = reqwest::get(url).await?;
        if !response.status().is_success() {
            return Err(EngineError::Network(format!(
                "download failed with status {}",
                response.status()
            )));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut file = tokio::fs::File::create(output).await?;
        let mut response = response;
        let mut downloaded = 0u64;
        let emit_interval = 512 * 1024; // 512 KB
        let mut next_emit = emit_interval;

        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if total_size > 0 && (downloaded >= next_emit || downloaded == total_size) {
                let msg = format!("{}/{} bytes", downloaded, total_size);
                self.emit(
                    EngineEvent::info(EventPhase::Download, msg).with_bytes(downloaded, total_size),
                );
                while next_emit <= downloaded {
                    next_emit = next_emit.saturating_add(emit_interval);
                }
            }
        }
        file.flush().await?;
        Ok(())
    }
}

impl From<TestResult> for TestSummary {
    fn from(value: TestResult) -> Self {
        Self {
            bios: value.bios,
            uefi: value.uefi,
            logs: value.logs.iter().map(|p| p.display().to_string()).collect(),
            passed: value.passed,
        }
    }
}

// ── Re-exports ───────────────────────────────────────────────────────────────
// Public functions that are used by other engine modules via crate::orchestrator::
pub use helpers::{cache_subdir, default_cache_root, run_command_capture, run_command_lossy};
pub use verify::sha256_file;
