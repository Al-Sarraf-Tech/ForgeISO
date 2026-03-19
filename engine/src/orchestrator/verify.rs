use std::io::Read;
use std::path::Path;

use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};
use crate::iso::{inspect_iso, read_primary_volume_id};

use super::helpers::{default_cache_root, run_command_lossy_async};
use super::{ForgeIsoEngine, Iso9660Compliance, VerifyResult};

use crate::config::IsoSource;

impl ForgeIsoEngine {
    pub async fn verify(&self, source: &str, sums_url: Option<&str>) -> EngineResult<VerifyResult> {
        self.emit(EngineEvent::info(
            EventPhase::Verify,
            "verifying ISO checksum",
        ));

        let resolved = self
            .resolve_source(&IsoSource::from_raw(source), &default_cache_root()?)
            .await?;
        let metadata = inspect_iso(
            &resolved.source_path,
            resolved.source_kind,
            resolved.source_value,
        )?;

        let filename = resolved
            .source_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| EngineError::InvalidConfig("Unable to get ISO filename".to_string()))?;

        // Determine the SHA256SUMS URL — graceful degradation if unavailable.
        #[allow(clippy::option_if_let_else)] // multi-branch, map_or would be less readable
        let effective_sums_url: Option<String> = if let Some(url) = sums_url {
            Some(url.to_string())
        } else if let Some(distro) = metadata.distro {
            if let Some(release) = &metadata.release {
                match distro {
                    crate::config::Distro::Ubuntu => Some(format!(
                        "https://releases.ubuntu.com/{}/SHA256SUMS",
                        release
                    )),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        // If we have a sums URL, fetch and compare; otherwise just report the hash.
        let (expected, matched) = if let Some(ref url) = effective_sums_url {
            self.emit(EngineEvent::info(
                EventPhase::Verify,
                format!("fetching checksums from {}", url),
            ));
            let sums_content = reqwest::get(url).await?.text().await?;

            // Parse SHA256SUMS format: <hash>  <filename> or <hash>  *<filename>
            let mut expected_hash = None;
            for line in sums_content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let hash = parts[0];
                    let file_path = parts[1].trim_start_matches('*');
                    if file_path.ends_with(&filename) || file_path == filename {
                        expected_hash = Some(hash.to_string());
                        break;
                    }
                }
            }

            if let Some(expected) = expected_hash {
                let matched = metadata.sha256 == expected;
                self.emit(EngineEvent::info(
                    EventPhase::Verify,
                    if matched {
                        "checksum matches!".to_string()
                    } else {
                        "checksum mismatch!".to_string()
                    },
                ));
                (expected, matched)
            } else {
                // Filename not in SUMS file (e.g. renamed/injected ISO) — report hash only.
                self.emit(EngineEvent::info(
                    EventPhase::Verify,
                    format!(
                        "no entry for '{}' in checksums file — showing computed hash only",
                        filename
                    ),
                ));
                ("not found in checksums file".to_string(), false)
            }
        } else {
            self.emit(EngineEvent::info(
                EventPhase::Verify,
                "no checksums URL available — showing computed hash only".to_string(),
            ));
            ("no checksums source provided".to_string(), false)
        };

        self.emit(EngineEvent::info(
            EventPhase::Complete,
            format!("checksum verification completed (matched={matched})"),
        ));

        Ok(VerifyResult {
            filename,
            expected,
            actual: metadata.sha256,
            matched,
        })
    }

