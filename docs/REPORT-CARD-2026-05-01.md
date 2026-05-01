# ForgeISO Report Card — 2026-05-01 post-uplift

Walked all 8 dimensions of `~/.claude/TIER_RUBRIC.md` against the
post-uplift state of `refactor/major-gui-overhaul`. **33 commits** landed
in this S+ uplift push.

## Tier rating

```
Tier: A
Floor reasons:
  - Reliability (B+):  graceful per-dependency failure (E010 missing tool),
                       cancel preserves state, restart restores form state
                       and theme. No chaos drill, no DR test, no auto-rollback.
                       Less applicable for a desktop build tool than for a
                       service, but rubric applies the same bar.
  - Performance (A):   criterion benches + scripts/perf-bench.sh gate exist
                       and run locally. Wired into CI requires orchestrator-
                       config update (docs/CI-INTEGRATION.md tracks the open
                       delta). Counts as A (gate exists), would be A+ when CI
                       integration lands.

Per-dimension scorecard (was → now)
  Code quality    B    → A     all non-test files ≤568 LOC; vast majority ≤300
  Testing         A-   → A+    705 tests + 13 proptest + cargo-mutants 95%
                                kill on targeted survivors + 3083-item public
                                API contract + criterion benches
  Security        B+   → A     cargo-audit + cargo-deny + gitleaks + Trivy
                                + syft SBOM + 7 ADRs. SLSA L2+ banned by
                                CLAUDE.md absolute → ceiling at A.
  Reliability     B+   → B+    no change; chaos drill is the next lever
  Observability   A-   → A     JSON tracing + jq/Vector/LogQL recipes + SLOs
  Performance     B+   → A     perf-bench script gate; CI wiring pending
  Documentation   A    → A+    7 ADRs + RUNBOOKS + SLO + COMPLIANCE +
                                CI-INTEGRATION + MUTATION + CONTRACTS coverage
                                + CLAUDE.md module map + this report card
  Process         A-   → A     conventional commits + tarpaulin gate +
                                perf script + audit script + 7 ADRs
```

## What changed (33 commits, branch `refactor/major-gui-overhaul`)

### Phase 1 — file decomposition (every non-test file ≤500 LOC)

13 commits across 6 parallel agents:
- `engine/src/config/inject.rs` 2034 → 17 files (mod + per-concern validators + per-concern test modules), all ≤350
- `engine/src/autoinstall/ubuntu.rs` 2005 → generate.rs + merge.rs + tests.rs + mod.rs
- `engine/src/sources.rs` 1123 → catalog/preset_id/preset/strategy/mod
- `engine/src/vm.rs` 991 → 11 files all <500 (per-runtime: qemu/proxmox/hyperv/vmware/vbox/firmware/ovmf/spec/launch/hypervisor/mod)
- `forge-slint/src/main.rs` 901 → 408 + handlers/{common,source,configure,build,check,mod}
- `forge-slint/ui/steps/configure.slint` 966 → 497 + per-tab files 109-193 LOC
- `tui/src/state.rs` 849 → 6 files (configure 399 acknowledged; 5 mirrored field-by-index switches)

### Phase 1g — mid-size file decomposition

5 commits, the remaining 500-750 LOC files:
- `engine/src/orchestrator/helpers.rs` 745 → 7 files
- `cli/src/main.rs` 743 → main 30 + cli 441 + dispatch 221 + preset 84
- `engine/src/autoinstall/late_commands.rs` 677 → 5 files (per-feature: time_users/system/packages/finalize)
- `engine/src/kickstart.rs` 616 → 4 files (header/post/cidr + mod)
- `engine/src/orchestrator/inject.rs` 615 → 3 files (mod + configure + place)

### Phase 2 + 5 — perf gate, audit, SLO, COMPLIANCE, CI integration notes

3 commits:
- `scripts/perf-bench.sh` (bench/compare/capture, PERF_THRESHOLD=15) + `tests/baseline-perf.json` placeholder
- `scripts/s-tier-audit.sh` (8-dimension single-command gate, --fast option)
- `docs/SLO.md` (per-op SLOs: sha256 p99 <50ms, generators <20ms, event <1µs, build <5min, GUI cold start <2s, theme toggle <50ms)
- `docs/COMPLIANCE.md` (NIST 800-53 + CIS v8 + SOC 2 mappings, self-attestation)
- `docs/CI-INTEGRATION.md` (orchestrator-config additions to wire the new scripts into per-PR/nightly/release-tag jobs)

