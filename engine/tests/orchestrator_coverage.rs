//! Coverage tests for orchestrator surfaces that don't require xorriso/qemu.
//!
//! These tests exist to lift workspace coverage above the tarpaulin floor by
//! exercising the pure-Rust paths that the binary-dependent integration tests
//! can't touch on stripped-down CI workers (no xorriso, no qemu, no real ISO).
//!
//! Scope:
//! * `ForgeIsoEngine::report(..)` — JSON output, HTML output, unsupported
//!   format error, and missing input file error.
//! * `ForgeIsoEngine::inspect_source(..)` — NotFound for a non-existent local
//!   path (the path-resolution branch that doesn't need a real ISO byte
//!   stream).
//! * `ForgeIsoEngine::new()` / `Default` / `subscribe()` — broadcast wiring.
//! * `From<TestResult> for TestSummary` — trivial converter that nonetheless
//!   needs an explicit kill-test so a future field rename doesn't silently
//!   drop data.
//!
//! These tests deliberately avoid `xorriso`, `qemu-system-x86_64`, network
//! I/O, and real ISO bytes — they run on every workstation and CI worker.

use std::path::PathBuf;

use forgeiso_engine::config::{BuildConfig, IsoSource, ProfileKind, ScanPolicy, TestingPolicy};
use forgeiso_engine::iso::{BootSupport, IsoMetadata, SourceKind};
use forgeiso_engine::report::BuildReport;
use forgeiso_engine::{EngineEvent, ForgeIsoEngine, TestResult};
use tempfile::TempDir;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn dummy_metadata() -> IsoMetadata {
    IsoMetadata {
        source_path: PathBuf::from("/tmp/example.iso"),
        source_kind: SourceKind::LocalPath,
        source_value: "/tmp/example.iso".to_string(),
        size_bytes: 0,
        sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        volume_id: Some("ForgeISO-test".to_string()),
        distro: None,
        release: Some("test".to_string()),
        edition: None,
        architecture: Some("amd64".to_string()),
        rootfs_path: None,
        boot: BootSupport::default(),
        inspected_at: chrono::Utc::now().to_rfc3339(),
        warnings: vec![],
    }
}

fn dummy_build_config() -> BuildConfig {
    BuildConfig {
        name: "coverage-fixture".to_string(),
        source: IsoSource::from_raw("/tmp/example.iso".to_string()),
        overlay_dir: None,
        output_label: Some("LABEL".to_string()),
        profile: ProfileKind::Minimal,
        auto_scan: false,
        auto_test: false,
        scanning: ScanPolicy::default(),
        testing: TestingPolicy::default(),
        keep_workdir: false,
        expected_sha256: None,
    }
}

fn write_seed_report(build_dir: &PathBuf) {
    std::fs::create_dir_all(build_dir).expect("create build dir");
    let report = BuildReport::new(&dummy_build_config(), &dummy_metadata());
    let raw = serde_json::to_vec_pretty(&report).expect("serialise");
    std::fs::write(build_dir.join("build-report.json"), raw).expect("write");
}

// ── ForgeIsoEngine bring-up ──────────────────────────────────────────────────

#[test]
fn engine_new_and_default_produce_independent_subscribers() {
    let e1 = ForgeIsoEngine::new();
    let e2 = ForgeIsoEngine::default();
    let _r1 = e1.subscribe();
    let _r2 = e2.subscribe();
    // Just exercising the constructors + subscribe wiring; if either panics
    // (e.g. broadcast::channel(0)), the test fails immediately.
}

#[test]
fn engine_subscribe_receives_emitted_events() {
    let engine = ForgeIsoEngine::new();
    let mut rx = engine.subscribe();
    // emit() is pub(crate); use the public report() flow to push an event.
    let tmp = TempDir::new().expect("tmp");
    let build_dir = tmp.path().to_path_buf();
    write_seed_report(&build_dir);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    rt.block_on(async {
        engine
            .report(&build_dir, "json")
            .await
            .expect("report json");
    });
    // At least two infos should have been emitted by report().
    let mut seen = 0_usize;
    while let Ok(_evt) = rx.try_recv() {
        seen += 1;
    }
    assert!(
        seen >= 2,
        "report() should emit at least 2 events, got {seen}"
    );
}

