use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, EngineResult};

use super::{IsoSource, ProfileKind};

const fn default_true() -> bool {
    true
}

/// Controls which security and SBOM scanning tools are invoked after the ISO is built.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ScanPolicy {
    /// Generate a Software Bill of Materials (SBOM) from the ISO filesystem. Enabled by default.
    #[serde(default = "default_true")]
    pub enable_sbom: bool,
    /// Run Trivy vulnerability scanning against the ISO contents. Enabled by default.
    #[serde(default = "default_true")]
    pub enable_trivy: bool,
    /// Run Syft + Grype for additional SBOM generation and vulnerability matching.
    #[serde(default)]
    pub enable_syft_grype: bool,
    /// Run OpenSCAP compliance checks against the ISO filesystem.
    #[serde(default)]
    pub enable_open_scap: bool,
    /// Scan for accidentally embedded secrets (keys, tokens, credentials). Enabled by default.
    #[serde(default = "default_true")]
    pub enable_secrets_scan: bool,
    /// Treat secrets-scan findings as hard errors, failing the build immediately.
    #[serde(default)]
    pub strict_secrets: bool,
}

impl Default for ScanPolicy {
    fn default() -> Self {
        Self {
            enable_sbom: default_true(),
            enable_trivy: default_true(),
            enable_syft_grype: false,
            enable_open_scap: false,
            enable_secrets_scan: default_true(),
            strict_secrets: false,
        }
    }
}

/// Controls which boot-mode and smoke tests are executed against the finished ISO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestingPolicy {
    /// Boot-test the ISO in BIOS (legacy) mode via QEMU. Enabled by default.
    #[serde(default = "default_true")]
    pub bios: bool,
    /// Boot-test the ISO in UEFI mode via QEMU + OVMF. Enabled by default.
    #[serde(default = "default_true")]
    pub uefi: bool,
    /// Run a quick installer smoke-test to confirm the ISO reaches the installer prompt.
    #[serde(default = "default_true")]
    pub smoke: bool,
}

impl Default for TestingPolicy {
    fn default() -> Self {
        Self {
            bios: true,
            uefi: true,
            smoke: true,
        }
    }
}

/// Top-level configuration that drives a complete ForgeISO build.
///
/// Specifies the source ISO, optional injection overlay, output label, scanning
/// and testing policies, and the YAML file from which an [`InjectConfig`](super::InjectConfig)
/// can be constructed. Load from YAML with [`BuildConfig::from_yaml_str`] or
/// [`BuildConfig::from_path`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Human-readable name for this build configuration, used in logs and output filenames.
    pub name: String,
    /// Source ISO location — a local path or an HTTP(S) URL.
    pub source: IsoSource,
    /// Optional directory whose contents are overlaid onto the ISO root before repack.
    #[serde(default)]
    pub overlay_dir: Option<PathBuf>,
    /// Volume label written to the output ISO. Must be ≤32 ASCII characters with no control chars.
    #[serde(default)]
    pub output_label: Option<String>,
    /// Software profile controlling which package set is injected. Defaults to `Minimal`.
    #[serde(default = "default_profile")]
    pub profile: ProfileKind,
    /// When `true`, automatically run the full scanner suite after a successful build.
    #[serde(default)]
    pub auto_scan: bool,
    /// When `true`, automatically boot-test the finished ISO. Requires `testing.smoke = true`.
    #[serde(default)]
    pub auto_test: bool,
    /// Fine-grained control over which scanning tools are executed.
    #[serde(default)]
    pub scanning: ScanPolicy,
    /// Fine-grained control over which boot-mode tests are executed.
    #[serde(default)]
    pub testing: TestingPolicy,
    /// Preserve the temporary work directory after build for post-mortem inspection.
    #[serde(default)]
    pub keep_workdir: bool,
    /// If set, the downloaded ISO's SHA-256 must match before any operation proceeds.
    #[serde(default)]
    pub expected_sha256: Option<String>,
}

const fn default_profile() -> ProfileKind {
    ProfileKind::Minimal
}