    /// Validate ISO-9660 compliance for a local file.
    ///
    /// Returns a structured `Iso9660Compliance` result without emitting errors —
    /// failure information is encoded in the result's `compliant` and `error` fields.
    #[allow(clippy::unused_async)] // async kept for API consistency
    pub async fn validate_iso9660(&self, path_str: &str) -> EngineResult<Iso9660Compliance> {
        let path = std::path::Path::new(path_str);

        self.emit(EngineEvent::info(
            EventPhase::Verify,
            format!("checking ISO-9660 compliance: {}", path.display()),
        ));

        if !path.exists() {
            return Ok(Iso9660Compliance {
                compliant: false,
                volume_id: None,
                size_bytes: 0,
                boot_bios: false,
                boot_uefi: false,
                el_torito_present: false,
                check_method: "iso9660_header".into(),
                error: Some(format!("File not found: {}", path.display())),
            });
        }

        let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        // Check CD001 primary volume descriptor at sector 16.
        let volume_id = match read_primary_volume_id(path) {
            Ok(vid) => vid,
            Err(e) => {
                self.emit(EngineEvent::warn(
                    EventPhase::Verify,
                    format!("ISO-9660 compliance failed: {e}"),
                ));
                return Ok(Iso9660Compliance {
                    compliant: false,
                    volume_id: None,
                    size_bytes,
                    boot_bios: false,
                    boot_uefi: false,
                    el_torito_present: false,
                    check_method: "iso9660_header".into(),
                    error: Some(e.to_string()),
                });
            }
        };

        // Enrich with El Torito boot detection via xorriso when available.
        let mut boot_bios = false;
        let mut boot_uefi = false;
        let mut el_torito_present = false;
        let mut check_method = "iso9660_header".to_string();

        if which::which("xorriso").is_ok() {
            check_method = "iso9660_header+xorriso".to_string();
            // xorriso may exit non-zero on some ISOs even when useful output is produced;
            // use the lossy runner so we still get stdout/stderr.
            if let Ok(result) = run_command_lossy_async(
                "xorriso",
                &[
                    "-indev".to_string(),
                    path.display().to_string(),
                    "-report_el_torito".to_string(),
                    "plain".to_string(),
                ],
                None,
            )
            .await
            {
                let report = format!(
                    "{}\n{}",
                    result.stdout.to_lowercase(),
                    result.stderr.to_lowercase()
                );
                el_torito_present = report.contains("el torito")
                    || report.contains("boot catalog")
                    || report.contains("boot img");
                boot_bios = report.contains("pltf  bios")
                    || report.contains("boot img :   1  bios")
                    || report.contains("platform id: 0x00")
                    || report.contains("platform id :  0 = 80x86");
                boot_uefi = report.contains("pltf  uefi")
                    || report.contains("boot img :   2  uefi")
                    || report.contains("platform id: 0xef")
                    || report.contains("platform id :  0xef = efi");
            }
        }

        self.emit(EngineEvent::info(
            EventPhase::Verify,
            format!(
                "ISO-9660 compliant — volume_id={:?} boot_bios={} boot_uefi={}",
                volume_id, boot_bios, boot_uefi
            ),
        ));

        Ok(Iso9660Compliance {
            compliant: true,
            volume_id,
            size_bytes,
            boot_bios,
            boot_uefi,
            el_torito_present,
            check_method,
            error: None,
        })
    }
}

pub fn sha256_file(path: &Path) -> EngineResult<String> {
    use sha2::{Digest, Sha256};

    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 8192];

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Verify a file against a caller-supplied expected SHA-256 hex digest.
/// Returns an error if the digest does not match, allowing the operation to
/// be aborted before any ISO content is trusted or modified.
pub(super) fn check_expected_sha256(path: &Path, expected: &str) -> EngineResult<()> {
    let actual = sha256_file(path)?;
    let expected_norm = expected.trim().to_ascii_lowercase();
    if actual != expected_norm {
        return Err(EngineError::Runtime(format!(
            "SHA-256 mismatch for {}: expected {expected_norm}, got {actual}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_expected_sha256_accepts_match() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
        tmp.write_all(b"hello world").expect("write");
        let path = tmp.path();
        // Known SHA-256 of "hello world"
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576d6b859e46fcdd6b0";
        // Use sha256_file to get the actual hash of our temp content
        let actual = sha256_file(path).expect("hash");
        // Verify round-trip
        assert!(check_expected_sha256(path, &actual).is_ok());
        // Verify mismatch is rejected
        assert!(check_expected_sha256(path, expected).is_err());
    }
}
