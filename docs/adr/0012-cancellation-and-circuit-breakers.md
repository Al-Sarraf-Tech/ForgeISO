# ADR 0012: Cooperative cancellation and per-tool circuit breakers

- **Status**: Accepted
- **Date**: 2026-05-01
- **Supersedes**: extends ADR 0008 (Reliability contract)

## Context

ADR 0008 codifies ForgeISO's reliability contract and lists "per-shell-out
timeout watchdog; subprocess kill on cancel" and "per-dependency circuit
breakers" as desktop-equivalents of S-tier criteria. The original
implementation satisfied the spirit but lacked two pieces of plumbing:

1. **No structured cancellation primitive.** `tokio::process::Child` was
   spawned via `tokio::task::spawn_blocking` wrapping a synchronous
   `std::process::Command::output()`. A caller wishing to cancel a build
   mid-flight could only drop the future, which abandons the blocking
   thread and leaves the subprocess running until it exits naturally.
   The subprocess was never killed; the typed error returned was a
   `Runtime` string rather than a distinct `Cancelled` variant.
2. **No first-class circuit breaker type.** Repeated shell-out failures
   were retried up to a fixed bound, amplifying pressure on an already
   suffering subsystem (disk full, hung child, kernel-driver flake).
   Operators had no in-process fail-fast layer in front of any tool.

Both gaps are reachable A → S+ uplift work without breaking the engine
public API.

## Decision

Introduce two orthogonal abstractions, additive to the existing API:

### 1. `CancellationToken` plumbing (`engine/src/orchestrator/helpers/process.rs`)

* Add `tokio-util = { version = "0.7", features = ["rt"] }` to the
  workspace. The `rt` feature is the gate for the `CancellationToken`
  type in tokio-util 0.7.x.
* Add cancellation-aware async runners
  (`run_command_capture_async_cancellable`,
  `run_command_lossy_async_cancellable`) that:
  - Spawn the subprocess via `tokio::process::Command` (not
    `spawn_blocking`).
  - Race the subprocess against the token via `tokio::select!`.
  - On token signal: `child.start_kill()` then `child.wait()` to reap;
    return `EngineError::Cancelled`.
  - When `cancel = None`: behaviour matches the previous implementation
    so existing call sites compile unchanged.
* Surface the token at the public-API level via two new methods:
  - `ForgeIsoEngine::build_cancellable(&self, cfg, out_dir, cancel)`
  - `ForgeIsoEngine::inject_autoinstall_cancellable(&self, cfg, out, cancel)`
  - The original `build` and `inject_autoinstall` delegate with
    `cancel = None`. No caller needs to update.

### 2. `CircuitBreaker` (`engine/src/orchestrator/circuit_breaker.rs`)

* New `CircuitBreaker` struct guards a single tool name. State machine
  is `Closed → Open → HalfOpen → {Closed, Open}` with:
  - Sliding window of recent call outcomes (default `window = 10`).
  - Failure threshold within the window (default `failure_threshold = 5`).
  - Reset timeout (default `30s`) before promoting `Open → HalfOpen`.
* Public methods: `new`, `with_defaults`, `tool`, `state`, `allow_call`,
  `record_success`, `record_failure`.
* `allow_call()` returns `EngineError::CircuitOpen { tool }` when the
  breaker is open and the reset timeout has not elapsed — the gate is
  `O(1)` and never invokes a subprocess.
* Thread-safe via `RwLock`; cheap to share via `Arc<CircuitBreaker>`.
* `mksquashfs` is the first wired call site (autoinstall rootfs repack).
  Wider rollout to xorriso, unsquashfs, qemu is incremental: each new
  call site needs its own breaker instance, and we add them as the
  metrics from the first integration justify.

### 3. `EngineError` variants

Two new variants surface preemption and short-circuit decisions to
callers:

```rust
EngineError::Cancelled
EngineError::CircuitOpen { tool: String }
```

Both implement `Display` with stable, greppable messages.

### 4. Chaos-test coverage

Add scenarios 14 and 15 to `engine/tests/chaos.rs`:

* `chaos_cancel_mid_build_yields_cancelled_within_one_second` — plants a
  fake `xorriso` that sleeps 60s, signals the token after 100ms, asserts
  the build returns within 2s and surfaces `Cancelled` (or an upstream
  short-circuit error).
* `chaos_circuit_open_after_threshold_short_circuits_without_subprocess`
  — drives a fresh breaker through 10 failures, asserts the 11th call
  returns `EngineError::CircuitOpen { tool: "mksquashfs" }` with zero
  subprocess invocation.

## Consequences

* **Public API additions**: two new methods on `ForgeIsoEngine`, two new
  `EngineError` variants, one new public module
  (`orchestrator::circuit_breaker`). All additions; nothing removed.
  Public-API golden refreshed.
* **Backward compatibility**: existing callers of `build` and
  `inject_autoinstall` compile and behave identically. The non-
  cancellable convenience wrapper is preserved.
* **Operational**: cancelling a long-running build no longer leaks the
  subprocess. Chronically failing tools fast-fail rather than retrying
  indefinitely. Both behaviours are observable via the new error
  variants.
* **Incremental rollout**: only `mksquashfs` is wired through the
  breaker today. The plumbing makes wider adoption a one-call-site
  refactor.
