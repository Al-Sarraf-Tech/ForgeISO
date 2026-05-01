# ForgeISO Chaos / Fault-Injection Suite

The chaos suite (`engine/tests/chaos.rs`) verifies that the `forgeiso-engine`
returns a typed [`EngineError`](../engine/src/error.rs) variant for every
failure mode it can encounter. It does NOT call real `xorriso`, `qemu`,
`squashfs`, `mtools`, or the network — every scenario is hermetic, runs in
under one second per case, and is safe to execute on a developer laptop or
in CI without root, kvm, or sudo.

The suite complements [`docs/RUNBOOKS.md`](RUNBOOKS.md): runbooks describe
what an operator does when an error code surfaces in production; chaos
tests guarantee the engine actually emits that error code instead of
panicking, hanging, or producing a corrupt build artefact.

---

## How to run

```bash
# Just the chaos scenarios (fast — ~1 second total)
cargo test -p forgeiso-engine --test chaos

# As part of the standard CI gate
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The suite has no environment dependencies beyond a writeable temp dir and
`bash` on the test runner. It serializes scenarios that mutate `$PATH`
through an internal `tokio::sync::Mutex` (`PATH_LOCK`) so concurrent test
threads do not race; non-PATH scenarios run in parallel with the rest of
the integration suite.

---

## Scenario inventory

Each scenario asserts the engine returns a specific [`EngineError`] variant.
Where multiple variants are accepted in `matches!`, the comment explains why
(short-circuit at an earlier validation layer is acceptable as long as the
result is still typed and deterministic).

| # | Test name | Failure injected | Asserted variant |
|---|-----------|------------------|------------------|
| 1 | `chaos_missing_xorriso_yields_missing_tool` | `$PATH` set to an empty dir | `MissingTool` (or upstream `InvalidConfig`) |
| 2 | `chaos_xorriso_nonzero_exit_yields_runtime` | Fake `xorriso`/`unsquashfs`/`mksquashfs` exit 1 | `Runtime` / `InvalidConfig` / `MissingTool` |
| 3 | `chaos_corrupt_iso_yields_invalid_config` | 1 KB random-byte file as source | `InvalidConfig` ("too small to be an ISO image") |
| 4 | `chaos_no_cd001_signature_yields_invalid_config` | 64 KB of zeros — past sector 16 but no `CD001` signature | `InvalidConfig` ("not an ISO-9660 image") |
| 5 | `chaos_sha256_missing_file_yields_io` | `sha256_file` on non-existent path | `Io` (`std::io::ErrorKind::NotFound`) |
| 6 | `chaos_source_iso_not_found_yields_not_found` | `BuildConfig.source` references missing file | `NotFound` / `MissingTool` |
| 7 | `chaos_sha256_mismatch_yields_runtime` | `expected_sha256` does not match actual contents | `Runtime` (message contains "SHA-256") |
| 8 | `chaos_readonly_output_dir_yields_io_or_runtime` | Output directory chmod 0555 (Unix only) | `Io` / `Runtime` / `MissingTool` |
| 9 | `chaos_inject_with_corrupt_iso_yields_error` | `inject_autoinstall` against 1 KB random file | `InvalidConfig` / `Runtime` / `Io` |
| 10 | `chaos_event_subscriber_drop_does_not_panic_engine` | Subscriber dropped before any event emitted | Deterministic `NotFound` / `MissingTool` / `InvalidConfig` |
| 11 | `chaos_two_engines_same_workspace_both_fail_gracefully` | Two engines run the same build name concurrently | Both fail with typed errors, no panic, no deadlock |
| 12 | `chaos_sha256_on_directory_yields_io` | `sha256_file(<dir>)` instead of file | `Io` |
| 13 | `chaos_fake_tool_silent_noop_yields_error` | Fake tool exits 0 without producing output | `Runtime` / `InvalidConfig` / `Io` |

---

## Helper module

`chaos.rs` defines a small `chaos_helpers` module with the fake-binary
harness:

```rust
mod chaos_helpers {
    pub enum FakeBehavior { ExitNonZero, ExitZeroNoOp }

    pub fn install_fake_binary(dir: &Path, name: &str, behavior: FakeBehavior) -> PathBuf;
    pub struct PathGuard { /* restores PATH on drop */ }
    pub fn write_synthetic_iso(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf;
    pub fn build_config_for(name: &str, source: &Path) -> BuildConfig;
    pub fn inject_config_for(out_name: &str, source: &Path) -> InjectConfig;
}
```

`PathGuard` saves `$PATH` on construction and restores it on drop, so even
if a test panics the runner's environment is left clean for the next test.

---

## When to add a new scenario

Add a chaos test whenever any of the following ships:

- A new `EngineError` variant in `engine/src/error.rs`.
- A new shell-out from the orchestrator (`engine/src/orchestrator/`) — even
  if the existing variants are reused, the new call site has its own
  failure surface.
- A new `validate()` method on a config struct that gates the engine
  pipeline.
- A new public async method on `ForgeIsoEngine` that can be triggered from
  the CLI/TUI/GUI without intermediate validation.
- A reproducer for any production incident where the engine surfaced a
  panic, deadlock, infinite loop, or untyped error.

Each new test should:

1. Fit in 10–30 lines including assertions.
2. Not call real ISO-handling tools or the network.
3. Hold `PATH_LOCK` if and only if it mutates `$PATH`.
4. Assert with `matches!` (not `==`) so the variant payload can drift
   without breaking the test, and include the actual error in the
   `assert!` message via `{err}` for fast triage on regression.
5. Include a comment explaining which production failure mode it models.

---

## Out-of-scope (for now)

Scenarios that would be valuable but are not in the current suite, with
the reason each was deferred:

- **True subprocess timeout (kill after N seconds)** — the engine's
  `run_command_capture_async` does not currently take a timeout argument;
  the OS-level deadline would have to be wired into the helper before a
  meaningful test can be written. Tracked separately.
- **Interrupting an in-flight `inject_autoinstall`** — the engine has no
  cancel-channel API on `ForgeIsoEngine` today; the broadcast subscriber
  is event-only, not control-plane. A `CancellationToken` would need to be
  threaded through the pipeline first.
- **Real `xorriso` returning a partially-written ISO** — would require a
  fixture ISO and is better covered by `e2e_regression.rs` once a test
  fixture set is in tree.

These belong in follow-up work that touches the engine's process-execution
helpers, not the chaos suite itself.
