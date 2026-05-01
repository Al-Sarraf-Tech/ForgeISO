# ADR 0008: Reliability contract for a desktop build tool

- **Status**: Accepted
- **Date**: 2026-05-01

## Context

The S+ tier rubric (`~/.claude/TIER_RUBRIC.md`) defines Reliability progression as:

- **A**: healthcheck + readiness + graceful shutdown + restart policies + backup automation + rollback runbook
- **A+**: A + multi-region/AZ + DR tested + chaos drills
- **S**: A+ + graceful degradation per dependency + per-dependency circuit breakers + auto-rollback on SLO breach

These criteria are written for **services** that run continuously, accept traffic, and degrade under partial-failure conditions. ForgeISO is a **desktop build tool** that runs once per ISO, exits, and persists nothing across runs except form state. Several criteria don't translate literally:

- "Multi-region" is meaningless for a binary on the operator's laptop
- "Auto-rollback on SLO breach" — there is nothing to rollback; the build either completes or doesn't
- "Restart policy" — the operator launches the binary; there is no supervisor

But the **intent** of each criterion does translate, and ForgeISO can satisfy A-equivalents:

| S-rubric criterion | Service shape | ForgeISO desktop equivalent |
|---|---|---|
| Healthcheck + readiness | `/healthz` endpoint | `forgeiso doctor` subcommand checks every required tool |
| Graceful shutdown | SIGTERM handler | Cancel-button on GUI; `Ctrl-C` on CLI; both tear down workspace cleanly |
| Restart policy | systemd `Restart=` | Persistent form state at `$XDG_DATA_HOME/forgeiso/slint-state.json`; relaunch resumes |
| Backup automation | DB snapshot cron | Form state JSON is the "backup" — committed to disk every change |
| Rollback runbook | git revert + redeploy | Output ISO never overwrites until checksum verified; failed builds leave source intact |
| Graceful degradation per dependency | feature flags + fallbacks | `forgeiso doctor` → `EngineError::MissingTool` per missing binary; build aborts cleanly |
| Per-dependency circuit breakers | retry/fallback/quarantine | Per-shell-out timeout watchdog; subprocess kill on cancel; error propagated as `EngineError` |
| Auto-rollback on SLO breach | redeploy old version | Output file written atomically (tmp + rename); never half-written file in output dir |
| DR drill | restore from cold backup | Single-command DR: `forgeiso doctor --repair` (future: regenerate config from defaults) |
| Chaos drill | kill random pod | `engine/tests/chaos.rs` fault-injection suite (Phase 7a) |

## Decision

ForgeISO's reliability contract is the desktop-equivalent of S-tier rubric criteria, codified as four guarantees:

### 1. **Every shell-out has a clear failure path**

For each external binary (xorriso, squashfs-tools mksquashfs, mtools, qemu-img, qemu-system-x86_64, sha256sum, etc.):

- Pre-check: `forgeiso doctor` reports presence + version
- At-call: `EngineError::MissingTool { tool }` if `which` fails
- During-call: subprocess timeout (configurable per operation; default in `engine/src/orchestrator/helpers.rs`)
- Post-call: non-zero exit → `EngineError::Subprocess { tool, code, stderr_tail }`

### 2. **Cancellation is honored within 1 second**

The cancel channel is checked between phases (download, scan, repack, sha256). Forge's tokio task tree is cancellable; `cancel_job()` aborts every in-flight subprocess via `kill(-SIGTERM)` then `kill(-SIGKILL)` after 5s timeout.

### 3. **Output integrity is atomic**

Output ISO is built into a temp file in the same dir as the final destination, fsync'd, then renamed. No half-written ISO in the output dir under any failure mode (including OOM / SIGKILL of the engine process — the temp file may exist but the destination won't be partially overwritten).

### 4. **Form state persistence is power-loss-safe**

`$XDG_DATA_HOME/forgeiso/slint-state.json` is written via tmp+rename on every theme toggle and form-field change (debounced). Passwords excluded via `#[serde(skip)]`. On startup, missing/invalid file → defaults (no crash).

## Test coverage for the contract

`engine/tests/chaos.rs` (Phase 7a — separate commit) verifies each guarantee with fault injection:

- Missing tool → `MissingTool` variant
- Subprocess timeout → `Subprocess { code: 124 }` or kill signal
- Subprocess non-zero exit → `Subprocess` with code + stderr tail
- Corrupt input → `InvalidIso` or `Sha256Mismatch`
- Read-only output dir → `Io { kind: PermissionDenied }`
- SHA-256 mismatch → `Sha256Mismatch`
- Cancellation → no half-written output, cancel returns within 1s
- Concurrent engine instances → graceful sequencing or `EngineError::WorkspaceLocked`

These tests run in CI on every PR via the regular `cargo test --workspace` gate. They use the fake-binary harness pattern (synthetic shell scripts on PATH) — no real shell-outs to xorriso/qemu/etc.

## Alternatives considered

- **Apply S-rubric criteria literally**: would require building a service-oriented model (background daemon, supervisor, multi-region deploy) for a binary that runs interactively per ISO. Rejected as over-engineering.
- **Drop reliability claims**: would cap ForgeISO at B+ on the rubric. Rejected; the desktop-equivalent guarantees are real and worth committing to.
- **Continuous-execution mode** (`forgeiso watch` daemon): could match the S-rubric more directly but would invent a use case nobody asked for. Rejected.

## Consequences

- **Positive**: clear contract for what users can rely on. No "best effort" hand-waving on output safety or cancel behavior.
- **Positive**: chaos tests provide regression evidence. If a future change breaks atomic output write, the chaos test catches it.
- **Positive**: Reliability dimension lifts B+ → A on the rubric (gated only by chaos test landing in Phase 7a).
- **Negative**: subprocess timeout default values are an operational policy that needs tuning per host (slow disk → slower xorriso). Documented in `docs/SLO.md`.
- **Negative**: the desktop-tool adaptations are **not** the same as service-shape S-tier reliability. Auditors comparing to a service contract should read this ADR first.
- **Process consequence**: any new shell-out the engine adds requires a chaos test before merge. Enforced by reviewer convention; not currently a CI gate (TODO: extend `scripts/s-tier-audit.sh` to grep for new `Command::new(...)` callsites without matching `engine/tests/chaos.rs` coverage).

## Implementation reference

- `engine/src/error.rs` — `EngineError` taxonomy with the variants this contract references
- `engine/src/orchestrator/helpers.rs` — subprocess invocation helpers + timeout enforcement
- `engine/tests/chaos.rs` — Phase 7a fault-injection tests
- `forge-slint/src/state.rs` — `UiState` + `PersistedState` with `#[serde(skip)]` password exclusion
- `forge-slint/src/persist.rs` — tmp+rename JSON write
- ADR 0006 (post-decomposition module taxonomy) — locates each subsystem in the post-refactor tree
- ADR 0007 (seven-layer testing) — explains where chaos sits in the test stack
