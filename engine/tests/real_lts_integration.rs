//! Real-upstream-LTS integration test.
//!
//! Exercises the full `ForgeIsoEngine::build` pipeline against an actual
//! Ubuntu Server LTS ISO (currently 24.04.4 / Noble Numbat). Unlike the
//! synthetic-ISO test matrix in `scripts/test-releases.sh`, this test
//! takes the real-world path through `xorriso` extract, autoinstall
//! injection, `unsquashfs` / `mksquashfs` rootfs handling, and final
//! ISO repack.
//!
//! ## Why
//!
//! The synthetic ISOs lack a populated `casper/filesystem.squashfs` (or
//! ship a 4-file stub since v0.3.2), so they exercise only the
//! "warn-only, rootfs not modified" branch of the engine. The real LTS
//! ISO drives the unsquashfs/mksquashfs path end-to-end, which is the
//! one a real user actually hits in production.
//!
//! ## How to run
//!
//! Two-layer gate so this never runs accidentally in CI or on a dev
//! laptop without explicit consent:
//!
//! 1. The test is annotated `#[ignore]`, so `cargo test` skips it. Run
//!    with `cargo test --workspace -- --ignored real_lts_integration`.
//! 2. The test additionally requires `FORGEISO_RUN_REAL_LTS=1` in the
//!    environment. If unset, the test exits early with a SKIP message.
//!
//! ## Inputs
//!
//! The cached ISO must be at one of:
//!
//! - `$FORGEISO_LTS_CACHE_DIR/ubuntu-24.04.4-live-server-amd64.iso`
//! - `$HOME/.cache/forgeiso/ubuntu-24.04.4-live-server-amd64.iso`
//!
//! If both are missing, the test prints a SKIP message with the
//! download command and exits with a passing status. Runs in CI that
//! want a hard fail on missing input should use `--exact` plus a
//! pre-flight script that downloads the ISO via
//! `tests/fixtures/download-real-lts.sh`.
//!
//! ## Pinned checksum
//!
//! The cached ISO is verified against `PINNED_LTS_SHA256` before the
//! build runs. A mismatch fails the test loudly — there is no silent
//! "trust whatever is on disk" path. To rotate the LTS version, update
//! `PINNED_LTS_FILENAME`, `PINNED_LTS_SHA256`, and the corresponding
//! line in `tests/fixtures/download-real-lts.sh` in the same commit.

use std::path::{Path, PathBuf};
use std::time::Instant;

use forgeiso_engine::{
    BuildConfig, ForgeIsoEngine, IsoSource, ProfileKind, ScanPolicy, TestingPolicy,
};

/// Filename of the pinned LTS ISO. Filename is included in the cache
/// path lookup (see [`locate_cached_iso`]).
const PINNED_LTS_FILENAME: &str = "ubuntu-24.04.4-live-server-amd64.iso";

/// SHA-256 of the pinned LTS ISO. Computed locally on amarillo
/// 2026-05-02; matches the published Ubuntu 24.04.4 server live
/// installer hash on releases.ubuntu.com.
const PINNED_LTS_SHA256: &str = "e907d92eeec9df64163a7e454cbc8d7755e8ddc7ed42f99dbc80c40f1a138433";

/// Environment variable that gates this test. Set to `1` to opt in.
const RUN_GATE_VAR: &str = "FORGEISO_RUN_REAL_LTS";

/// Optional override for the cache directory. If unset, the test falls
/// back to `$HOME/.cache/forgeiso/`.
const CACHE_DIR_VAR: &str = "FORGEISO_LTS_CACHE_DIR";

/// Locate the cached ISO. Returns `Some(path)` if a candidate file is
/// found at one of the documented paths and `None` otherwise. Existence
/// is checked but the SHA-256 is verified separately by the caller.
fn locate_cached_iso() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var(CACHE_DIR_VAR) {
        let candidate = Path::new(&dir).join(PINNED_LTS_FILENAME);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let home = std::env::var("HOME").ok()?;
    let candidate = Path::new(&home)
        .join(".cache")
        .join("forgeiso")
        .join(PINNED_LTS_FILENAME);
    if candidate.is_file() {
        return Some(candidate);
    }
    None
}

/// Compute SHA-256 of the file at `path` using the engine's own
/// streaming hasher (the same one production code uses). Returns the
/// hex-encoded digest.
fn sha256_hex(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    use std::fs::File;
    use std::io::{BufReader, Read};

    let f = File::open(path).expect("open ISO for hashing");
    let mut reader = BufReader::with_capacity(1 << 20, f);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = reader.read(&mut buf).expect("read ISO");
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    format!("{:x}", hasher.finalize())
}

