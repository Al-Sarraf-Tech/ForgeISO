//! Public-API contract test for `forgeiso-engine`.
//!
//! Invokes `cargo public-api -p forgeiso-engine` as a subprocess, compares
//! the captured surface against the locked baseline at
//! `engine/tests/public-api.golden`, and fails if they differ.
//!
//! Failure means a public item was added, removed, or had its signature
//! changed. Such a change MUST be accompanied by an ADR (under
//! `docs/adr/`) explaining the rationale, alongside a refresh of the
//! golden via:
//!
//! ```bash
//! ./scripts/regenerate-api-golden.sh
//! ```
//!
//! The test is gated behind the `api-contract` feature flag of the
//! environment variable `FORGEISO_RUN_API_CONTRACT=1` to avoid forcing
//! every developer to install the nightly toolchain (cargo-public-api
//! requires nightly to emit rustdoc JSON). CI sets the environment
//! variable; the gate enforces the contract there.

use std::path::PathBuf;
use std::process::Command;

const GOLDEN_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/public-api.golden");

fn workspace_root() -> PathBuf {
    // engine/Cargo.toml lives one level under the workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("engine crate must have a parent (workspace root)")
        .to_path_buf()
}

fn capture_current_api() -> Result<String, String> {
    let output = Command::new("cargo")
        .args(["public-api", "-p", "forgeiso-engine"])
        .current_dir(workspace_root())
        .output()
        .map_err(|e| {
            format!(
                "failed to spawn `cargo public-api -p forgeiso-engine`: {e}. \
                 Install via `cargo install cargo-public-api --locked` and ensure \
                 the nightly toolchain is available (`rustup toolchain install nightly`)."
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "`cargo public-api` exited with status {:?}\n--- stderr ---\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8(output.stdout)
        .map_err(|e| format!("non-UTF8 output from cargo public-api: {e}"))
}

fn normalize(s: &str) -> String {
    // Strip trailing whitespace per line, drop empty trailing lines.
    let mut out: Vec<&str> = s.lines().map(str::trim_end).collect();
    while matches!(out.last(), Some(&"")) {
        out.pop();
    }
    out.join("\n")
}

#[test]
fn engine_public_api_matches_golden() {
    if std::env::var("FORGEISO_RUN_API_CONTRACT").ok().as_deref() != Some("1") {
        eprintln!(
            "skipping engine_public_api_matches_golden (set FORGEISO_RUN_API_CONTRACT=1 to enable). \
             Requires `cargo install cargo-public-api --locked` and the nightly toolchain."
        );
        return;
    }

    let golden = std::fs::read_to_string(GOLDEN_PATH).unwrap_or_else(|e| {
        panic!(
            "failed to read golden file at {GOLDEN_PATH}: {e}. \
             Generate it via `./scripts/regenerate-api-golden.sh`."
        )
    });

    let current = match capture_current_api() {
        Ok(s) => s,
        Err(e) => panic!("could not capture current public API: {e}"),
    };

    let golden_n = normalize(&golden);
    let current_n = normalize(&current);

    if golden_n == current_n {
        return;
    }

    // Compute a small diff summary so the failure message is actionable.
    let golden_lines: std::collections::BTreeSet<&str> = golden_n.lines().collect();
    let current_lines: std::collections::BTreeSet<&str> = current_n.lines().collect();

    let added: Vec<&&str> = current_lines.difference(&golden_lines).collect();
    let removed: Vec<&&str> = golden_lines.difference(&current_lines).collect();

    let preview = |label: &str, items: &[&&str]| {
        let mut buf = format!("\n{label} ({} item(s)):\n", items.len());
        for item in items.iter().take(20) {
            buf.push_str("  ");
            buf.push_str(item);
            buf.push('\n');
        }
        if items.len() > 20 {
            buf.push_str(&format!("  ... and {} more\n", items.len() - 20));
        }
        buf
    };

    let added_str = preview("ADDED", &added);
    let removed_str = preview("REMOVED", &removed);

    panic!(
        "Engine public API changed. If intentional, regenerate golden via \
         `cargo public-api -p forgeiso-engine > engine/tests/public-api.golden` \
         (or run `./scripts/regenerate-api-golden.sh`) and write an ADR \
         under docs/adr/ explaining the change.{added_str}{removed_str}"
    );
}
