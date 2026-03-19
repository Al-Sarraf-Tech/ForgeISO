pub mod build;
mod diff;
mod doctor;
pub mod helpers;
mod inject;
mod report;
mod scan_test;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub host_os: String,
    pub host_arch: String,
    pub linux_supported: bool,
    pub tooling: BTreeMap<String, bool>,
    pub warnings: Vec<String>,
    pub timestamp: String,
    /// Per-distro inject readiness — keys: ubuntu, fedora, mint, arch, scan, test.
    pub distro_readiness: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub workspace_root: PathBuf,
    pub output_dir: PathBuf,
    pub report_json: PathBuf,
    pub report_html: PathBuf,
    pub artifacts: Vec<PathBuf>,
    pub iso: IsoMetadata,
    /// Resolved local path of the *input* ISO used for this operation.
    /// Always a local filesystem path (URLs are resolved/downloaded before use).
    pub source_iso: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub report: crate::scanner::ScanSummary,
    pub report_json: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub bios: bool,
    pub uefi: bool,
    pub logs: Vec<PathBuf>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub filename: String,
    pub expected: String,
    pub actual: String,
    pub matched: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub path: String,
    pub base_size: u64,
    pub target_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsoDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<DiffEntry>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub program: String,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

// ── ForgeIsoEngine struct + core methods ─────────────────────────────────────

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
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(2048);
        Self { events }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    pub(crate) fn emit(&self, event: EngineEvent) {
        let _ = self.events.send(event);
    }

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
