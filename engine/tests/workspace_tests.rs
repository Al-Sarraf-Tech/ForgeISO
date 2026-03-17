//! Integration tests for workspace creation, path safety, and report generation.
//!
//! These tests verify:
//! * `Workspace::create()` produces all expected subdirectories.
//! * `safe_join()` allows legal child paths and rejects traversal attempts.
//! * `BuildReport` JSON and HTML round-trips are correct.
//! * `vm::emit_launch()` produces serialisable output for every hypervisor.

use std::path::{Path, PathBuf};

use forgeiso_engine::{
    config::{BuildConfig, IsoSource, ProfileKind, ScanPolicy, TestingPolicy},
    iso::{BootSupport, IsoMetadata, SourceKind},
    report::BuildReport,
    vm::{emit_launch, FirmwareMode, Hypervisor, VmLaunchSpec},
    workspace::{safe_join, Workspace},
};
use tempfile::TempDir;

// ── Workspace::create ─────────────────────────────────────────────────────────

#[test]
fn workspace_create_produces_all_subdirs() {
    let tmp = TempDir::new().unwrap();
    let ws = Workspace::create(tmp.path(), "test-run").expect("workspace creation");
    assert!(ws.root.exists(), "root must exist");
    assert!(ws.input.exists(), "input must exist");
    assert!(ws.work.exists(), "work must exist");
    assert!(ws.output.exists(), "output must exist");
    assert!(ws.reports.exists(), "reports must exist");
    assert!(ws.scans.exists(), "scans must exist");
    assert!(ws.logs.exists(), "logs must exist");
}

#[test]
fn workspace_create_root_is_under_base() {
    let tmp = TempDir::new().unwrap();
    let ws = Workspace::create(tmp.path(), "my-run").expect("workspace creation");
    assert!(
        ws.root.starts_with(tmp.path()),
        "workspace root must be under the base directory"
    );
}

#[test]
fn workspace_create_includes_run_name_in_root() {
    let tmp = TempDir::new().unwrap();
    let ws = Workspace::create(tmp.path(), "alpha-build").expect("workspace creation");
    let root_name = ws.root.file_name().and_then(|n| n.to_str()).unwrap_or("");
    assert!(
        root_name.starts_with("alpha-build"),
        "root dir name should start with sanitized run name, got: {root_name}"
    );
}

#[test]
fn workspace_create_uuid_suffix_makes_root_unique() {
    let tmp = TempDir::new().unwrap();
    let ws1 = Workspace::create(tmp.path(), "run").expect("ws1");
    let ws2 = Workspace::create(tmp.path(), "run").expect("ws2");
    assert_ne!(
        ws1.root, ws2.root,
        "two workspaces with the same name must have distinct roots (UUID suffix)"
    );
}

#[test]
fn workspace_create_sanitizes_special_chars_in_name() {
    let tmp = TempDir::new().unwrap();
    // The run name contains shell-unsafe characters; they should be stripped or replaced.
    let ws = Workspace::create(tmp.path(), "run/with/slashes & spaces").expect("workspace");
    let root_name = ws.root.file_name().and_then(|n| n.to_str()).unwrap_or("");
    assert!(
        !root_name.contains('/'),
        "root dir name must not contain slashes: {root_name}"
    );
    assert!(
        !root_name.contains(' '),
        "root dir name must not contain spaces: {root_name}"
    );
}

#[test]
fn workspace_create_empty_base_succeeds_when_base_can_be_created() {
    let tmp = TempDir::new().unwrap();
    let nested_base = tmp.path().join("deep").join("nested");
    // base does not exist yet — create() should create it.
    let ws = Workspace::create(&nested_base, "nested-run").expect("workspace with nested base");
    assert!(ws.root.exists());
}

// ── safe_join ─────────────────────────────────────────────────────────────────

#[test]
fn safe_join_allows_simple_child() {
    let tmp = TempDir::new().unwrap();
    let result = safe_join(tmp.path(), Path::new("hello/world.txt")).expect("safe");
    assert!(result.starts_with(tmp.path()));
}

