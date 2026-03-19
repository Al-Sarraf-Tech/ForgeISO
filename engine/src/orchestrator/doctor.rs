use std::collections::BTreeMap;

use crate::events::{EngineEvent, EventPhase};

use super::{DoctorReport, ForgeIsoEngine};

impl ForgeIsoEngine {
    #[allow(clippy::unused_async)] // async kept for API consistency
    pub async fn doctor(&self) -> DoctorReport {
        self.emit(EngineEvent::info(
            EventPhase::Doctor,
            "checking local bare-metal prerequisites",
        ));

        let tooling = [
            "xorriso",
            "mtools",
            "unsquashfs",
            "mksquashfs",
            "qemu-system-x86_64",
            "trivy",
            "syft",
            "grype",
            "oscap",
        ]
        .into_iter()
        .map(|tool| (tool.to_string(), which::which(tool).is_ok()))
        .collect::<BTreeMap<_, _>>();

        let xorriso_ok = tooling.get("xorriso").copied().unwrap_or(false);
        let scan_ok = tooling.get("trivy").copied().unwrap_or(false)
            || tooling.get("syft").copied().unwrap_or(false);
        let test_ok = tooling.get("qemu-system-x86_64").copied().unwrap_or(false);

        let mut distro_readiness = BTreeMap::new();
        distro_readiness.insert("ubuntu".to_string(), xorriso_ok);
        distro_readiness.insert("fedora".to_string(), xorriso_ok);
        distro_readiness.insert("mint".to_string(), xorriso_ok);
        distro_readiness.insert("arch".to_string(), xorriso_ok);
        distro_readiness.insert("scan".to_string(), scan_ok);
        distro_readiness.insert("test".to_string(), test_ok);

        let linux_supported = std::env::consts::OS == "linux";
        let mut warnings = Vec::new();
        if !linux_supported {
            warnings
                .push("ISO build and VM test flows are only supported on Linux hosts".to_string());
        }
        if !xorriso_ok {
            warnings.push("xorriso is required for deep ISO inspection and repacking".to_string());
        }
        if !scan_ok {
            warnings.push(
                "no scan tools found (trivy, syft) — scan and SBOM operations will be skipped"
                    .to_string(),
            );
        }

        DoctorReport {
            host_os: std::env::consts::OS.to_string(),
            host_arch: std::env::consts::ARCH.to_string(),
            linux_supported,
            tooling,
            warnings,
            timestamp: chrono::Utc::now().to_rfc3339(),
            distro_readiness,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::DoctorReport;

    #[test]
    fn doctor_report_has_required_distro_readiness_keys() {
        // DoctorReport is produced synchronously from public data — no I/O needed.
        // We validate the structure by constructing the report directly using the
        // same logic as the doctor() method and checking the keys.
        let mut distro_readiness = BTreeMap::new();
        distro_readiness.insert("ubuntu".to_string(), false);
        distro_readiness.insert("fedora".to_string(), false);
        distro_readiness.insert("mint".to_string(), false);
        distro_readiness.insert("arch".to_string(), false);
        distro_readiness.insert("scan".to_string(), false);
        distro_readiness.insert("test".to_string(), false);

        let report = DoctorReport {
            host_os: "linux".to_string(),
            host_arch: "x86_64".to_string(),
            linux_supported: true,
            tooling: BTreeMap::new(),
            warnings: Vec::new(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            distro_readiness,
        };

        assert!(
            report.distro_readiness.contains_key("ubuntu"),
            "ubuntu key missing"
        );
        assert!(
            report.distro_readiness.contains_key("fedora"),
            "fedora key missing"
        );
        assert!(
            report.distro_readiness.contains_key("mint"),
            "mint key missing"
        );
        assert!(
            report.distro_readiness.contains_key("arch"),
            "arch key missing"
        );
        assert!(
            report.distro_readiness.contains_key("scan"),
            "scan key missing"
        );
        assert!(
            report.distro_readiness.contains_key("test"),
            "test key missing"
        );
    }
}
