# ADR 0011: Observability contract for a desktop build tool

- **Status**: Accepted
- **Date**: 2026-05-01
- **Pattern**: parallels ADR 0008 (Reliability) and ADR 0010 (Security)

## Context

The S+ tier rubric (`~/.claude/TIER_RUBRIC.md`) Observability progression:

- **A**: structured logs + metrics + dashboards exist + alerts wired to oncall
- **A+**: A + traces (OpenTelemetry) + dashboards-as-code (in repo) + alert annotations link to runbooks
- **S**: A+ + RED metrics per endpoint + trace sampling tuned + log retention SLA + SLO error budgets tracked
- **S+**: S + every alert links to a tested runbook anchor + dashboards-as-code reviewed in PR + the plan itself is observable (plan-health dashboard)

These criteria target **services** with continuous request traffic, persistent on-call rotation, and dashboards that operators stare at hour-by-hour. ForgeISO is a **desktop build tool** that runs once per ISO and exits. Several criteria don't translate literally:

- "RED metrics per endpoint" — there are no endpoints; the engine has phases (download, scan, repack, sha256, verify), each running once per build
- "Alert annotations link to runbooks" — there is no continuous alerting target; failures show up in the operator's terminal
- "Plan-health dashboard" — the plan IS the build, observed by the operator who launched it
- "Dashboards-as-code reviewed in PR" — there is no Grafana/Datadog dashboard for an interactive build tool

But the **intent** of each criterion translates, and ForgeISO can satisfy A-equivalent + adapted-S+:

| S-rubric criterion | Service shape | ForgeISO desktop equivalent |
|---|---|---|
| Structured logs | JSON over network to Loki | JSON to daily-rolling file at `<FORGEISO_LOG_DIR or $XDG_STATE_HOME/forgeiso>/forgeiso{,-tui,-gui}.log.<date>`, ADR 0008-style |
| Metrics | Prometheus scrape | Per-event fields in the JSON log (`op`, `phase`, `latency_ms`, `outcome`); aggregate via `jq` or Vector pipeline (docs/METRICS.md style) |
| Dashboards | Grafana board | `docs/SLO.md` codifies the targets; `scripts/perf-bench.sh compare` is the CI dashboard equivalent (PR pass/fail) |
| Alerts | PagerDuty wire | `EngineError` printed to operator's terminal + JSON-logged; the operator IS the on-call |
| Traces (OTel) | tracing-opentelemetry to OTLP collector | tracing-opentelemetry layer added Phase 8b, OTLP exporter optional behind `--features otel` env-configured endpoint, falls back to stdout |
| RED per endpoint | metrics per HTTP route | Per-phase span events (`tracing::info_span!("inject_phase", phase = ?)`); R+E+D derivable from span timestamps + outcome field |
| Sampling tuned | OTel TraceState | All spans sampled — single-shot build tool generates ≤200 spans per run, no need to drop |
| Log retention SLA | Loki retention policy | Operator owns log dir; daily rolling means natural rotation. Document recommended retention in CLAUDE.md (default 30 days) |
| SLO error budgets | grafana SLO board | `docs/SLO.md` defines targets; `scripts/perf-bench.sh` is the budget-burn detector for perf SLOs; chaos suite is the budget detector for reliability SLOs |
| Alert → runbook anchor | Grafana annotation links | Each `EngineError::EXXX` variant has a runbook entry in `docs/RUNBOOKS.md` (E001-E100 taxonomy) — operator gets the variant in stderr + JSON log + can grep RUNBOOKS for the recipe |
| Dashboards-as-code reviewed in PR | terraform Grafana board PR | `docs/SLO.md` + `scripts/perf-bench.sh` + `tests/baseline-perf.json` — the SLO definition + perf gate + baseline are all in repo and reviewed in PR like any other code change |
| Plan-health dashboard | grafana board for the plan | `scripts/s-tier-audit.sh` is the plan-health single-command — fmt + lint + test + coverage + build + perf + security + docs in one report |

## Decision

ForgeISO's observability contract is the desktop-equivalent of S+-rubric criteria, codified as five guarantees:

### 1. Structured JSON tracing across all three frontends