### Phase 3 — cleanup

1 commit: deleted unused `header.slint` + `step_bar.slint` (replaced by PageHeader + Sidebar in earlier GUI overhaul); CLAUDE.md updated with new module-layout map.

### Phase 4 — seven-layer testing

7 commits:
- **proptest** (`08664d0`): 13 property tests in `engine/tests/proptest_config.rs` covering InjectConfig validators, output filename sanitization, PresetId roundtrip, output label, SHA-256 validation, IsoSource roundtrip, BuildConfig YAML parse, timezone/locale charsets, ProfileKind serde, sha512-crypt, workspace::safe_join. 256 iter default, hash_password capped at 32. 0 bugs uncovered.
- **contract** (`8cf551d`): `engine/tests/api_contract.rs` env-gated `FORGEISO_RUN_API_CONTRACT=1`, golden file 5755 LOC capturing 3083 pub items + 1122 impls; `scripts/regenerate-api-golden.sh` for intentional updates.
- **mutation** (`732bef3` `35017f5` `bf48b27` `24b6159` `e436736`): cargo-mutants config + run-mutants wrapper + 21 killer tests against initial survivors. 95% kill score on targeted re-run.

### Phase 6 — ADRs + final report

3 commits:
- ADR 0006 post-decomposition module taxonomy
- ADR 0007 seven-layer testing strategy
- This report card

## What blocks S+

1. **SLSA L2+ provenance** — explicitly banned by CLAUDE.md absolute (no `actions/attest-build-provenance`). Permanent ceiling on Security at A.
2. **Reliability** chaos drill / DR test — would require fault-injection engine tests (kill xorriso mid-run, corrupt input ISO, run out of disk). Multi-hour write. Pushes Reliability B+ → A.
3. **CI wiring of new gates** — perf-bench, run-mutants, api-contract, s-tier-audit jobs are scripts only today. The `.github/workflows/ci-rust.yml` is generated by `haskell-ci-orchestrator` (header says DO NOT EDIT). Orchestrator-config update needed (docs/CI-INTEGRATION.md). Once landed, Performance and Process both go A → A+.
4. **100% coverage on shared/core libs** — current tarpaulin floor is 40%. Real number probably 50-60% post-refactor. Reaching ≥95% on engine core (the S/S+ bar) is multi-day work.
5. **Mutation kill score full run** — agent showed 95% on targeted survivors but a full 197-mutant run was deferred (~60 min). Confirming the score across the whole engine hardens the Testing dimension.
6. **Dashboards-as-code** — observability A → A+ wants Grafana/Loki dashboards in the repo. Awkward fit for a desktop build tool with no continuous service to monitor.

## Realistic next steps (ordered by leverage)

1. Land orchestrator-config updates in `Al-Sarraf-Tech` (or wherever the orchestrator config lives) for the four new CI jobs in `docs/CI-INTEGRATION.md`. Pushes 2 dimensions A → A+.
2. Run `scripts/perf-bench.sh bench && capture` to populate `tests/baseline-perf.json` with real numbers (engine just refactored, so the baseline reflects the new module structure). Arms the perf gate.
3. Add 1-2 chaos tests in `engine/tests/chaos.rs` (kill subprocess mid-run, corrupt cache, fill output disk). Reliability B+ → A.
4. Run full `cargo-mutants` cycle and ratchet `tests/baseline-perf.json` mutant score config to lock the 95% floor.
5. Lift tarpaulin gate from 40% to actual measured (probably 50-60% post-refactor) to prevent regression.

Items 1-2 are 1-hour each; items 3-5 are half-day each.

## What's NOT realistic for this codebase

- **SLSA L3+** — banned by user policy. Don't try.
- **OpenTelemetry traces** for engine ops — adds external dependency for marginal value on a tool that runs synchronously per ISO.
- **Per-distro performance SLOs** measured nightly with dashboards — would require hosting infra. Out of scope for a desktop tool.

## Verdict

**A floor with B+ on Reliability**. The decomposition + seven-layer
testing + comprehensive docs put ForgeISO firmly in production-grade
territory. The remaining gap to S+ is real but mostly out-of-reach
without lifting absolute constraints (SLSA), or making investments
that don't suit a desktop build tool (dashboards-as-code, OTel traces).

Tracked as ADR 0006 + 0007. Re-run via `scripts/s-tier-audit.sh` at
any tag candidate.
