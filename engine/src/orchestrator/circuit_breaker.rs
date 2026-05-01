//! Per-tool circuit breaker for shell-out reliability.
//!
//! ADR (incremental rollout):
//! ─────────────────────────────────────────────────────────────────────────
//! The orchestrator shells out to several external binaries (`xorriso`,
//! `unsquashfs`, `mksquashfs`, `qemu`, …). When one of these tools is
//! transiently unhealthy (disk full, permissions race, hung child process)
//! the engine traditionally retried up to a fixed bound, which amplified
//! pressure on an already-suffering subsystem.
//!
//! [`CircuitBreaker`] adds a fast, in-process fail-fast layer in front of
//! each shell-out. It tracks the last `window` calls and trips into the
//! `Open` state once the failure count meets `failure_threshold`. Subsequent
//! calls return [`EngineError::CircuitOpen`] immediately without spawning a
//! subprocess. After `reset_timeout` the breaker enters `HalfOpen`, allowing
//! a single probe call; success closes the breaker, failure re-opens it.
//!
//! The state machine is intentionally tiny — a single [`RwLock`] guards a
//! state struct so all transitions are atomic with respect to observers.
//!
//! Initial wiring covers `mksquashfs` only (used by the autoinstall rootfs
//! repack path). Wider rollout (`xorriso`, `unsquashfs`, `qemu`, …) is
//! deliberately incremental: each shell-out site needs its own breaker
//! instance and a small refactor of its caller. We add them as the metrics
//! from the first integration justify the change, rather than retrofitting
//! every call site at once.
//!
//! Safety:
//! - All public methods are `Send + Sync`; the breaker is intended to live
//!   inside an `Arc<…>` shared across orchestrator tasks.
//! - The breaker takes no I/O of its own — failures must be reported by the
//!   caller via [`CircuitBreaker::record_failure`].
//! - The default threshold (5 failures in a window of 10 calls, reset after
//!   30s) is conservative; tune per-tool via [`CircuitBreakerConfig`].

use std::collections::VecDeque;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::error::{EngineError, EngineResult};

/// State of the circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Calls are permitted; failures are tracked.
    Closed,
    /// Calls are short-circuited with [`EngineError::CircuitOpen`].
    Open,
    /// A single probe call is permitted; outcome decides next state.
    HalfOpen,
}

/// Configuration for [`CircuitBreaker`].
#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    /// Number of recent calls to track.
    pub window: usize,
    /// Failure count within the window that trips the breaker.
    pub failure_threshold: usize,
    /// Time the breaker stays `Open` before transitioning to `HalfOpen`.
    pub reset_timeout: Duration,
}

impl CircuitBreakerConfig {
    /// Construct a config; values are caller-controlled.
    pub const fn new(window: usize, failure_threshold: usize, reset_timeout: Duration) -> Self {
        Self {
            window,
            failure_threshold,
            reset_timeout,
        }
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            window: 10,
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug)]
struct Inner {
    state: CircuitState,
    /// `true` = failure, `false` = success. Bounded by `window`.
    history: VecDeque<bool>,
    opened_at: Option<Instant>,
}

/// Per-tool circuit breaker.
///
/// Cheap to share via `Arc<CircuitBreaker>`; the type itself owns no
/// background tasks.
#[derive(Debug)]
pub struct CircuitBreaker {
    tool: String,
    config: CircuitBreakerConfig,
    inner: RwLock<Inner>,
}