// ── ForgeIsoEngine::report ───────────────────────────────────────────────────

#[test]
fn report_writes_json_when_format_is_json() {
    let tmp = TempDir::new().expect("tmp");
    let build_dir = tmp.path().to_path_buf();
    write_seed_report(&build_dir);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    let engine = ForgeIsoEngine::new();
    let out = rt
        .block_on(engine.report(&build_dir, "json"))
        .expect("report ok");
    assert_eq!(
        out.file_name().and_then(|n| n.to_str()),
        Some("report.json")
    );
    assert!(out.exists(), "report.json must exist");
    let body = std::fs::read_to_string(&out).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid json");
    assert!(
        parsed.get("metadata").is_some(),
        "report.json must contain metadata"
    );
}

#[test]
fn report_writes_html_when_format_is_html() {
    let tmp = TempDir::new().expect("tmp");
    let build_dir = tmp.path().to_path_buf();
    write_seed_report(&build_dir);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    let engine = ForgeIsoEngine::new();
    let out = rt
        .block_on(engine.report(&build_dir, "html"))
        .expect("report ok");
    assert_eq!(
        out.file_name().and_then(|n| n.to_str()),
        Some("report.html")
    );
    let body = std::fs::read_to_string(&out).expect("read");
    assert!(
        body.contains("<!doctype html>"),
        "html report must start with doctype"
    );
    assert!(
        body.contains("ForgeISO Local Build Report"),
        "html report must include title"
    );
}

#[test]
fn report_rejects_unsupported_format() {
    let tmp = TempDir::new().expect("tmp");
    let build_dir = tmp.path().to_path_buf();
    write_seed_report(&build_dir);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    let engine = ForgeIsoEngine::new();
    let err = rt
        .block_on(engine.report(&build_dir, "yaml"))
        .expect_err("yaml is not supported");
    assert!(
        err.to_string().to_lowercase().contains("unsupported"),
        "error message should mention 'unsupported': {err}"
    );
}

#[test]
fn report_returns_io_error_when_seed_missing() {
    let tmp = TempDir::new().expect("tmp");
    let build_dir = tmp.path().to_path_buf();
    // No seed written.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    let engine = ForgeIsoEngine::new();
    let err = rt
        .block_on(engine.report(&build_dir, "json"))
        .expect_err("missing seed must error");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("no such") || msg.contains("not found") || msg.contains("io"),
        "error should describe missing file: {err}"
    );
}

#[test]
fn report_rejects_corrupt_seed_json() {
    let tmp = TempDir::new().expect("tmp");
    let build_dir = tmp.path().to_path_buf();
    std::fs::create_dir_all(&build_dir).expect("create");
    std::fs::write(build_dir.join("build-report.json"), b"not-json").expect("write");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    let engine = ForgeIsoEngine::new();
    let err = rt
        .block_on(engine.report(&build_dir, "json"))
        .expect_err("corrupt seed must error");
    // serde_json::Error is wrapped in EngineError::Serde; just confirm we got one.
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("expected") || msg.contains("invalid") || msg.contains("json"),
        "error should describe parse failure: {err}"
    );
}

// ── ForgeIsoEngine::inspect_source — Path NotFound branch ────────────────────

#[test]
fn inspect_source_local_path_not_found_errors() {
    let tmp = TempDir::new().expect("tmp");
    let cache = tmp.path().to_path_buf();
    let missing = tmp.path().join("does-not-exist.iso");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    let engine = ForgeIsoEngine::new();
    let err = rt
        .block_on(engine.inspect_source(&missing.display().to_string(), Some(&cache)))
        .expect_err("missing local ISO must error");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("does not exist") || msg.contains("not found") || msg.contains("no such"),
        "error should describe missing local source: {err}"
    );
}

// ── From<TestResult> for TestSummary ─────────────────────────────────────────

