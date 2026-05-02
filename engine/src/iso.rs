//! ISO inspection and metadata extraction.
//!
//! Functions in this module read a source ISO without modifying it and
//! produce an [`IsoMetadata`] record describing what was found:
//!
//! - the [`SourceKind`] discriminator (HTTP URL, preset id, local path)
//! - distro detection (Ubuntu / Fedora / Arch / Mint / Debian / RHEL family)
//! - boot-mode capability ([`BootSupport`] — BIOS, UEFI, both, or
//!   neither)
//! - volume label, manifest paths, and any per-distro signature files
//!
//! Inspection runs as the first step of [`crate::ForgeIsoEngine::build`]
//! so the rest of the pipeline can branch on real ISO contents rather
//! than the user's claim about what the ISO is.
//!
//! ISO repacking (the inverse direction — apply changes and emit a new
//! `.iso`) lives in [`crate::orchestrator::build`].

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    config::Distro,
    error::{EngineError, EngineResult},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    LocalPath,
    DownloadedUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BootSupport {
    pub bios: bool,
    pub uefi: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsoMetadata {
    pub source_path: PathBuf,
    pub source_kind: SourceKind,
    pub source_value: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub volume_id: Option<String>,
    pub distro: Option<Distro>,
    pub release: Option<String>,
    pub edition: Option<String>,
    pub architecture: Option<String>,
    pub rootfs_path: Option<String>,
    pub boot: BootSupport,
    pub inspected_at: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedIso {
    pub source_path: PathBuf,
    pub source_kind: SourceKind,
    pub source_value: String,
    pub _download_dir: Option<PathBuf>,
}

pub fn inspect_iso(
    path: &Path,
    source_kind: SourceKind,
    source_value: String,
) -> EngineResult<IsoMetadata> {
    if !path.exists() {
        return Err(EngineError::NotFound(format!(
            "ISO not found: {}",
            path.display()
        )));
    }

    let metadata = std::fs::metadata(path)?;
    let sha256 = crate::orchestrator::sha256_file(path)?;
    let volume_id = read_primary_volume_id(path)?;

    let mut info = IsoMetadata {
        source_path: path.to_path_buf(),
        source_kind,
        source_value,
        size_bytes: metadata.len(),
        sha256,
        volume_id,
        distro: None,
        release: None,
        edition: None,
        architecture: None,
        rootfs_path: None,
        boot: BootSupport::default(),
        inspected_at: chrono::Utc::now().to_rfc3339(),
        warnings: Vec::new(),
    };

    if let Some(label) = info.volume_id.clone() {
        infer_from_label(&label, &mut info);
    }

    if which::which("xorriso").is_ok() {
        enrich_with_xorriso(path, &mut info)?;
    } else {
        info.warnings.push(
            "xorriso is not installed; ISO metadata is limited until local tooling is available"
                .to_string(),
        );
    }

    Ok(info)
}

fn enrich_with_xorriso(path: &Path, info: &mut IsoMetadata) -> EngineResult<()> {
    // xorriso -report_el_torito exits non-zero on ISOs that have no El Torito
    // boot records — this is expected, not an error.  Use run_command_lossy so
    // we capture whatever output is available regardless of exit status.
    let boot_report = match crate::orchestrator::run_command_lossy(
        "xorriso",
        &[
            "-indev".to_string(),
            path.display().to_string(),
            "-report_el_torito".to_string(),
            "plain".to_string(),
        ],
        None,
    ) {
        Ok(out) => format!(
            "{}\n{}",
            out.stdout.to_lowercase(),
            out.stderr.to_lowercase()
        ),
        Err(_) => String::new(),
    };
    info.boot.bios = boot_report.contains("pltf  bios")
        || boot_report.contains("boot img :   1  bios")
        || boot_report.contains("platform id: 0x00")
        || boot_report.contains("platform id :  0 = 80x86");
    info.boot.uefi = boot_report.contains("pltf  uefi")
        || boot_report.contains("boot img :   2  uefi")
        || boot_report.contains("platform id: 0xef")
        || boot_report.contains("platform id :  0xef = efi");

    if let Some(body) = extract_optional_file(path, "/.disk/info")? {
        infer_from_disk_info(&body, info);
    }
    if let Some(body) = extract_optional_file(path, "/.treeinfo")? {
        infer_from_treeinfo(&body, info);
    }
    if let Some(body) = extract_optional_file(path, "/arch/version")? {
        infer_from_arch_version(&body, info);
    }

    if info.rootfs_path.is_none() {
        for candidate in [
            "/casper/filesystem.squashfs",
            "/live/filesystem.squashfs",
            "/LiveOS/squashfs.img",
            "/arch/x86_64/airootfs.sfs",
            "/arch/x86_64/airootfs.erofs",
        ] {
            if iso_path_exists(path, candidate)? {
                info.rootfs_path = Some(candidate.trim_start_matches('/').to_string());
                break;
            }
        }
    }

    Ok(())
}

fn extract_optional_file(path: &Path, iso_path: &str) -> EngineResult<Option<String>> {
    let tmp = crate::orchestrator::cache_subdir("extract")?
        .join(format!("forgeiso-extract-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp)?;
    let out = tmp.join("extract.txt");
    let result = crate::orchestrator::run_command_capture(
        "xorriso",
        &[
            "-osirrox".to_string(),
            "on".to_string(),
            "-indev".to_string(),
            path.display().to_string(),
            "-extract".to_string(),
            iso_path.to_string(),
            out.display().to_string(),
        ],
        None,
    );

    match result {
        Ok(_) if out.exists() => {
            // Read before removing so the temp dir is always cleaned up,
            // even when read_to_string returns an error (e.g. invalid UTF-8).
            let read_result = std::fs::read_to_string(&out);
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(Some(read_result?))
        }
        Ok(_) => {
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(None)
        }
        Err(_) => {
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(None)
        }
    }
}

fn iso_path_exists(path: &Path, iso_path: &str) -> EngineResult<bool> {
    let result = crate::orchestrator::run_command_capture(
        "xorriso",
        &[
            "-indev".to_string(),
            path.display().to_string(),
            "-ls".to_string(),
            iso_path.to_string(),
        ],
        None,
    );

    match result {
        Ok(output) => Ok(!output.stdout.trim().is_empty()),
        Err(_) => Ok(false),
    }
}

fn infer_from_label(label: &str, info: &mut IsoMetadata) {
    let lowered = label.to_lowercase();
    if lowered.contains("ubuntu") {
        info.distro = Some(Distro::Ubuntu);
    } else if lowered.contains("mint") {
        info.distro = Some(Distro::Mint);
    } else if lowered.contains("fedora") {
        info.distro = Some(Distro::Fedora);
    } else if lowered.contains("arch") {
        info.distro = Some(Distro::Arch);
    }

    if info.architecture.is_none() {
        info.architecture = infer_architecture(label);
    }
    if info.release.is_none() {
        info.release = capture_version(label);
    }
}

fn infer_from_disk_info(body: &str, info: &mut IsoMetadata) {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return;
    }

    infer_from_label(trimmed, info);
    if info.edition.is_none() {
        info.edition = Some(trimmed.to_string());
    }
}

fn infer_from_treeinfo(body: &str, info: &mut IsoMetadata) {
    for line in body.lines() {
        if let Some(value) = line.strip_prefix("family =") {
            let family = value.trim().to_lowercase();
            if family.contains("fedora") {
                info.distro = Some(Distro::Fedora);
            }
        }
        if let Some(value) = line.strip_prefix("version =") {
            info.release = Some(value.trim().to_string());
        }
        if let Some(value) = line.strip_prefix("arch =") {
            info.architecture = Some(value.trim().to_string());
        }
        if let Some(value) = line.strip_prefix("variant =") {
            info.edition = Some(value.trim().to_string());
        }
    }
}

fn infer_from_arch_version(body: &str, info: &mut IsoMetadata) {
    info.distro = Some(Distro::Arch);
    let version = body
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    if let Some(version) = version {
        info.release = Some(version.to_string());
    }
}

fn capture_version(input: &str) -> Option<String> {
    let regex = Regex::new(r"(\d{4}\.\d{2}\.\d{2}|\d{2}\.\d{2}|\d{1,2}(?:\.\d+)?)").ok()?;
    regex
        .captures(input)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

fn infer_architecture(input: &str) -> Option<String> {
    let lowered = input.to_lowercase();
    if lowered.contains("amd64") || lowered.contains("x86_64") || lowered.contains("64bit") {
        Some("x86_64".to_string())
    } else if lowered.contains("arm64") || lowered.contains("aarch64") {
        Some("aarch64".to_string())
    } else if lowered.contains("i386") || lowered.contains("i686") || lowered.contains("32bit") {
        Some("i686".to_string())
    } else {
        None
    }
}

pub(crate) fn read_primary_volume_id(path: &Path) -> EngineResult<Option<String>> {
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::Start(16 * 2048))?;

    let mut sector = [0_u8; 2048];
    if let Err(error) = file.read_exact(&mut sector) {
        return Err(EngineError::InvalidConfig(format!(
            "{} is too small to be an ISO image: {error}",
            path.display()
        )));
    }

    if &sector[1..6] != b"CD001" {
        return Err(EngineError::InvalidConfig(format!(
            "{} is not an ISO-9660 image",
            path.display()
        )));
    }

    let raw = &sector[40..72];
    let text = String::from_utf8_lossy(raw)
        .trim()
        .trim_matches(char::from(0))
        .trim()
        .to_string();
    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_from_label() {
        assert_eq!(
            capture_version("Ubuntu 24.04.1 LTS"),
            Some("24.04".to_string())
        );
        assert_eq!(
            capture_version("Arch Linux 2026.03.05"),
            Some("2026.03.05".to_string())
        );
    }

    #[test]
    fn infers_arch_from_label() {
        assert_eq!(
            infer_architecture("Ubuntu amd64"),
            Some("x86_64".to_string())
        );
        assert_eq!(
            infer_architecture("Fedora aarch64"),
            Some("aarch64".to_string())
        );
    }

    #[test]
    fn infer_from_disk_info_non_empty_sets_edition() {
        let mut info = IsoMetadata {
            source_path: PathBuf::from("/tmp/test.iso"),
            source_kind: SourceKind::LocalPath,
            source_value: "/tmp/test.iso".to_string(),
            size_bytes: 0,
            sha256: String::new(),
            volume_id: None,
            distro: None,
            release: None,
            edition: None,
            architecture: None,
            rootfs_path: None,
            boot: BootSupport::default(),
            inspected_at: String::new(),
            warnings: Vec::new(),
        };
        infer_from_disk_info(
            "Ubuntu 24.04 LTS \"Noble Numbat\" - Release amd64 (20240425)",
            &mut info,
        );
        assert_eq!(info.distro, Some(Distro::Ubuntu));
        assert!(
            info.edition.is_some(),
            "edition must be set from .disk/info"
        );
    }

    #[test]
    fn infer_from_label_recognises_each_known_distro() {
        for (label, distro) in [
            ("Ubuntu 24.04", Distro::Ubuntu),
            ("Linux Mint 22", Distro::Mint),
            ("Fedora 40 Server", Distro::Fedora),
            ("Arch Linux 2026.05.01", Distro::Arch),
        ] {
            let mut info = empty_metadata();
            infer_from_label(label, &mut info);
            assert_eq!(info.distro, Some(distro), "label {label} -> {distro:?}");
        }
    }

    #[test]
    fn infer_from_label_leaves_distro_unset_for_unknown_label() {
        let mut info = empty_metadata();
        infer_from_label("UnknownDistro 1.0", &mut info);
        assert!(info.distro.is_none(), "unknown label must not set a distro");
    }

    #[test]
    fn infer_architecture_returns_none_for_unrecognised_text() {
        assert!(infer_architecture("no architecture mentioned").is_none());
    }

    #[test]
    fn infer_architecture_recognises_i686_aliases() {
        assert_eq!(infer_architecture("Linux i386"), Some("i686".to_string()));
        assert_eq!(infer_architecture("Linux i686"), Some("i686".to_string()));
        assert_eq!(
            infer_architecture("Live 32bit edition"),
            Some("i686".to_string())
        );
    }

    #[test]
    fn capture_version_returns_none_for_label_without_digits() {
        assert!(capture_version("no digits here").is_none());
    }

    #[test]
    fn infer_from_arch_version_sets_distro_and_release() {
        let mut info = empty_metadata();
        infer_from_arch_version("2026.05.01\n", &mut info);
        assert_eq!(info.distro, Some(Distro::Arch));
        assert_eq!(info.release.as_deref(), Some("2026.05.01"));
    }

    #[test]
    fn infer_from_arch_version_leaves_release_unset_for_blank_body() {
        let mut info = empty_metadata();
        infer_from_arch_version("\n", &mut info);
        assert_eq!(info.distro, Some(Distro::Arch));
        assert!(info.release.is_none(), "blank body must not set release");
    }

    #[test]
    fn infer_from_disk_info_does_not_overwrite_existing_edition() {
        let mut info = empty_metadata();
        info.edition = Some("Pre-set".to_string());
        infer_from_disk_info("Ubuntu 24.04", &mut info);
        assert_eq!(
            info.edition.as_deref(),
            Some("Pre-set"),
            "edition must not be overwritten"
        );
    }

    #[test]
    fn read_primary_volume_id_rejects_too_small_file() {
        let dir = tempfile::tempdir().expect("dir");
        let p = dir.path().join("small.iso");
        std::fs::write(&p, b"not enough bytes").expect("write");
        let r = read_primary_volume_id(&p);
        assert!(matches!(r, Err(EngineError::InvalidConfig(_))));
    }

    #[test]
    fn read_primary_volume_id_rejects_file_without_cd001() {
        let dir = tempfile::tempdir().expect("dir");
        let p = dir.path().join("blob.iso");
        std::fs::write(&p, vec![0_u8; 17 * 2048]).expect("write");
        let r = read_primary_volume_id(&p);
        assert!(matches!(r, Err(EngineError::InvalidConfig(_))));
    }

    #[test]
    fn read_primary_volume_id_returns_volume_label_when_present() {
        let dir = tempfile::tempdir().expect("dir");
        let p = dir.path().join("ok.iso");
        let mut blob = vec![0_u8; 17 * 2048];
        let pvd = 16 * 2048;
        blob[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        blob[pvd + 40..pvd + 48].copy_from_slice(b"FORGEISO");
        std::fs::write(&p, &blob).expect("write");
        let vid = read_primary_volume_id(&p).expect("read");
        assert_eq!(vid.as_deref(), Some("FORGEISO"));
    }

    #[test]
    fn read_primary_volume_id_returns_none_for_empty_label() {
        let dir = tempfile::tempdir().expect("dir");
        let p = dir.path().join("blank.iso");
        let mut blob = vec![0_u8; 17 * 2048];
        let pvd = 16 * 2048;
        blob[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        // bytes 40..72 stay zero -> label is empty after trim
        std::fs::write(&p, &blob).expect("write");
        let vid = read_primary_volume_id(&p).expect("read");
        assert!(vid.is_none(), "empty PVD label must yield None");
    }

    fn empty_metadata() -> IsoMetadata {
        IsoMetadata {
            source_path: PathBuf::from("/tmp/test.iso"),
            source_kind: SourceKind::LocalPath,
            source_value: "/tmp/test.iso".to_string(),
            size_bytes: 0,
            sha256: String::new(),
            volume_id: None,
            distro: None,
            release: None,
            edition: None,
            architecture: None,
            rootfs_path: None,
            boot: BootSupport::default(),
            inspected_at: String::new(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn infer_from_treeinfo_fedora_sets_distro_and_release() {
        let body = "[general]\nfamily = Fedora\nversion = 40\narch = x86_64\nvariant = Server\n";
        let mut info = IsoMetadata {
            source_path: PathBuf::from("/tmp/test.iso"),
            source_kind: SourceKind::LocalPath,
            source_value: "/tmp/test.iso".to_string(),
            size_bytes: 0,
            sha256: String::new(),
            volume_id: None,
            distro: None,
            release: None,
            edition: None,
            architecture: None,
            rootfs_path: None,
            boot: BootSupport::default(),
            inspected_at: String::new(),
            warnings: Vec::new(),
        };
        infer_from_treeinfo(body, &mut info);
        assert_eq!(info.distro, Some(Distro::Fedora));
        assert_eq!(info.release.as_deref(), Some("40"));
        assert_eq!(info.architecture.as_deref(), Some("x86_64"));
        assert_eq!(info.edition.as_deref(), Some("Server"));
    }
}
