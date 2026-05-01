# ForgeISO Service Level Objectives

Per-operation SLOs codified for the engine hot paths. The benches in
`engine/benches/engine_hot_paths.rs` exercise each path; CI gates regressions
via `scripts/perf-bench.sh compare`.

These SLOs are the **ceiling** above which the user-perceptible delay starts
to matter. They are not aspirations — they are the bar that must hold for any
release.

## Engine hot-path SLOs

| Operation | Bench | p50 SLO | p99 SLO | Measured (placeholder) |
|---|---|---|---|---|
| `sha256_file` (4 MiB buffer) | `sha256_file` | < 30 ms | < 50 ms | TBD — populate via `scripts/perf-bench.sh capture` |
| Ubuntu autoinstall yaml gen | `generate_autoinstall_yaml` | < 10 ms | < 20 ms | TBD |
| Fedora kickstart cfg gen | `generate_kickstart_cfg` | < 10 ms | < 20 ms | TBD |
| Mint preseed gen | `generate_mint_preseed` | < 10 ms | < 20 ms | TBD |
| `EngineEvent::with_bytes` | `event_with_bytes` | < 200 ns | < 1 µs | TBD |

## End-to-end SLOs

These are not bench-measured; they are operator commitments backed by the
e2e regression tests in `engine/tests/e2e_regression.rs` and the integration
tests in `engine/tests/distro_regression.rs`.

| Operation | Target |
|---|---|
| `forgeiso build` (typical 3 GB Ubuntu ISO, NVMe) | < 5 min p99 |
| `forgeiso scan` (typical 3 GB ISO) | < 30 s p99 |
| `forgeiso doctor` (host tools check, no I/O) | < 1 s p99 |
| `forgeiso-desktop` cold start (warm cache) | < 2 s to first paint |
| `forgeiso-desktop` theme toggle | < 50 ms perceived |

## Error budget

For each SLO above:

- **Error budget**: 1% of operations may exceed the p99 target before the
  oncall is paged.
- **Action on burn**: If the p99 violation rate exceeds 5% over a 24h window,
  CI's `perf` job is the first place to look. Then the engine bench HTML
  report at `target/criterion/report/index.html`.

## Process

- New release may not raise any p99 by more than `PERF_THRESHOLD%` (default 15).
- Lowering an SLO requires an ADR documenting the user-facing impact.
- Raising an SLO is automatic when `scripts/perf-bench.sh capture` is run
  after a confirmed-faster build; document the cause in CHANGELOG.

## Trigger phrases

This document is the source of truth for the question "is ForgeISO performing
to spec?". Reference it from CI failure messages and runbook entries that
respond to perf alerts.
