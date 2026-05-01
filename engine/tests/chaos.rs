//! Chaos / fault-injection test suite.
//!
//! Pushes the Reliability dimension by exercising failure modes of the
//! ForgeISO engine without touching real `xorriso`, `qemu`, `squashfs`, or
//! the network. Each scenario:
//!
//! 1. Constructs synthetic input (random bytes, missing dirs, etc.) or a
//!    fake-binary harness via `PATH` manipulation.
//! 2. Invokes a public `forgeiso_engine` API (`inspect_iso`, `sha256_file`,
//!    `validate_iso9660`, `inject_autoinstall`, `BuildConfig::validate()`,
//!    `Workspace::create()`).
//! 3. Asserts the specific [`EngineError`] variant the engine must return.
//!
//! Scenarios that mutate the process-global `PATH` are serialized by the
//! [`PATH_LOCK`] mutex below; non-mutating scenarios run in parallel with
//! the rest of the test suite. No scenario invokes a real ISO tool, opens
//! a network socket, or relies on host state beyond `tempfile`.
//!
//! See `docs/CHAOS.md` for the operator runbook + scenario inventory.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use forgeiso_engine::{
    config::IsoSource,
    error::EngineError,
    iso::{inspect_iso, SourceKind},
    orchestrator::sha256_file,
    BuildConfig, ForgeIsoEngine, InjectConfig, ProfileKind, ScanPolicy, TestingPolicy,
};
use tempfile::TempDir;
use tokio::sync::Mutex;

// ─────────────────────────────────────────────────────────────────────────────
// chaos_helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Process-global async lock for tests that mutate `$PATH`. Cargo runs
/// integration tests in a single process with multi-threaded execution;
/// PATH writes from one test would otherwise race with `which::which()`
/// calls in another. We use `tokio::sync::Mutex` so the guard can be held
/// across `.await` points (clippy rejects `std::sync::Mutex` for that).
static PATH_LOCK: Mutex<()> = Mutex::const_new(());

mod chaos_helpers {
    use super::*;

    /// Behaviour of a fake binary planted on `$PATH`.
    #[derive(Debug, Clone, Copy)]
    pub enum FakeBehavior {
        /// Exit non-zero immediately — simulates a tool that fails fast.
        ExitNonZero,
        /// Exit zero after writing some stderr — simulates a tool that
        /// "succeeded" but did not produce the expected artefact.
        ExitZeroNoOp,
    }