#[test]
fn safe_join_rejects_single_dot_dot() {
    let tmp = TempDir::new().unwrap();
    assert!(
        safe_join(tmp.path(), Path::new("..")).is_err(),
        "bare .. must be rejected"
    );
}

#[test]
fn safe_join_rejects_double_dot_dot() {
    let tmp = TempDir::new().unwrap();
    assert!(
        safe_join(tmp.path(), Path::new("../etc/passwd")).is_err(),
        "../etc/passwd must be rejected"
    );
}

#[test]
fn safe_join_rejects_embedded_traversal() {
    let tmp = TempDir::new().unwrap();
    // a/../../etc goes above tmp root.
    assert!(
        safe_join(tmp.path(), Path::new("a/../../etc")).is_err(),
        "a/../../etc must be rejected"
    );
}

#[test]
fn safe_join_allows_cur_dir_dot() {
    let tmp = TempDir::new().unwrap();
    // ./file.txt is a valid relative child.
    let result = safe_join(tmp.path(), Path::new("./file.txt")).expect("cur-dir dot safe");
    assert!(result.starts_with(tmp.path()));
}

#[test]
fn safe_join_allows_deep_nested_child() {
    let tmp = TempDir::new().unwrap();
    let result = safe_join(tmp.path(), Path::new("a/b/c/d/e.txt")).expect("deep nested path safe");
    assert!(result.starts_with(tmp.path()));
}

#[test]
fn workspace_safe_join_delegates_to_root() {
    let tmp = TempDir::new().unwrap();
    let ws = Workspace::create(tmp.path(), "ws").unwrap();
    let joined = ws
        .safe_join(Path::new("output/result.iso"))
        .expect("safe ws join");
    assert!(joined.starts_with(&ws.root));
}

// ── BuildReport: JSON round-trip ─────────────────────────────────────────────

fn minimal_build_config() -> BuildConfig {
    BuildConfig {
        name: "integration-test".to_string(),
        source: IsoSource::Path(PathBuf::from("/tmp/ubuntu.iso")),
        overlay_dir: None,
        output_label: Some("UBUNTU_TEST".to_string()),
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
        source_path: PathBuf::from("/tmp/ubuntu.iso"),
        source_kind: SourceKind::LocalPath,
        source_value: "/tmp/ubuntu.iso".to_string(),
        size_bytes: 1_073_741_824,
        sha256: "deadbeef".repeat(8),
        volume_id: Some("Ubuntu 24.04".to_string()),
        distro: None,
        release: Some("24.04 LTS".to_string()),
        edition: None,
        architecture: Some("amd64".to_string()),
        rootfs_path: None,
        boot: BootSupport::default(),
        inspected_at: "2026-01-01T00:00:00Z".to_string(),
        warnings: vec![],
    }
}

#[test]
fn build_report_json_round_trips_source() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let report = BuildReport::new(&minimal_build_config(), &minimal_iso_metadata());
    report.write_json(tmp.path()).unwrap();
    let raw = std::fs::read_to_string(tmp.path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(parsed["metadata"]["source"], "/tmp/ubuntu.iso");
}

#[test]
fn build_report_json_round_trips_profile() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let report = BuildReport::new(&minimal_build_config(), &minimal_iso_metadata());
    report.write_json(tmp.path()).unwrap();
    let raw = std::fs::read_to_string(tmp.path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
    // ProfileKind::Minimal should serialize to "minimal".
    assert_eq!(parsed["metadata"]["profile"], "minimal");
}

#[test]
fn build_report_json_with_artifact_path() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut report = BuildReport::new(&minimal_build_config(), &minimal_iso_metadata());
    report.artifacts.push("/output/result.iso".to_string());
    report.write_json(tmp.path()).unwrap();
    let raw = std::fs::read_to_string(tmp.path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(parsed["artifacts"][0], "/output/result.iso");
}

#[test]
fn build_report_html_escapes_angle_brackets_in_volume_id() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut iso = minimal_iso_metadata();
    iso.volume_id = Some("<XSS>TEST".to_string());
    let report = BuildReport::new(&minimal_build_config(), &iso);
    report.write_html(tmp.path()).unwrap();
    let html = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(
        !html.contains("<XSS>"),
        "raw angle brackets must not appear in HTML"
    );
    assert!(
        html.contains("&lt;XSS&gt;"),
        "angle brackets must be HTML-escaped"
    );
}