/// Read the first 5 bytes at offset `0x8001` of the ISO and assert the
/// `CD001` ISO-9660 volume descriptor magic.
fn assert_iso9660_magic(path: &Path) {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let mut f = File::open(path).expect("open output ISO");
    f.seek(SeekFrom::Start(0x8001))
        .expect("seek to volume descriptor");
    let mut magic = [0u8; 5];
    f.read_exact(&mut magic).expect("read magic");
    assert_eq!(
        &magic,
        b"CD001",
        "output ISO at {} is missing the ISO-9660 CD001 magic",
        path.display()
    );
}

#[tokio::test]
#[ignore = "real-LTS test — opt in with FORGEISO_RUN_REAL_LTS=1 and `cargo test -- --ignored`"]
async fn ubuntu_24_04_4_server_lts_round_trip() {
    if std::env::var(RUN_GATE_VAR).ok().as_deref() != Some("1") {
        eprintln!(
            "[real_lts_integration] SKIP: {}=1 not set in environment.",
            RUN_GATE_VAR
        );
        return;
    }

    let cached = match locate_cached_iso() {
        Some(p) => p,
        None => {
            eprintln!(
                "[real_lts_integration] SKIP: cached ISO {} not found at \
                 ${CACHE_DIR_VAR}/ or $HOME/.cache/forgeiso/. Download via \
                 `bash tests/fixtures/download-real-lts.sh` to populate the \
                 cache before re-running.",
                PINNED_LTS_FILENAME,
                CACHE_DIR_VAR = CACHE_DIR_VAR,
            );
            return;
        }
    };

    eprintln!(
        "[real_lts_integration] cached ISO: {} ({} bytes)",
        cached.display(),
        std::fs::metadata(&cached).expect("stat cached ISO").len()
    );

    let actual_sha = sha256_hex(&cached);
    assert_eq!(
        actual_sha, PINNED_LTS_SHA256,
        "cached ISO sha256 mismatch — refuse to run integration test against \
         an ISO that does not match the pinned upstream hash. Expected {}, \
         got {}.",
        PINNED_LTS_SHA256, actual_sha,
    );
    eprintln!("[real_lts_integration] sha256 verified against pinned upstream");

    let tmp = tempfile::tempdir().expect("create tempdir for output");
    let out_dir = tmp.path().to_path_buf();

    let cfg = BuildConfig {
        name: "real-lts-roundtrip".to_string(),
        source: IsoSource::from_raw(cached.display().to_string()),
        overlay_dir: None,
        output_label: Some("FORGEISO-RLTS".to_string()),
        profile: ProfileKind::Minimal,
        auto_scan: false,
        auto_test: false,
        scanning: ScanPolicy::default(),
        testing: TestingPolicy::default(),
        keep_workdir: false,
        expected_sha256: Some(PINNED_LTS_SHA256.to_string()),
    };

    eprintln!(
        "[real_lts_integration] starting engine build into {}",
        out_dir.display()
    );
    let started = Instant::now();
    let engine = ForgeIsoEngine::new();
    let result = engine
        .build(&cfg, &out_dir)
        .await
        .expect("real-LTS build must succeed");
    let elapsed = started.elapsed();
    eprintln!(
        "[real_lts_integration] build completed in {:.1}s",
        elapsed.as_secs_f64()
    );

    // ── Output assertions ────────────────────────────────────────────────────
    assert!(
        result.output_dir.is_dir(),
        "output_dir must exist: {}",
        result.output_dir.display()
    );
    assert!(
        result.report_json.is_file(),
        "report_json must exist: {}",
        result.report_json.display()
    );
    assert!(
        !result.artifacts.is_empty(),
        "build must produce at least one artifact"
    );

    // The output ISO is the artifact whose name ends with .iso. There may
    // be additional artifacts (manifest.json, report.html) alongside it.
    let output_iso = result
        .artifacts
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "iso"))
        .unwrap_or_else(|| {
            panic!(
                "no .iso artifact in build result; got: {:?}",
                result.artifacts
            )
        });
    assert!(
        output_iso.is_file(),
        "output ISO does not exist on disk: {}",
        output_iso.display()
    );

    let out_size = std::fs::metadata(output_iso)
        .expect("stat output ISO")
        .len();
    assert!(
        out_size > 1_000_000_000,
        "output ISO is implausibly small ({} bytes); the unsquashfs/mksquashfs \
         path likely failed silently",
        out_size
    );
    eprintln!(
        "[real_lts_integration] output ISO {} ({} bytes)",
        output_iso.display(),
        out_size
    );

    assert_iso9660_magic(output_iso);
    eprintln!("[real_lts_integration] CD001 ISO-9660 magic verified");
}