#[test]
fn test_result_to_summary_preserves_all_fields() {
    let logs = vec![PathBuf::from("/tmp/a.log"), PathBuf::from("/tmp/b.log")];
    let result = TestResult {
        bios: true,
        uefi: false,
        logs: logs.clone(),
        passed: true,
    };
    let summary: forgeiso_engine::report::TestSummary = result.into();
    assert!(summary.bios);
    assert!(!summary.uefi);
    assert!(summary.passed);
    assert_eq!(
        summary.logs,
        logs.iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>(),
        "log paths must be string-rendered identically",
    );
}

#[test]
fn test_result_to_summary_handles_empty_logs() {
    let result = TestResult {
        bios: false,
        uefi: true,
        logs: vec![],
        passed: false,
    };
    let summary: forgeiso_engine::report::TestSummary = result.into();
    assert!(!summary.bios);
    assert!(summary.uefi);
    assert!(!summary.passed);
    assert!(summary.logs.is_empty());
}

// ── inspect_iso — synthetic ISO with CD001 primary volume descriptor ─────────

/// Writes a minimal ISO-9660 fixture: 16 empty sectors + a primary volume
/// descriptor at sector 16 with the volume identifier `vol_id`. Enough for
/// `inspect_iso` to succeed without xorriso (it will append a warning).
fn write_synthetic_iso(out: &std::path::Path, vol_id: &str) {
    const SECTOR: usize = 2048;
    let mut buf = vec![0_u8; 17 * SECTOR];
    // Sector 16: primary volume descriptor.
    let s16 = 16 * SECTOR;
    buf[s16] = 1; // type = primary
    buf[s16 + 1..s16 + 6].copy_from_slice(b"CD001");
    buf[s16 + 6] = 1; // version
                      // Volume identifier lives at offset 40..72 (32 bytes), space-padded.
    let id = vol_id.as_bytes();
    let id_len = id.len().min(32);
    let dst = &mut buf[s16 + 40..s16 + 40 + id_len];
    dst.copy_from_slice(&id[..id_len]);
    for i in id_len..32 {
        buf[s16 + 40 + i] = b' ';
    }
    std::fs::write(out, &buf).expect("write synthetic ISO");
}

#[test]
fn inspect_iso_reads_synthetic_volume_id_and_warns_without_xorriso() {
    let tmp = TempDir::new().expect("tmp");
    let iso = tmp.path().join("synthetic.iso");
    write_synthetic_iso(&iso, "ubuntu 24.04 amd64");
    let md =
        forgeiso_engine::iso::inspect_iso(&iso, SourceKind::LocalPath, iso.display().to_string())
            .expect("inspect ok");
    assert_eq!(md.volume_id.as_deref(), Some("ubuntu 24.04 amd64"));
    // Inferred from label.
    assert_eq!(
        md.architecture.as_deref(),
        Some("x86_64"),
        "amd64 in label should map to x86_64"
    );
    assert_eq!(md.size_bytes, 17 * 2048);
    assert!(!md.sha256.is_empty(), "sha256 must be computed");
    // Without xorriso a warning is appended; tolerate either presence (CI may have xorriso).
    if which::which("xorriso").is_err() {
        assert!(
            md.warnings.iter().any(|w| w.contains("xorriso")),
            "expected xorriso warning, got {:?}",
            md.warnings
        );
    }
}

#[test]
fn inspect_iso_rejects_non_iso_file() {
    let tmp = TempDir::new().expect("tmp");
    let bad = tmp.path().join("not-an-iso.bin");
    std::fs::write(&bad, vec![0u8; 16 * 2048 + 2048]).expect("write");
    // 16 sectors of zeros + one zero sector — sector 16 lacks CD001.
    let err =
        forgeiso_engine::iso::inspect_iso(&bad, SourceKind::LocalPath, bad.display().to_string())
            .expect_err("zero-filled sector 16 must fail CD001 check");
    assert!(
        err.to_string().to_lowercase().contains("not an iso"),
        "error should mention ISO-9660 mismatch: {err}"
    );
}