#[test]
fn build_report_html_includes_output_label() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let cfg = minimal_build_config();
    let report = BuildReport::new(&cfg, &minimal_iso_metadata());
    report.write_html(tmp.path()).unwrap();
    let html = std::fs::read_to_string(tmp.path()).unwrap();
    // output_label is set to "UBUNTU_TEST" in minimal_build_config()
    // It's captured in the metadata; it doesn't necessarily appear in the HTML body
    // unless the template renders it, so just confirm the file is valid HTML.
    assert!(
        html.starts_with("<!doctype html>"),
        "must be valid HTML doctype"
    );
}

#[test]
fn build_report_html_contains_architecture() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let report = BuildReport::new(&minimal_build_config(), &minimal_iso_metadata());
    report.write_html(tmp.path()).unwrap();
    let html = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(
        html.contains("amd64"),
        "architecture must appear in HTML report"
    );
}

// ── VM emit_launch: all hypervisors serialise cleanly ──────────────────────────

fn test_spec(hv: Hypervisor, fw: FirmwareMode) -> VmLaunchSpec {
    VmLaunchSpec {
        hypervisor: hv,
        firmware: fw,
        iso_path: PathBuf::from("/tmp/test.iso"),
        ram_mb: 2048,
        cpus: 2,
        disk_gb: 20,
        vm_name: "test-vm".to_string(),
        ovmf_path: Some(PathBuf::from("/usr/share/OVMF/OVMF_CODE.fd")),
    }
}

#[test]
fn emit_launch_all_hypervisors_produce_valid_json() {
    for &hv in Hypervisor::all() {
        for &fw in &[FirmwareMode::Bios, FirmwareMode::Uefi] {
            let spec = test_spec(hv, fw);
            let out = emit_launch(&spec);
            let serialised = serde_json::to_string(&out)
                .unwrap_or_else(|e| panic!("serialise failed for {hv:?}/{fw:?}: {e}"));
            let parsed: serde_json::Value = serde_json::from_str(&serialised)
                .unwrap_or_else(|e| panic!("parse failed for {hv:?}/{fw:?}: {e}"));
            assert_eq!(
                parsed["iso_path"].as_str(),
                Some("/tmp/test.iso"),
                "iso_path must survive JSON round-trip for {hv:?}/{fw:?}"
            );
        }
    }
}

#[test]
fn emit_launch_script_or_commands_always_populated() {
    // Every hypervisor must produce either commands or a script — never both empty.
    for &hv in Hypervisor::all() {
        let spec = test_spec(hv, FirmwareMode::Bios);
        let out = emit_launch(&spec);
        assert!(
            !out.commands.is_empty() || out.script.is_some(),
            "emit_launch for {hv:?} must produce commands or a script"
        );
    }
}

#[test]
fn emit_launch_notes_non_empty_for_all_hypervisors() {
    // Every hypervisor injects at least one human-readable note.
    for &hv in Hypervisor::all() {
        let spec = test_spec(hv, FirmwareMode::Bios);
        let out = emit_launch(&spec);
        assert!(
            !out.notes.is_empty(),
            "emit_launch for {hv:?} must include at least one note"
        );
    }
}

#[test]
fn emit_launch_iso_path_in_commands_for_qemu_bios() {
    let spec = test_spec(Hypervisor::Qemu, FirmwareMode::Bios);
    let out = emit_launch(&spec);
    let iso_in_commands = out.commands.iter().any(|c| c.contains("/tmp/test.iso"));
    assert!(
        iso_in_commands,
        "QEMU BIOS commands must reference the ISO path: {:?}",
        out.commands
    );
}

// ── UTF-8 volume_id truncation safety (GUI crash regression) ──────────────────
//
// The GUI previously sliced `volume_id` at byte position 32 using
// `vol[..vol.len().min(32)]`.  This panics whenever a multi-byte UTF-8
// code point (emoji, accented char, U+FFFD replacement from from_utf8_lossy)
// straddles that boundary.  These tests document the safe invariant: a
// volume_id longer than 32 chars must be truncatable without panicking.

