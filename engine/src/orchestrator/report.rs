use std::path::{Path, PathBuf};

use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};
use crate::iso::{inspect_iso, SourceKind};
use crate::report::BuildReport;

use super::ForgeIsoEngine;

impl ForgeIsoEngine {
    /// Re-render an existing `build-report.json` from `build_dir` into the
    /// requested `format` (`"json"` or `"html"`), returning the output path.
    ///
    /// Returns [`EngineError::InvalidConfig`] for unsupported format strings.
    #[allow(clippy::unused_async)] // async kept for API consistency
    pub async fn report(&self, build_dir: &Path, format: &str) -> EngineResult<PathBuf> {
        let input = build_dir.join("build-report.json");
        let raw = std::fs::read_to_string(&input)?;
        let report: BuildReport = serde_json::from_str(&raw)?;
        let output = match format {
            "json" => {
                let path = build_dir.join("report.json");
                std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
                path
            }
            "html" => {
                let path = build_dir.join("report.html");
                report.write_html(&path)?;
                path
            }
            other => {
                return Err(EngineError::InvalidConfig(format!(
                    "unsupported format: {other}"
                )))
            }
        };
        self.emit(EngineEvent::info(
            EventPhase::Report,
            format!("report rendered to {}", output.display()),
        ));
        self.emit(EngineEvent::info(
            EventPhase::Complete,
            "report generation completed",
        ));
        Ok(output)
    }

    /// Inspect a local ISO file and return its metadata as a JSON value.
    ///
    /// Wraps [`crate::iso::inspect_iso`]; the result is serialised to
    /// `serde_json::Value` for front-end consumption.
    #[allow(clippy::unused_async)] // async kept for API consistency
    pub async fn inspect_iso(&self, iso: &Path) -> EngineResult<serde_json::Value> {
        let metadata = inspect_iso(iso, SourceKind::LocalPath, iso.display().to_string())?;
        serde_json::to_value(metadata).map_err(EngineError::from)
    }
}