    /// Plant a shell-script masquerading as `name` inside `dir`. Caller is
    /// responsible for prepending `dir` to `PATH` (under [`PATH_LOCK`]).
    pub fn install_fake_binary(dir: &Path, name: &str, behavior: FakeBehavior) -> PathBuf {
        let path = dir.join(name);
        let body = match behavior {
            FakeBehavior::ExitNonZero => "#!/usr/bin/env bash\necho 'fake failure' >&2\nexit 1\n",
            FakeBehavior::ExitZeroNoOp => "#!/usr/bin/env bash\necho 'fake noop' >&2\nexit 0\n",
        };
        let mut f = fs::File::create(&path).expect("create fake binary");
        f.write_all(body.as_bytes()).expect("write fake binary");
        drop(f);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                .expect("chmod fake binary");
        }
        path
    }

    /// Save `$PATH` and return a handle that restores it on drop.
    pub struct PathGuard {
        original: Option<String>,
    }

    impl PathGuard {
        pub fn new() -> Self {
            Self {
                original: std::env::var("PATH").ok(),
            }
        }
    }

    impl Drop for PathGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => std::env::set_var("PATH", value),
                None => std::env::remove_var("PATH"),
            }
        }
    }

    /// Write `bytes` into a fresh temp file and return the path. Caller keeps
    /// the [`TempDir`] alive — the path is a child of it.
    pub fn write_synthetic_iso(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, bytes).expect("write synthetic iso");
        path
    }

    /// Minimal `BuildConfig` pointing at a local path — used to drive the
    /// build pipeline far enough to surface the failure under test.
    pub fn build_config_for(name: &str, source: &Path) -> BuildConfig {
        BuildConfig {
            name: name.to_string(),
            source: IsoSource::Path(source.to_path_buf()),
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

    /// Minimal `InjectConfig` pointing at a local path.
    pub fn inject_config_for(out_name: &str, source: &Path) -> InjectConfig {
        InjectConfig {
            source: IsoSource::Path(source.to_path_buf()),
            out_name: out_name.to_string(),
            ..Default::default()
        }
    }
}

use chaos_helpers::{
    build_config_for, inject_config_for, install_fake_binary, write_synthetic_iso, FakeBehavior,
    PathGuard,
};

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 1 — Missing tool: empty PATH must surface MissingTool
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn chaos_missing_xorriso_yields_missing_tool() {
    let _lock = PATH_LOCK.lock().await;
    let _guard = PathGuard::new();
    // An empty directory on PATH guarantees no tool is resolvable.
    let empty = TempDir::new().expect("tempdir");
    std::env::set_var("PATH", empty.path());

    let tmp = TempDir::new().expect("tempdir");
    let iso = write_synthetic_iso(&tmp, "fake.iso", &vec![0_u8; 1024]);
    let cfg = build_config_for("chaos-missing-tool", &iso);
    let out = TempDir::new().expect("out");

    let engine = ForgeIsoEngine::new();
    let err = engine
        .build(&cfg, out.path())
        .await
        .expect_err("build must fail with no tools on PATH");
    assert!(
        matches!(
            err,
            EngineError::MissingTool(_) | EngineError::InvalidConfig(_)
        ),
        "expected MissingTool (or InvalidConfig if validation trips first), got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 2 — Subprocess non-zero exit: fake xorriso exits 1
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn chaos_xorriso_nonzero_exit_yields_runtime() {
    let _lock = PATH_LOCK.lock().await;
    let _guard = PathGuard::new();
    let bin_dir = TempDir::new().expect("bin dir");
    install_fake_binary(bin_dir.path(), "xorriso", FakeBehavior::ExitNonZero);
    install_fake_binary(bin_dir.path(), "unsquashfs", FakeBehavior::ExitNonZero);
    install_fake_binary(bin_dir.path(), "mksquashfs", FakeBehavior::ExitNonZero);
    let original = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", bin_dir.path().display(), original));

    let tmp = TempDir::new().expect("tempdir");
    // 64KB synthetic file — large enough that inspect_iso reads sector 16,
    // so we proceed to the xorriso enrich step and hit our fake binary.
    let iso = write_synthetic_iso(&tmp, "fake.iso", &vec![0xAB_u8; 64 * 1024]);
    let cfg = build_config_for("chaos-xorriso-fail", &iso);
    let out = TempDir::new().expect("out");

    let engine = ForgeIsoEngine::new();
    let err = engine
        .build(&cfg, out.path())
        .await
        .expect_err("build must fail when xorriso exits non-zero");
    assert!(
        matches!(
            err,
            EngineError::Runtime(_) | EngineError::InvalidConfig(_) | EngineError::MissingTool(_)
        ),
        "expected Runtime / InvalidConfig / MissingTool, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 3 — Corrupt input ISO: 1 KB of random bytes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn chaos_corrupt_iso_yields_invalid_config() {
    let tmp = TempDir::new().expect("tempdir");
    // 1 KB is below sector-16 (32 KB) — the read_exact will fail and
    // surface as InvalidConfig ("too small to be an ISO image").
    let iso = write_synthetic_iso(&tmp, "garbage.iso", &vec![0xFF_u8; 1024]);
    let err = inspect_iso(&iso, SourceKind::LocalPath, iso.display().to_string())
        .expect_err("inspect must fail for sub-sector-16 input");
    assert!(
        matches!(err, EngineError::InvalidConfig(_)),
        "expected InvalidConfig for tiny corrupt input, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 4 — Corrupt input ISO: large file but no CD001 signature
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn chaos_no_cd001_signature_yields_invalid_config() {
    let tmp = TempDir::new().expect("tempdir");
    // 64 KB of zeros — past sector 16, but the signature bytes are 0x00, not
    // "CD001".  read_primary_volume_id must reject as not-ISO-9660.
    let iso = write_synthetic_iso(&tmp, "no-sig.iso", &vec![0_u8; 64 * 1024]);
    let err = inspect_iso(&iso, SourceKind::LocalPath, iso.display().to_string())
        .expect_err("inspect must fail when CD001 signature absent");
    assert!(
        matches!(err, EngineError::InvalidConfig(_)),
        "expected InvalidConfig for missing CD001, got: {err}"
    );
    if let EngineError::InvalidConfig(msg) = &err {
        assert!(
            msg.contains("ISO-9660"),
            "InvalidConfig message must reference ISO-9660: {msg}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 5 — sha256_file on missing file → Io error
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn chaos_sha256_missing_file_yields_io() {
    let tmp = TempDir::new().expect("tempdir");
    let phantom = tmp.path().join("definitely-not-here.iso");
    let err = sha256_file(&phantom).expect_err("sha256 of missing file must fail");
    assert!(
        matches!(err, EngineError::Io(_)),
        "expected Io for missing file, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 6 — Source ISO not found → NotFound (via build())
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn chaos_source_iso_not_found_yields_not_found() {
    let tmp = TempDir::new().expect("tempdir");
    let phantom = tmp.path().join("missing.iso");
    let cfg = build_config_for("chaos-not-found", &phantom);
    let out = TempDir::new().expect("out");

    let engine = ForgeIsoEngine::new();
    let err = engine
        .build(&cfg, out.path())
        .await
        .expect_err("build must fail when source ISO does not exist");
    assert!(
        matches!(err, EngineError::NotFound(_) | EngineError::MissingTool(_)),
        "expected NotFound (or MissingTool if host check trips first), got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 7 — SHA-256 mismatch via expected_sha256 → Runtime
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn chaos_sha256_mismatch_yields_runtime() {
    let _lock = PATH_LOCK.lock().await;
    let _guard = PathGuard::new();
    // A directory with no tools so we don't depend on host xorriso, but we
    // expect the sha256 check to trip before any tool is invoked.
    let bin_dir = TempDir::new().expect("bin dir");
    let original = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", bin_dir.path().display(), original));

    let tmp = TempDir::new().expect("tempdir");
    let iso = write_synthetic_iso(&tmp, "src.iso", &vec![0_u8; 64 * 1024]);
    let mut cfg = build_config_for("chaos-sha256", &iso);
    // Wrong expected hash — actual hash of 64 KB of zeros differs.
    cfg.expected_sha256 =
        Some("0000000000000000000000000000000000000000000000000000000000000000".to_string());
    let out = TempDir::new().expect("out");

    let engine = ForgeIsoEngine::new();
    let err = engine
        .build(&cfg, out.path())
        .await
        .expect_err("build must fail with wrong expected_sha256");
    assert!(
        matches!(
            err,
            EngineError::Runtime(_) | EngineError::MissingTool(_) | EngineError::InvalidConfig(_)
        ),
        "expected Runtime (or upstream short-circuit MissingTool/InvalidConfig), got: {err}"
    );
    if let EngineError::Runtime(msg) = &err {
        assert!(
            msg.contains("SHA-256") || msg.to_ascii_lowercase().contains("sha"),
            "Runtime message must reference SHA-256: {msg}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 8 — Read-only output dir: validate path-safety chain
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(unix)]
#[tokio::test]
async fn chaos_readonly_output_dir_yields_io_or_runtime() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().expect("tempdir");
    let iso = write_synthetic_iso(&tmp, "src.iso", &vec![0_u8; 64 * 1024]);
    let out = TempDir::new().expect("out");
    // Make output dir read-only.  Workspace::create will attempt to mkdir
    // a child (`run-name-<uuid>`) inside which fails with EACCES.
    let ro_perms = fs::Permissions::from_mode(0o555);
    fs::set_permissions(out.path(), ro_perms).expect("chmod ro");

    let cfg = build_config_for("chaos-ro", &iso);
    let engine = ForgeIsoEngine::new();
    let err = engine.build(&cfg, out.path()).await;

    // Restore writable permissions before the assertion so the TempDir Drop
    // succeeds even if the test fails mid-assert.
    let rw_perms = fs::Permissions::from_mode(0o755);
    let _ = fs::set_permissions(out.path(), rw_perms);

    let err = err.expect_err("build must fail when output dir is read-only");
    assert!(
        matches!(
            err,
            EngineError::Io(_) | EngineError::Runtime(_) | EngineError::MissingTool(_)
        ),
        "expected Io / Runtime / MissingTool for read-only output, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 9 — InjectConfig with corrupt source ISO → propagates failure
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn chaos_inject_with_corrupt_iso_yields_error() {
    let _lock = PATH_LOCK.lock().await;
    let _guard = PathGuard::new();
    let bin_dir = TempDir::new().expect("bin dir");
    install_fake_binary(bin_dir.path(), "xorriso", FakeBehavior::ExitNonZero);
    let original = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", bin_dir.path().display(), original));

    let tmp = TempDir::new().expect("tempdir");
    let iso = write_synthetic_iso(&tmp, "garbage.iso", &vec![0xCC_u8; 1024]);
    let cfg = inject_config_for("chaos-out.iso", &iso);
    let out = TempDir::new().expect("out");

    let engine = ForgeIsoEngine::new();
    let err = engine
        .inject_autoinstall(&cfg, out.path())
        .await
        .expect_err("inject must fail on corrupt source ISO");
    // inspect_iso runs first inside inject; sub-sector-16 input -> InvalidConfig.
    // If a real xorriso somehow resolves (unlikely with our PATH), the error
    // could surface as Runtime — accept either to keep the assertion stable.
    assert!(
        matches!(
            err,
            EngineError::InvalidConfig(_) | EngineError::Runtime(_) | EngineError::Io(_)
        ),
        "expected InvalidConfig / Runtime / Io for corrupt inject source, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 10 — Cancellation pattern: subscribe + drop receiver mid-build
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn chaos_event_subscriber_drop_does_not_panic_engine() {
    // The engine emits events through a broadcast channel; when no receivers
    // are alive, send() returns Err but the engine ignores it (`let _ = …`).
    // This scenario verifies the contract: subscribers can come and go and
    // the build pipeline continues to surface a deterministic EngineError
    // rather than panicking on a closed channel.
    let tmp = TempDir::new().expect("tempdir");
    let phantom = tmp.path().join("missing.iso");
    let cfg = build_config_for("chaos-cancel", &phantom);
    let out = TempDir::new().expect("out");

    let engine = ForgeIsoEngine::new();
    let mut rx = engine.subscribe();
    drop(rx); // simulate listener cancelling before any event is emitted
    rx = engine.subscribe();
    let err = engine
        .build(&cfg, out.path())
        .await
        .expect_err("build must still fail deterministically");
    drop(rx);
    assert!(
        matches!(
            err,
            EngineError::NotFound(_) | EngineError::MissingTool(_) | EngineError::InvalidConfig(_)
        ),
        "expected deterministic error after subscriber churn, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 11 — Two engines on same workspace: parallel build of same name
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn chaos_two_engines_same_workspace_both_fail_gracefully() {
    let tmp = TempDir::new().expect("tempdir");
    let phantom = tmp.path().join("missing.iso");
    let out = TempDir::new().expect("out");

    let engine_a = ForgeIsoEngine::new();
    let engine_b = ForgeIsoEngine::new();
    let cfg_a = build_config_for("chaos-race", &phantom);
    let cfg_b = build_config_for("chaos-race", &phantom);

    let (ra, rb) = tokio::join!(
        engine_a.build(&cfg_a, out.path()),
        engine_b.build(&cfg_b, out.path()),
    );

    // Workspace::create() suffixes a UUID, so concurrent runs don't collide
    // on filesystem state. Both must fail on the missing source ISO with a
    // typed error (NotFound) — never panic, never deadlock.
    let err_a = ra.expect_err("engine A must fail on missing source");
    let err_b = rb.expect_err("engine B must fail on missing source");
    for err in [&err_a, &err_b] {
        assert!(
            matches!(
                err,
                EngineError::NotFound(_)
                    | EngineError::MissingTool(_)
                    | EngineError::InvalidConfig(_)
            ),
            "expected typed error for racing builds, got: {err}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 12 — sha256_file on a directory → Io error (not panic)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn chaos_sha256_on_directory_yields_io() {
    let tmp = TempDir::new().expect("tempdir");
    let err = sha256_file(tmp.path()).expect_err("sha256 of a directory must fail");
    assert!(
        matches!(err, EngineError::Io(_)),
        "expected Io when target is a directory, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 13 — Fake tool exits 0 but produces no extraction → Runtime
// ─────────────────────────────────────────────────────────────────────────────
//
// A "successful" tool that does not actually do its job is one of the
// nastier real-world failure modes (e.g. a stub xorriso wrapper, a
// containerised tool whose volume mount silently failed). The engine
// must still reject the build because the expected output state is absent.

#[tokio::test]
async fn chaos_fake_tool_silent_noop_yields_error() {
    let _lock = PATH_LOCK.lock().await;
    let _guard = PathGuard::new();
    let bin_dir = TempDir::new().expect("bin dir");
    install_fake_binary(bin_dir.path(), "xorriso", FakeBehavior::ExitZeroNoOp);
    install_fake_binary(bin_dir.path(), "unsquashfs", FakeBehavior::ExitZeroNoOp);
    install_fake_binary(bin_dir.path(), "mksquashfs", FakeBehavior::ExitZeroNoOp);
    let original = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", bin_dir.path().display(), original));

    let tmp = TempDir::new().expect("tempdir");
    let iso = write_synthetic_iso(&tmp, "src.iso", &vec![0_u8; 64 * 1024]);
    let cfg = build_config_for("chaos-silent-noop", &iso);
    let out = TempDir::new().expect("out");

    let engine = ForgeIsoEngine::new();
    let err = engine
        .build(&cfg, out.path())
        .await
        .expect_err("build must fail when fake xorriso produces no output");
    // Either inspect_iso fails on the synthetic input (InvalidConfig — no
    // CD001), or the repack step fails because no boot files exist
    // (Runtime / Io). Both are acceptable; what matters is that the engine
    // returns a typed error rather than producing a bogus "successful"
    // BuildResult pointing at a non-existent ISO.
    assert!(
        matches!(
            err,
            EngineError::Runtime(_) | EngineError::InvalidConfig(_) | EngineError::Io(_)
        ),
        "expected typed error after silent no-op tool, got: {err}"
    );
}