#[test]
fn inspect_iso_rejects_truncated_file() {
    let tmp = TempDir::new().expect("tmp");
    let small = tmp.path().join("tiny.iso");
    std::fs::write(&small, vec![0u8; 100]).expect("write");
    let err = forgeiso_engine::iso::inspect_iso(
        &small,
        SourceKind::LocalPath,
        small.display().to_string(),
    )
    .expect_err("truncated file must fail");
    assert!(
        err.to_string().to_lowercase().contains("too small")
            || err.to_string().to_lowercase().contains("invalid"),
        "error should mention size: {err}"
    );
}

#[test]
fn inspect_iso_returns_not_found_for_missing_path() {
    let tmp = TempDir::new().expect("tmp");
    let missing = tmp.path().join("missing.iso");
    let err = forgeiso_engine::iso::inspect_iso(
        &missing,
        SourceKind::LocalPath,
        missing.display().to_string(),
    )
    .expect_err("missing path must fail");
    assert!(
        err.to_string().to_lowercase().contains("not found")
            || err.to_string().to_lowercase().contains("no such"),
        "error should describe missing file: {err}"
    );
}

#[test]
fn inspect_iso_recognises_fedora_label() {
    let tmp = TempDir::new().expect("tmp");
    let iso = tmp.path().join("fedora.iso");
    write_synthetic_iso(&iso, "Fedora-Workstation-40-1.14");
    let md =
        forgeiso_engine::iso::inspect_iso(&iso, SourceKind::LocalPath, iso.display().to_string())
            .expect("inspect ok");
    assert!(
        matches!(md.distro, Some(forgeiso_engine::config::Distro::Fedora)),
        "Fedora label should map to Distro::Fedora, got {:?}",
        md.distro
    );
}

#[test]
fn inspect_iso_recognises_arm64_arch() {
    let tmp = TempDir::new().expect("tmp");
    let iso = tmp.path().join("arm.iso");
    write_synthetic_iso(&iso, "Debian-12-arm64");
    let md =
        forgeiso_engine::iso::inspect_iso(&iso, SourceKind::LocalPath, iso.display().to_string())
            .expect("inspect ok");
    assert_eq!(md.architecture.as_deref(), Some("aarch64"));
}

// ── generate_autoinstall_yaml — kill-tests for the is_ubuntu_like guard ─────
// These mirror the lib-level tests in engine/src/autoinstall/ubuntu/tests.rs
// but live in the integration test layer so cargo-mutants compiles them as a
// separate test crate. This belt-and-braces approach is here because earlier
// full mutation runs reported the lib-side `delete ! in is_ubuntu_like`
// mutant as MISSED — rerunning under the integration crate confirms whether
// the survivor is a real test gap or a build-cache artifact.

#[test]
fn integration_generate_ubuntu_with_ppa_pulls_software_properties_common() {
    use forgeiso_engine::autoinstall::generate_autoinstall_yaml;
    use forgeiso_engine::config::{InjectConfig, IsoSource};
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/coverage.iso".to_string()),
        out_name: "out.iso".to_string(),
        distro: None, // None defaults to Ubuntu (is_ubuntu_like = true).
        apt_repos: vec!["ppa:deadsnakes/ppa".to_string()],
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    // Slice the `packages:` section so we don't accidentally count the ppa
    // string in `late-commands:` (which adds the same PPA via add-apt-repository).
    let packages_block = yaml.split("late-commands:").next().unwrap_or(&yaml);
    assert!(
        packages_block.contains("software-properties-common"),
        "Ubuntu + PPA must add software-properties-common to the packages: section, got:\n{packages_block}"
    );
}

#[test]
fn integration_generate_arch_with_ppa_does_not_pull_software_properties_common() {
    use forgeiso_engine::autoinstall::generate_autoinstall_yaml;
    use forgeiso_engine::config::{Distro, InjectConfig, IsoSource};
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/coverage.iso".to_string()),
        out_name: "out.iso".to_string(),
        distro: Some(Distro::Arch),
        apt_repos: vec!["ppa:deadsnakes/ppa".to_string()],
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        !yaml.contains("software-properties-common"),
        "Arch must NEVER auto-add software-properties-common (no apt): {yaml}"
    );
}

