use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    config::{BuildConfig, ProfileKind},
    error::EngineResult,
    iso::IsoMetadata,
    scanner::ScanSummary,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildMetadata {
    pub generated_at: DateTime<Utc>,
    pub tool_name: String,
    pub tool_version: String,
    pub profile: ProfileKind,
    pub source: String,
    pub source_sha256: String,
    pub detected_distro: Option<String>,
    pub detected_release: Option<String>,
    pub detected_architecture: Option<String>,
    pub volume_id: Option<String>,
    pub output_label: Option<String>,
    pub warnings: Vec<String>,
    pub tool_versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSummary {
    pub bios: bool,
    pub uefi: bool,
    pub logs: Vec<String>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReport {
    pub metadata: BuildMetadata,
    pub artifacts: Vec<String>,
    pub scan_summary: Option<ScanSummary>,
    pub test_summary: Option<TestSummary>,
}

impl BuildReport {
    pub fn new(cfg: &BuildConfig, iso: &IsoMetadata) -> Self {
        Self {
            metadata: BuildMetadata {
                generated_at: Utc::now(),
                tool_name: "forgeiso".to_string(),
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                profile: cfg.profile,
                source: cfg.source.display_value(),
                source_sha256: iso.sha256.clone(),
                detected_distro: iso.distro.map(|value| format!("{:?}", value)),
                detected_release: iso.release.clone(),
                detected_architecture: iso.architecture.clone(),
                volume_id: iso.volume_id.clone(),
                output_label: cfg.output_label.clone(),
                warnings: iso.warnings.clone(),
                tool_versions: BTreeMap::new(),
            },
            artifacts: Vec::new(),
            scan_summary: None,
            test_summary: None,
        }
    }

    pub fn write_json(&self, out: &Path) -> EngineResult<()> {
        std::fs::write(out, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn write_html(&self, out: &Path) -> EngineResult<()> {
        let artifact_items = self
            .artifacts
            .iter()
            .map(|a| format!("<li>{}</li>", html_escape(a)))
            .collect::<Vec<_>>()
            .join("\n");

        let warning_items = self
            .metadata
            .warnings
            .iter()
            .map(|a| format!("<li>{}</li>", html_escape(a)))
            .collect::<Vec<_>>()
            .join("\n");

        let body = format!(
            "<!doctype html><html><head><meta charset='utf-8'><title>ForgeISO Report</title><style>body{{font-family:Inter,Segoe UI,Arial,sans-serif;background:#0f172a;color:#e2e8f0;padding:24px;line-height:1.55}}section{{background:#111827;border:1px solid #334155;border-radius:12px;padding:16px;margin:12px 0}}h1,h2{{margin:0 0 12px}}ul{{margin:8px 0 0 18px}}code{{background:#020617;padding:2px 6px;border-radius:6px}}</style></head><body><h1>ForgeISO Local Build Report</h1><section><h2>Source</h2><p><b>Input:</b> <code>{source}</code></p><p><b>SHA-256:</b> <code>{sha}</code></p><p><b>Distro:</b> {distro}</p><p><b>Release:</b> {release}</p><p><b>Architecture:</b> {arch}</p><p><b>Volume ID:</b> {volume}</p></section><section><h2>Artifacts</h2><ul>{artifact_items}</ul></section><section><h2>Warnings</h2><ul>{warning_items}</ul></section></body></html>",
            source = html_escape(&self.metadata.source),
            sha = html_escape(&self.metadata.source_sha256),
            distro = html_escape(self.metadata.detected_distro.as_deref().unwrap_or("unknown")),
            release = html_escape(self.metadata.detected_release.as_deref().unwrap_or("unknown")),
            arch = html_escape(self.metadata.detected_architecture.as_deref().unwrap_or("unknown")),
            volume = html_escape(self.metadata.volume_id.as_deref().unwrap_or("unknown")),
            artifact_items = artifact_items,
            warning_items = warning_items,
        );

        std::fs::write(out, body)?;
        Ok(())
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BuildConfig, IsoSource, ProfileKind, ScanPolicy, TestingPolicy};
    use crate::iso::{IsoMetadata, SourceKind};
    use std::path::PathBuf;

    fn minimal_build_config() -> BuildConfig {
        BuildConfig {
            name: "test-build".to_string(),
            source: IsoSource::Path(PathBuf::from("/tmp/test.iso")),
            overlay_dir: None,
            output_label: None,
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        }
    }

    fn minimal_iso_metadata() -> IsoMetadata {
        IsoMetadata {
            source_path: PathBuf::from("/tmp/test.iso"),
            source_kind: SourceKind::LocalPath,
            source_value: "/tmp/test.iso".to_string(),
            size_bytes: 1024,
            sha256: "abcdef1234567890".to_string(),
            volume_id: Some("TEST_ISO".to_string()),
            distro: None,
            release: None,
            edition: None,
            architecture: None,
            rootfs_path: None,
            boot: crate::iso::BootSupport::default(),
            inspected_at: "2026-01-01T00:00:00Z".to_string(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn html_escape_ampersand() {
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }

    #[test]
    fn html_escape_less_than() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
    }

    #[test]
    fn html_escape_double_quote() {
        assert_eq!(html_escape(r#"say "hello""#), "say &quot;hello&quot;");
    }

    #[test]
    fn html_escape_single_quote() {
        assert_eq!(html_escape("it's"), "it&#x27;s");
    }

    #[test]
    fn html_escape_no_change_for_clean_input() {
        let clean = "Ubuntu 24.04 LTS server amd64";
        assert_eq!(html_escape(clean), clean);
    }

    #[test]
    fn build_report_new_captures_source_sha256() {
        let cfg = minimal_build_config();
        let iso = minimal_iso_metadata();
        let report = BuildReport::new(&cfg, &iso);
        assert_eq!(report.metadata.source_sha256, "abcdef1234567890");
    }

    #[test]
    fn build_report_new_captures_volume_id() {
        let cfg = minimal_build_config();
        let iso = minimal_iso_metadata();
        let report = BuildReport::new(&cfg, &iso);
        assert_eq!(report.metadata.volume_id, Some("TEST_ISO".to_string()));
    }

    #[test]
    fn build_report_write_json_produces_valid_json() {
        let tmp = tempfile::NamedTempFile::new().expect("tmp file");
        let cfg = minimal_build_config();
        let iso = minimal_iso_metadata();
        let mut report = BuildReport::new(&cfg, &iso);
        report.artifacts.push("/tmp/out.iso".to_string());
        report.write_json(tmp.path()).expect("write_json");
        let content = std::fs::read_to_string(tmp.path()).expect("read json");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("must be valid JSON");
        assert_eq!(
            parsed["artifacts"][0].as_str(),
            Some("/tmp/out.iso"),
            "artifact path must round-trip through JSON"
        );
    }

    #[test]
    fn build_report_write_html_contains_sha256() {
        let tmp = tempfile::NamedTempFile::new().expect("tmp file");
        let cfg = minimal_build_config();
        let iso = minimal_iso_metadata();
        let report = BuildReport::new(&cfg, &iso);
        report.write_html(tmp.path()).expect("write_html");
        let content = std::fs::read_to_string(tmp.path()).expect("read html");
        assert!(
            content.contains("abcdef1234567890"),
            "HTML must contain the SHA-256 hash"
        );
        assert!(
            content.contains("ForgeISO Report"),
            "HTML must contain the report title"
        );
    }

    #[test]
    fn build_report_write_html_escapes_xss_in_source() {
        let tmp = tempfile::NamedTempFile::new().expect("tmp file");
        let mut cfg = minimal_build_config();
        cfg.source = IsoSource::Url("<script>alert(1)</script>".to_string());
        let iso = minimal_iso_metadata();
        let report = BuildReport::new(&cfg, &iso);
        report.write_html(tmp.path()).expect("write_html");
        let content = std::fs::read_to_string(tmp.path()).expect("read html");
        assert!(
            !content.contains("<script>alert(1)</script>"),
            "raw XSS payload must not appear in HTML output"
        );
        assert!(
            content.contains("&lt;script&gt;"),
            "XSS payload must be HTML-escaped"
        );
    }

    #[test]
    fn build_report_warnings_appear_in_html() {
        let tmp = tempfile::NamedTempFile::new().expect("tmp file");
        let cfg = minimal_build_config();
        let mut iso = minimal_iso_metadata();
        iso.warnings
            .push("squashfs not found; rootfs not modified".to_string());
        let report = BuildReport::new(&cfg, &iso);
        report.write_html(tmp.path()).expect("write_html");
        let content = std::fs::read_to_string(tmp.path()).expect("read html");
        assert!(
            content.contains("squashfs not found"),
            "warnings must be rendered in HTML output"
        );
    }
}