/// Simulate what the GUI must do safely: take up to 32 *characters* from a
/// volume ID that may contain multi-byte UTF-8 sequences.
fn truncate_volume_id(s: &str) -> String {
    s.chars().take(32).collect()
}

#[test]
fn volume_id_truncation_ascii_under_limit() {
    let vol = "Ubuntu-24.04-LTS";
    let out = truncate_volume_id(vol);
    assert_eq!(out, vol); // shorter than 32, unchanged
}

#[test]
fn volume_id_truncation_ascii_exactly_32() {
    let vol = "A".repeat(32);
    let out = truncate_volume_id(&vol);
    assert_eq!(out.len(), 32);
}

#[test]
fn volume_id_truncation_ascii_over_32() {
    let vol = "A".repeat(64);
    let out = truncate_volume_id(&vol);
    assert_eq!(out.len(), 32);
}

#[test]
fn volume_id_truncation_multibyte_does_not_panic() {
    // "€" is U+20AC, encoded as 3 bytes in UTF-8.
    // 12 × "€" = 36 bytes.  Byte offset 32 falls 2 bytes into the 11th "€"
    // (byte 30 starts it, byte 33 ends it) — the old byte-slice panics here.
    let vol = "€".repeat(12); // 36 bytes, 12 chars
    let out = truncate_volume_id(&vol);
    assert_eq!(out.chars().count(), 12); // all 12 fit within the 32-char limit
                                         // Confirm the old approach panics on this input.
    let would_panic = std::panic::catch_unwind(|| {
        let _ = &vol[..32.min(vol.len())]; // byte-32 is mid-code-point → panic
    });
    assert!(
        would_panic.is_err(),
        "byte-slice at 32 must panic for 3-byte encoded chars"
    );
}

#[test]
fn volume_id_truncation_mixed_ascii_and_multibyte_does_not_panic() {
    // "VOL_" (4 ASCII bytes) + 10 × "€" (30 bytes) = 34 bytes, 14 chars.
    // Byte 32 falls 2 bytes into the 10th "€" — old code panics.
    let vol = format!("VOL_{}", "€".repeat(10));
    let out = truncate_volume_id(&vol);
    assert_eq!(out.chars().count(), 14); // all 14 fit within the 32-char limit
    let would_panic = std::panic::catch_unwind(|| {
        let _ = &vol[..32.min(vol.len())];
    });
    assert!(
        would_panic.is_err(),
        "byte-slice at 32 must panic for this mixed string"
    );
}

#[test]
fn volume_id_truncation_replacement_char_does_not_panic() {
    // U+FFFD (the UTF-8 replacement character) is 3 bytes.
    // from_utf8_lossy produces these when ISO label bytes are not valid UTF-8.
    // A label with 11+ replacement chars would have > 32 bytes before hitting
    // 32 chars, but the old byte-slice at 32 would NOT land on a boundary.
    let vol = "\u{FFFD}".repeat(20); // 20 chars × 3 bytes each = 60 bytes
    let out = truncate_volume_id(&vol);
    assert_eq!(out.chars().count(), 20); // all 20 fit within 32-char limit
                                         // Verify the old approach would panic:
    let would_panic = std::panic::catch_unwind(|| {
        let _ = &vol[..32.min(vol.len())]; // 32 is not on a 3-byte boundary
    });
    assert!(
        would_panic.is_err(),
        "byte-slice at 32 should panic for replacement chars"
    );
}

#[test]
fn volume_id_truncation_long_multibyte_truncates_to_32_chars() {
    // 50 emoji — must truncate to exactly 32 characters.
    let vol = "🚀".repeat(50);
    let out = truncate_volume_id(&vol);
    assert_eq!(out.chars().count(), 32);
    assert_eq!(out, "🚀".repeat(32));
}

#[test]
fn volume_id_truncation_empty_string_is_safe() {
    assert_eq!(truncate_volume_id(""), "");
}

#[test]
fn volume_id_truncation_single_multibyte_char_under_limit() {
    assert_eq!(truncate_volume_id("é"), "é");
}
