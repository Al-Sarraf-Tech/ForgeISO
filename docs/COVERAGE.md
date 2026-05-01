# Code Coverage Gate — ForgeISO

ForgeISO uses [`cargo-tarpaulin`](https://github.com/xd009642/tarpaulin) as
the workspace coverage tool. Configuration lives in
[`tarpaulin.toml`](../tarpaulin.toml); it is auto-discovered when running
`cargo tarpaulin` from the workspace root.

## Threshold

```toml
# tarpaulin.toml
fail-under = 80
```

A run whose measured coverage falls below the floor exits with a non-zero
status, failing the lint job. The threshold is a **regression floor**, not
a target; it represents the lowest acceptable coverage for the *measured
surface* (the engine library minus the binary-only entry points listed
under `exclude-files`).

### Ratchet history

| Date          | Floor | Why it changed |
|---------------|-------|----------------|
| 2026-04-15    |  40   | Initial commit, no test inventory yet. |
| 2026-05-01 AM |  64   | After the inject/autoinstall test sweep added 100+ kill tests. |
| 2026-05-01 PM |  80   | Excluded the CLI dispatch layer (engine logic is what we measure) and added `engine/tests/orchestrator_coverage.rs` to lift `iso.rs` + `orchestrator/{report,mod}.rs` from ~40% to ~80%. |

To raise the bar, edit `fail-under` in `tarpaulin.toml`, run
`cargo tarpaulin --workspace --skip-clean --out Stdout` to verify the new
floor is achievable, and commit with an ADR if the bump is more than
five points.

## Excluded surfaces (and why)

`tarpaulin.toml` lists every excluded path under `exclude-files`. Each
exclusion has a single justification: **tarpaulin cannot reach this
code without expensive fixtures, and the underlying logic is exercised
by another test layer**.

| Path                                 | Why excluded |
|--------------------------------------|--------------|
| `cli/src/main.rs`                    | Argparse entry-point; needs a TTY and a process to instrument. |
| `cli/src/obs.rs`                     | OTLP/log-output side-effects; tested via `engine::observability` doctests. |
| `cli/src/dispatch.rs`                | Match-on-subcommand dispatcher; every branch calls into a covered engine method. |
| `cli/src/output.rs`                  | Pretty-printer for engine result types; failure mode is "ugly text", not a behaviour. |
| `cli/src/handlers/*`                 | Thin wrappers — each `handle()` builds a config struct and delegates to the engine. The engine call is covered; the wrapping is mechanical. |
| `tui/*`                              | Ratatui event loop — tarpaulin cannot instrument the input/render thread without a PTY. |
| `forge-slint/*`                      | Slint codegen produces a generated `.rs` file tarpaulin treats as opaque. |
| `engine/tests/*`                     | Tarpaulin double-counts integration test files that it also instruments. |
| `engine/benches/*`                   | Criterion benches; not production code. |

## Pain points that prevent further ratcheting

These engine modules cannot reach 100% via unit tests because they shell
out to xorriso, qemu, or hit the network:

| Module                                         | Covered    | Blocker                              |
|------------------------------------------------|------------|--------------------------------------|
| `engine/src/orchestrator/build.rs`             | 24/122     | `xorriso -extract / -as mkisofs` pipeline |
| `engine/src/orchestrator/inject/mod.rs`        | 12/63      | Full `inject_autoinstall` requires a real source ISO |
| `engine/src/orchestrator/diff.rs`              | 15/68      | `get_iso_file_list` driver needs xorriso |
| `engine/src/orchestrator/scan_test.rs`         | 15/63      | qemu-system-x86_64 for the smoke-boot loop |
| `engine/src/orchestrator/verify.rs`            | 46/82      | Real ISO + network to releases.ubuntu.com |
| `engine/src/iso.rs::enrich_with_xorriso`       | partial    | Real ISO bytes + xorriso |

The measured-surface coverage already excludes the CLI dispatch layer
because the engine methods it calls are themselves covered. Lifting the
floor further requires either:

1. **Adding an end-to-end CI job** that runs against a stripped-down
   real ISO + xorriso + qemu (a separate workflow gated by available
   tooling) and merges its coverage into the headline number; or
2. **Refactoring the xorriso wrappers** behind a trait so a mock
   implementation can drive the high-level orchestration code in unit
   tests.

Both are tracked under the broader S+ uplift initiative (see
`docs/REPORT-CARD-2026-05-01.md`); neither is a same-PR change.

## How to Run

The coverage gate runs as part of CI:

```bash
cargo tarpaulin --workspace --skip-clean --out Stdout
```

For local debugging:

```bash
# Engine-only run (skips TUI/CLI; faster, useful when the broader workspace
# has unrelated compile errors):
cargo tarpaulin -p forgeiso-engine --skip-clean --out Stdout

# HTML report at target/tarpaulin/tarpaulin-report.html for line-level inspection:
cargo tarpaulin --workspace --skip-clean --out Html

# Just the modules below the gate (informational triage):
cargo tarpaulin --workspace --skip-clean --out Stdout 2>&1 \
  | awk '/^\|\|.*: [0-9]+\/[0-9]+ /{n=split($2,a,"/"); if (a[1]/a[2] < 0.6) print}'
```

## Operator Cheat-Sheet

```bash
# enforce the floor (CI does this)
cargo tarpaulin --workspace --skip-clean --out Stdout

# override the floor for a single run (e.g. after deleting a feature)
TARPAULIN_FAIL_UNDER=70 cargo tarpaulin --workspace --skip-clean --out Stdout

# add a new exclusion (e.g. a new generated module)
# 1. Add the glob to exclude-files in tarpaulin.toml
# 2. Re-run the gate, confirm the new module is missing from the report
# 3. Commit with a one-line note in this file's "Excluded surfaces" table
```

## Orchestrator Wiring

`.github/workflows/ci-rust.yml` is regenerated by
`haskell-ci-orchestrator`. The coverage gate runs as a dedicated job
that invokes `cargo tarpaulin --workspace --skip-clean --out Stdout` —
the binary reads `tarpaulin.toml` automatically. **No workflow changes
are needed for ratchet bumps**; updating `fail-under` in `tarpaulin.toml`
is sufficient.

If a future workflow regeneration ever drops the dedicated coverage job,
re-add it via the orchestrator's `coverage:` block; do not edit the
generated file directly.