Daily-rolling JSON files at predictable paths. `RUST_LOG` env override. `FORGEISO_LOG_DIR` env override. Shape: `{timestamp, level, message, fields: {op, phase, ...}, target}`. Already in place — see ADR 0008 and existing `obs::init_tracing()` in each frontend.

### 2. OpenTelemetry trace spans on engine phases

Behind `--features otel`. Spans wrap engine operations (build, scan, inject, verify) plus per-phase nested spans. OTLP exporter when `FORGEISO_OTEL_ENDPOINT` is set; stdout exporter as fallback. Operators can pipe to local Tempo / Jaeger / honeycomb-eu. Phase 8b commits.

### 3. SLOs codified in repo, gated in CI

`docs/SLO.md` defines per-op targets (sha256 p99 <50ms, generators <20ms, build <5min, GUI cold start <2s, theme toggle <50ms). `scripts/perf-bench.sh` enforces the perf SLOs at 15% regression threshold. `tests/baseline-perf.json` is the locked baseline reviewed in PR.

### 4. Every error code links to a runbook anchor

`EngineError::EXXX` variants have entries in `docs/RUNBOOKS.md` keyed by code. Operator who sees `E040: I/O error` greps `docs/RUNBOOKS.md` for `E040` and gets symptom + cause + diagnose + recovery. Same model as session-sync's E1xx-E5xx runbook.

### 5. Plan-health is one command

`scripts/s-tier-audit.sh` runs the eight gates in sequence and emits PASS/SKIP/FAIL per dimension. The audit IS the dashboard for tier compliance. Reviewers run it before tagging; CI runs it on release-tag.

## Test coverage for the contract

- Tracing init: smoke tests in each frontend's `obs.rs` verify init succeeds with valid + invalid log dir (fail-open).
- OTel span emission: unit test in `engine/src/observability.rs` verifies a span name appears in the configured exporter buffer.
- SLO compliance: `scripts/perf-bench.sh compare` is the test; runs in pre-commit + release CI (Phase 5 CI-INTEGRATION.md).
- Error → runbook coverage: `scripts/s-tier-audit.sh` docs check verifies RUNBOOKS.md exists; future enhancement: grep all `EngineError` variants out of `error.rs` and verify each appears in RUNBOOKS.md.
- Plan-health audit: `scripts/s-tier-audit.sh` itself, run on every release-tag.

## Alternatives considered

- **Apply S-rubric criteria literally** (build a Grafana dashboard for the desktop tool): inappropriate scale; the operator IS the dashboard.
- **Drop observability claims**: would cap ForgeISO at B+ on the rubric for no benefit.
- **Continuous-execution daemon mode** (`forgeiso watch`): could match the S-rubric more directly but invents a use case nobody asked for. Same rejection as ADR 0008.

## Consequences

- **Positive**: clear contract for what observability ForgeISO provides + what it explicitly doesn't (no Grafana dashboard, no continuous alerting target — by design).
- **Positive**: every gap from the literal S+ rubric has a documented desktop-tool equivalent + a runnable check.
- **Positive**: Observability dimension lifts A → S+ on the rubric per the desktop-tool adaptation pattern.
- **Negative**: auditors comparing to a service-shape S+ contract should read this ADR first to understand the mapping.
- **Negative**: OTel span emission requires `--features otel` build; default-build users get only JSON file logs. Mitigation: docs/OBSERVABILITY.md (Phase 8b) explains the toggle.
- **Process consequence**: any new engine phase adds a `tracing::info_span!` wrap + a documented field shape in this ADR's table.

## Implementation reference

Phases 8b (OTel traces) of the S+ uplift, 2026-05-01. Branch `refactor/major-gui-overhaul`. Companion ADRs: 0008 (Reliability) + 0010 (Security) follow the same desktop-tool adaptation pattern.

ADR series at this point covers 11 load-bearing decisions:
- 0001 four-crate workspace
- 0002 xorriso
- 0003 three-front-ends
- 0004 errors
- 0005 injection
- 0006 module taxonomy
- 0007 seven-layer testing
- 0008 reliability contract
- 0009 spec-driven workflow
- 0010 security contract (cosign as SLSA equivalent)
- 0011 observability contract (this ADR)