#[test]
fn integration_generate_fedora_with_ppa_does_not_pull_software_properties_common() {
    use forgeiso_engine::autoinstall::generate_autoinstall_yaml;
    use forgeiso_engine::config::{Distro, InjectConfig, IsoSource};
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/coverage.iso".to_string()),
        out_name: "out.iso".to_string(),
        distro: Some(Distro::Fedora),
        apt_repos: vec!["ppa:deadsnakes/ppa".to_string()],
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        !yaml.contains("software-properties-common"),
        "Fedora must NEVER auto-add software-properties-common: {yaml}"
    );
}

#[test]
fn integration_generate_ubuntu_with_apt_mirror_emits_apt_section() {
    use forgeiso_engine::autoinstall::generate_autoinstall_yaml;
    use forgeiso_engine::config::{InjectConfig, IsoSource};
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/coverage.iso".to_string()),
        out_name: "out.iso".to_string(),
        distro: None,
        apt_mirror: Some("http://mirrors.example.com/ubuntu".to_string()),
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        yaml.contains("apt:") && yaml.contains("mirrors.example.com"),
        "Ubuntu + apt_mirror must emit apt: section with the mirror URL, got:\n{yaml}"
    );
}

#[test]
fn integration_generate_arch_with_apt_mirror_omits_apt_section() {
    use forgeiso_engine::autoinstall::generate_autoinstall_yaml;
    use forgeiso_engine::config::{Distro, InjectConfig, IsoSource};
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/coverage.iso".to_string()),
        out_name: "out.iso".to_string(),
        distro: Some(Distro::Arch),
        apt_mirror: Some("http://mirrors.example.com/ubuntu".to_string()),
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    // Arch has no apt; the apt: section must not appear regardless of apt_mirror.
    assert!(
        !yaml.contains("apt:\n"),
        "Arch must NEVER emit an apt: block: {yaml}"
    );
}

#[test]
fn integration_generate_ubuntu_with_firewall_pulls_ufw() {
    use forgeiso_engine::autoinstall::generate_autoinstall_yaml;
    use forgeiso_engine::config::{FirewallConfig, InjectConfig, IsoSource};
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/coverage.iso".to_string()),
        out_name: "out.iso".to_string(),
        distro: None,
        firewall: FirewallConfig {
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    assert!(
        yaml.contains("ufw"),
        "Ubuntu + firewall.enabled must auto-add ufw, got:\n{yaml}"
    );
}

#[test]
fn integration_generate_arch_with_firewall_does_not_pull_ufw() {
    use forgeiso_engine::autoinstall::generate_autoinstall_yaml;
    use forgeiso_engine::config::{Distro, FirewallConfig, InjectConfig, IsoSource};
    let cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/coverage.iso".to_string()),
        out_name: "out.iso".to_string(),
        distro: Some(Distro::Arch),
        firewall: FirewallConfig {
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
    // Arch uses iptables/nftables, never ufw.
    let packages_block = yaml.split("late-commands:").next().unwrap_or(&yaml);
    assert!(
        !packages_block.contains("- ufw\n"),
        "Arch must NEVER add ufw to the packages: section, got:\n{packages_block}"
    );
}

// ── Engine event emission shape ──────────────────────────────────────────────

#[test]
fn report_emits_two_phase_completion_events() {
    let tmp = TempDir::new().expect("tmp");
    let build_dir = tmp.path().to_path_buf();
    write_seed_report(&build_dir);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    let engine = ForgeIsoEngine::new();
    let mut rx = engine.subscribe();
    rt.block_on(async {
        engine.report(&build_dir, "json").await.expect("report ok");
    });
    let mut events: Vec<EngineEvent> = Vec::new();
    while let Ok(evt) = rx.try_recv() {
        events.push(evt);
    }
    // Expect at least one Report-phase info and one Complete-phase info.
    use forgeiso_engine::EventPhase;
    assert!(
        events.iter().any(|e| e.phase == EventPhase::Report),
        "must emit a Report-phase event, got {events:?}",
    );
    assert!(
        events.iter().any(|e| e.phase == EventPhase::Complete),
        "must emit a Complete-phase event, got {events:?}",
    );
}