impl BuildConfig {
    /// # Errors
    /// Returns an error if the YAML is invalid or fails validation.
    pub fn from_yaml_str(raw: &str) -> EngineResult<Self> {
        let cfg: Self = serde_yaml::from_str(raw)?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// # Errors
    /// Returns an error if the file cannot be read or the YAML is invalid.
    pub fn from_path(path: &Path) -> EngineResult<Self> {
        let raw = std::fs::read_to_string(path)?;
        Self::from_yaml_str(&raw)
    }

    /// # Errors
    /// Returns an error if any required field is missing or invalid.
    pub fn validate(&self) -> EngineResult<()> {
        if self.name.trim().is_empty() {
            return Err(EngineError::InvalidConfig(
                "name cannot be empty".to_string(),
            ));
        }

        match &self.source {
            IsoSource::Path(path) => {
                if path.as_os_str().is_empty() {
                    return Err(EngineError::InvalidConfig(
                        "source path cannot be empty".to_string(),
                    ));
                }
            }
            IsoSource::Url(url) => {
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return Err(EngineError::InvalidConfig(
                        "source URL must start with http:// or https://".to_string(),
                    ));
                }
            }
        }

        if let Some(path) = &self.overlay_dir {
            if !path.exists() {
                return Err(EngineError::InvalidConfig(format!(
                    "overlay_dir does not exist: {}",
                    path.display()
                )));
            }
            if !path.is_dir() {
                return Err(EngineError::InvalidConfig(format!(
                    "overlay_dir must be a directory: {}",
                    path.display()
                )));
            }
        }

        if let Some(label) = &self.output_label {
            if label.trim().is_empty() {
                return Err(EngineError::InvalidConfig(
                    "output_label cannot be blank".to_string(),
                ));
            }
            if label.len() > 32 {
                return Err(EngineError::InvalidConfig(
                    "output_label must be 32 characters or fewer".to_string(),
                ));
            }
            if !label.is_ascii() {
                return Err(EngineError::InvalidConfig(
                    "output_label must contain only ASCII characters".to_string(),
                ));
            }
            if label.chars().any(|c| c.is_ascii_control()) {
                return Err(EngineError::InvalidConfig(
                    "output_label must not contain control characters".to_string(),
                ));
            }
        }

        if self.auto_test && !self.testing.smoke {
            return Err(EngineError::InvalidConfig(
                "auto_test requires testing.smoke=true".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_overlay_dir() {
        let cfg = BuildConfig {
            name: "demo".to_string(),
            source: IsoSource::from_raw("/tmp/base.iso"),
            overlay_dir: Some(PathBuf::from("/definitely/missing")),
            output_label: None,
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };

        assert!(cfg.validate().is_err());
    }

    #[test]
    fn scan_policy_defaults_enable_local_checks() {
        let policy = ScanPolicy::default();

        assert!(policy.enable_sbom);
        assert!(policy.enable_trivy);
        assert!(policy.enable_secrets_scan);
        assert!(!policy.enable_syft_grype);
        assert!(!policy.enable_open_scap);
    }

    // -- BuildConfig validation -------------------------------------------------

    #[test]
    fn build_config_from_yaml_str_minimal() {
        // IsoSource is #[serde(untagged)] -- deserializes from bare string
        let yaml = "name: test-build\nsource: /tmp/ubuntu.iso\n";
        let result = BuildConfig::from_yaml_str(yaml);
        assert!(result.is_ok(), "parse failed: {result:?}");
    }

    #[test]
    fn build_config_rejects_empty_name() {
        let yaml = "name: ''\nsource: /tmp/ubuntu.iso\n";
        let result = BuildConfig::from_yaml_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn build_config_rejects_whitespace_only_name() {
        let yaml = "name: '   '\nsource: /tmp/ubuntu.iso\n";
        let result = BuildConfig::from_yaml_str(yaml);
        assert!(result.is_err(), "whitespace-only name must be rejected");
    }

    #[test]
    fn build_config_rejects_blank_output_label() {
        let cfg = BuildConfig {
            name: "build".to_string(),
            source: IsoSource::from_raw("/tmp/test.iso"),
            overlay_dir: None,
            output_label: Some("   ".to_string()), // blank after trim
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };
        assert!(
            cfg.validate().is_err(),
            "blank output_label must be rejected"
        );
    }

    #[test]
    fn build_config_rejects_output_label_too_long() {
        let cfg = BuildConfig {
            name: "build".to_string(),
            source: IsoSource::from_raw("/tmp/test.iso"),
            overlay_dir: None,
            output_label: Some("A".repeat(33)), // 33 chars > 32 max
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };
        assert!(
            cfg.validate().is_err(),
            "output_label longer than 32 chars must be rejected"
        );
    }

    #[test]
    fn build_config_accepts_output_label_exactly_32_chars() {
        let cfg = BuildConfig {
            name: "build".to_string(),
            source: IsoSource::from_raw("/tmp/test.iso"),
            overlay_dir: None,
            output_label: Some("A".repeat(32)),
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };
        assert!(
            cfg.validate().is_ok(),
            "32-char output_label must be accepted"
        );
    }

    #[test]
    fn build_config_rejects_output_label_with_control_char() {
        for bad in &[
            "LABEL\nINJECT",
            "LABEL\rINJECT",
            "LABEL\0INJECT",
            "LABEL\tINJECT",
        ] {
            let cfg = BuildConfig {
                name: "build".to_string(),
                source: IsoSource::from_raw("/tmp/test.iso"),
                overlay_dir: None,
                output_label: Some((*bad).to_string()),
                profile: ProfileKind::Minimal,
                auto_scan: false,
                auto_test: false,
                scanning: ScanPolicy::default(),
                testing: TestingPolicy::default(),
                keep_workdir: false,
                expected_sha256: None,
            };
            assert!(
                cfg.validate().is_err(),
                "output_label {:?} with control char must be rejected",
                bad
            );
        }
    }

    #[test]
    fn build_config_from_path_round_trips_through_yaml() {
        let dir = tempfile::tempdir().expect("dir");
        let p = dir.path().join("build.yaml");
        std::fs::write(&p, "name: from-path-build\nsource: /tmp/x.iso\n").expect("write");
        let cfg = BuildConfig::from_path(&p).expect("from_path must parse");
        assert_eq!(cfg.name, "from-path-build");
    }

    #[test]
    fn build_config_from_path_returns_err_for_missing_file() {
        let r = BuildConfig::from_path(std::path::Path::new(
            "/nonexistent/forgeiso-build-config.yaml",
        ));
        assert!(r.is_err(), "missing file must produce an Err");
    }

    #[test]
    fn build_config_rejects_empty_source_path() {
        let cfg = BuildConfig {
            name: "n".to_string(),
            source: IsoSource::Path(PathBuf::new()), // empty path
            overlay_dir: None,
            output_label: None,
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn build_config_rejects_url_without_http_scheme() {
        let cfg = BuildConfig {
            name: "n".to_string(),
            source: IsoSource::Url("ftp://example.com/x.iso".to_string()),
            overlay_dir: None,
            output_label: None,
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };
        let r = cfg.validate();
        assert!(matches!(r, Err(EngineError::InvalidConfig(_))));
    }

    #[test]
    fn build_config_rejects_overlay_dir_pointing_at_a_file() {
        // Path exists but is a file, not a dir -> must be rejected.
        let dir = tempfile::tempdir().expect("dir");
        let f = dir.path().join("overlay-as-file");
        std::fs::write(&f, b"not a dir").expect("write");
        let cfg = BuildConfig {
            name: "n".to_string(),
            source: IsoSource::from_raw("/tmp/x.iso"),
            overlay_dir: Some(f),
            output_label: None,
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn build_config_rejects_non_ascii_output_label() {
        let cfg = BuildConfig {
            name: "n".to_string(),
            source: IsoSource::from_raw("/tmp/x.iso"),
            overlay_dir: None,
            output_label: Some("LÅBEL".to_string()), // non-ASCII Å
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
            expected_sha256: None,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn build_config_rejects_auto_test_without_smoke() {
        let testing = TestingPolicy {
            smoke: false,
            ..Default::default()
        };
        let cfg = BuildConfig {
            name: "build".to_string(),
            source: IsoSource::from_raw("/tmp/test.iso"),
            overlay_dir: None,
            output_label: None,
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: true,
            scanning: ScanPolicy::default(),
            testing,
            keep_workdir: false,
            expected_sha256: None,
        };
        assert!(
            cfg.validate().is_err(),
            "auto_test=true with smoke=false must be rejected"
        );
    }
}
