# Spec 0001: CancellationToken plumbing through engine orchestrator

- **Status**: Accepted (in flight, Phase 8a)
- **Date**: 2026-05-01
- **Pilots**: ADR 0009 (spec-driven development workflow)
- **Related**: ADR 0008 (reliability contract — cancel ≤1s guarantee)

## What

Thread a `tokio_util::sync::CancellationToken` through every engine
operation that shells out, plus add a `CircuitBreaker` abstraction per
shell-out tool. Surface new `EngineError::Cancelled` and
`EngineError::CircuitOpen { tool }` variants. Cover both paths in
`engine/tests/chaos.rs`.

## Why

ADR 0008 commits ForgeISO to "Cancellation honored within 1 second" and
"graceful shell-out failure". The current engine has no cancel channel
— operations only stop when their tokio task is dropped, which is racy
and doesn't propagate cleanly to subprocesses. The chaos suite documented
this as `out-of-scope` because the plumbing was missing. This spec
closes the gap between ADR 0008's contract and the actual implementation.

## Scope

- `engine/src/orchestrator/helpers/process.rs`: optional
  `CancellationToken` parameter on `run_command_capture*` functions;
  `tokio::select!` between command future and `cancel.cancelled()`;
  SIGTERM then SIGKILL after 5s timeout per ADR 0008.
- `engine/src/orchestrator/circuit_breaker.rs` (new): per-tool sliding-
  window failure counter; states Closed → Open → HalfOpen; `allow_call()`
  returns `Err(CircuitOpen)` when open.
- `engine/src/error.rs`: add `Cancelled` and `CircuitOpen { tool }` variants.
- `engine/Cargo.toml`: add `tokio-util = { version = "0.7", features = ["sync"] }`.
- `engine/tests/chaos.rs`: add `cancel_mid_build` and
  `circuit_open_after_threshold` scenarios.
- `engine/tests/public-api.golden`: regenerate via
  `scripts/regenerate-api-golden.sh` to capture new variants and signatures.
- `docs/CHAOS.md`: move the deferred items (cancel + circuit) out of
  "Out of scope" into "Covered scenarios".

## Non-goals

- Wider rollout of circuit breakers to every shell-out (xorriso,
  qemu-img, mtools). Phase 8a wires ONE shell-out (mksquashfs) as
  proof-of-concept; remaining call-sites are tracked as follow-ups.
- Configurable circuit-breaker thresholds via CLI/GUI flags. Defaults
  only for now (10-call sliding window, 5 failures opens, 60s half-open
  timeout).
- Auto-rollback on cancel — the build aborts, the partial output stays
  in the workspace tmp dir until the next `forgeiso doctor --repair`
  (separate spec).

## User-visible API

CLI: no new flags. Cancel is via `Ctrl-C` as today; the engine now
propagates the cancel cleanly instead of hard-killing tokio tasks.

GUI: cancel button already exists. Engine propagation makes the button
feel responsive (≤1s vs current best-effort).

Engine library:

```rust
// New, additive — existing call-sites compile unchanged.
pub async fn run_command_capture_async(
    cmd: &mut Command,
    cancel: Option<CancellationToken>,  // None = legacy behavior
) -> Result<Output, EngineError> { ... }

// New module:
pub mod circuit_breaker {
    pub struct CircuitBreaker { /* per-tool */ }
    impl CircuitBreaker {
        pub fn new(tool: &str, window: usize, threshold: usize) -> Self;
        pub fn allow_call(&self) -> Result<(), EngineError>;
        pub fn record_success(&self);
        pub fn record_failure(&self);
    }
}

// New EngineError variants:
EngineError::Cancelled
EngineError::CircuitOpen { tool: String }
```

## Error model

Two new variants, each gets a runbook entry in `docs/RUNBOOKS.md`:

- `Cancelled` (new code: E110): symptom = build aborted with "Cancelled"
  stderr; cause = operator clicked cancel or sent SIGINT; diagnose = N/A
  (intentional); recovery = re-launch.
- `CircuitOpen { tool }` (new code: E111): symptom = "circuit open for
  <tool>"; cause = tool failed >threshold times in sliding window;
  diagnose = run `forgeiso doctor` to check tool health; recovery = wait
  60s for half-open + retry, or `forgeiso doctor --repair`.

## Observability

New fields per ADR 0011:

- Cancel emit: `op = "cancel"`, `latency_ms`, `phase`.
- Circuit-open emit: `op = "circuit_open"`, `tool`,
  `failures_in_window`, `window_size`.

OTel span (when `--features otel`): `cancel` and `circuit_open` are
events on the parent phase span.

## Test plan

- 2 new chaos scenarios in `engine/tests/chaos.rs`:
  - `cancel_mid_build_returns_within_one_second`: spawn fake binary
    that sleeps 30s, cancel after 100ms, assert `EngineError::Cancelled`
    in ≤1s.
  - `circuit_open_after_5_failures`: trigger 5 failed calls via fake-
    failing binary, assert 6th call returns
    `EngineError::CircuitOpen { tool: "mksquashfs" }` without invoking
    the subprocess.
- Unit tests in `engine/src/orchestrator/circuit_breaker.rs`: state
  transitions Closed → Open, Open → HalfOpen, HalfOpen → Closed/Open.
- Property test in `engine/tests/proptest_config.rs`:
  `CircuitBreaker::record_*` calls in any sequence preserve invariant
  `failure_count <= window_size`.
- Public API contract: regenerate golden after new variants land.

## Rollout

Pilot-first per ADR 0009:

1. Phase 8a commits land the plumbing + ONE shell-out (mksquashfs).
2. Soak for one release; collect chaos-scenario results from CI.
3. Next release: wire xorriso + qemu-img + mtools through the breaker.
4. Following release: extend cancel plumbing to all async tasks
   (download, sha256-of-large-file). Update CHAOS.md to remove the
   remaining "out-of-scope" items.

## Implementation reference

Phase 8a, branch `refactor/major-gui-overhaul`. Commits TBD (agent in
flight). Pilots the ADR 0009 workflow:

- Brainstorm (this spec's scope section) ✅
- Spec (this document) ✅
- ADR — none required; this spec implements ADR 0008 + ADR 0011 contracts
- Plan — embedded in the YOUR TASK section of the agent prompt
- Review — Phase 8a agent reads this spec implicitly; future PRs cite it
- Implement — Phase 8a agent in flight ✅

## Open questions

- Should `CircuitBreaker` survive across CLI invocations (persist state
  to `$XDG_DATA_HOME/forgeiso/circuit-state.json`)? Today: no. Next
  spec if operators report needing cross-run persistence.
- Should the chaos test use real `tokio_util::time::DelayQueue` for the
  half-open timer or fast-forward via `tokio::time::pause`?
  Recommendation: `pause` for test speed; agent's choice.