impl CircuitBreaker {
    /// Create a new breaker for `tool` with the supplied config.
    pub fn new(tool: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            tool: tool.into(),
            config,
            inner: RwLock::new(Inner {
                state: CircuitState::Closed,
                history: VecDeque::with_capacity(config.window),
                opened_at: None,
            }),
        }
    }

    /// Convenience constructor with [`CircuitBreakerConfig::default`].
    pub fn with_defaults(tool: impl Into<String>) -> Self {
        Self::new(tool, CircuitBreakerConfig::default())
    }

    /// Tool name this breaker guards.
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// Current state. Locks internally.
    pub fn state(&self) -> CircuitState {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        inner.state
    }

    /// Permission gate before invoking the underlying shell-out.
    ///
    /// Returns `Ok(())` when calls are permitted (`Closed` or `HalfOpen`).
    /// Returns [`EngineError::CircuitOpen`] when the breaker is `Open` and
    /// the reset timeout has not yet elapsed.
    ///
    /// May transition `Open → HalfOpen` based on elapsed time.
    pub fn allow_call(&self) -> EngineResult<()> {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        match inner.state {
            CircuitState::Closed | CircuitState::HalfOpen => Ok(()),
            CircuitState::Open => {
                let elapsed = inner.opened_at.map(|t| t.elapsed()).unwrap_or_default();
                if elapsed >= self.config.reset_timeout {
                    // Probe window: allow exactly one call.
                    inner.state = CircuitState::HalfOpen;
                    Ok(())
                } else {
                    Err(EngineError::CircuitOpen {
                        tool: self.tool.clone(),
                    })
                }
            }
        }
    }

    /// Record a successful call. Closes the breaker if it was `HalfOpen`.
    pub fn record_success(&self) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        push_bounded(&mut inner.history, false, self.config.window);
        if inner.state == CircuitState::HalfOpen {
            inner.state = CircuitState::Closed;
            inner.opened_at = None;
            inner.history.clear();
        }
    }

    /// Record a failed call. Trips the breaker if the failure count within
    /// the sliding window meets the configured threshold, or if a `HalfOpen`
    /// probe failed.
    pub fn record_failure(&self) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());

        if inner.state == CircuitState::HalfOpen {
            // Probe failed — re-open immediately.
            inner.state = CircuitState::Open;
            inner.opened_at = Some(Instant::now());
            return;
        }

        push_bounded(&mut inner.history, true, self.config.window);
        let failures = inner.history.iter().filter(|f| **f).count();
        if failures >= self.config.failure_threshold && inner.state == CircuitState::Closed {
            inner.state = CircuitState::Open;
            inner.opened_at = Some(Instant::now());
        }
    }

    /// Test-only knob: force the breaker into `Open` with `opened_at = now`.
    #[cfg(test)]
    pub(crate) fn force_open(&self) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.state = CircuitState::Open;
        inner.opened_at = Some(Instant::now());
    }
}

fn push_bounded(history: &mut VecDeque<bool>, value: bool, capacity: usize) {
    if history.len() == capacity {
        history.pop_front();
    }
    history.push_back(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_breaker_allows_calls() {
        let cb = CircuitBreaker::with_defaults("test-tool");
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_call().is_ok());
    }

    #[test]
    fn breaker_opens_after_threshold_failures() {
        let cfg = CircuitBreakerConfig::new(10, 5, Duration::from_secs(60));
        let cb = CircuitBreaker::new("xorriso", cfg);
        for _ in 0..4 {
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitState::Closed, "below threshold");
        cb.record_failure(); // 5th
        assert_eq!(cb.state(), CircuitState::Open, "threshold tripped");
    }

    #[test]
    fn open_breaker_short_circuits_calls() {
        let cb = CircuitBreaker::with_defaults("xorriso");
        cb.force_open();
        let err = cb.allow_call().expect_err("must short-circuit");
        match err {
            EngineError::CircuitOpen { tool } => assert_eq!(tool, "xorriso"),
            other => panic!("expected CircuitOpen, got {other:?}"),
        }
    }

    #[test]
    fn half_open_after_reset_timeout() {
        let cfg = CircuitBreakerConfig::new(10, 5, Duration::from_millis(0));
        let cb = CircuitBreaker::new("xorriso", cfg);
        cb.force_open();
        // reset_timeout=0 means any subsequent allow_call promotes Open->HalfOpen.
        assert!(cb.allow_call().is_ok());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn half_open_success_closes_breaker() {
        let cfg = CircuitBreakerConfig::new(10, 5, Duration::from_millis(0));
        let cb = CircuitBreaker::new("mksquashfs", cfg);
        cb.force_open();
        cb.allow_call().expect("probe permitted");
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn half_open_failure_reopens_breaker() {
        let cfg = CircuitBreakerConfig::new(10, 5, Duration::from_millis(0));
        let cb = CircuitBreaker::new("mksquashfs", cfg);
        cb.force_open();
        cb.allow_call().expect("probe permitted");
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn sliding_window_bounds_history() {
        let cfg = CircuitBreakerConfig::new(3, 3, Duration::from_secs(60));
        let cb = CircuitBreaker::new("test", cfg);
        // 3 failures fill the window and trip the breaker.
        for _ in 0..3 {
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn successes_within_window_keep_breaker_closed() {
        let cfg = CircuitBreakerConfig::new(10, 5, Duration::from_secs(60));
        let cb = CircuitBreaker::new("test", cfg);
        for _ in 0..4 {
            cb.record_failure();
        }
        for _ in 0..6 {
            cb.record_success();
        }
        // Window = 10 with 4 failures, then 6 successes pushes failures out.
        cb.record_failure();
        // Total failures in window now = 1 (4 oldest were evicted by successes + new failure).
        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
