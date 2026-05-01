# ADR 0007: Seven-layer testing strategy

- **Status**: Accepted
- **Date**: 2026-05-01

## Context

The existing test suite was strong on unit + integration coverage (670+ tests, ~47% workspace coverage) but missing the rigour layers that distinguish A-tier engineering from S-tier:

- **Property-based testing** — fuzzes invariants over generated inputs; catches edge cases neither the unit-test author nor the integration-test author imagined.
- **Mutation testing** — perturbs source code and verifies the test suite catches the perturbation; quantifies test thoroughness with a numeric kill score, not just coverage percentage.
- **Contract testing on public API** — locks the externally-visible surface so unintentional API changes fail CI; intentional changes require an ADR.

The S+ tier rubric calls for a "seven-layer cake" of testing: contract + property + golden + mutation + integration + perf + unit. Without these, the project ceiling is A regardless of unit-coverage numbers.

## Decision

Adopt the seven-layer testing strategy. Each layer has a tool, a location, a CI gate, and an update procedure:

| Layer | Tool | Location | Gate | Update |
|---|---|---|---|---|
| Unit | `cargo test --lib` | `#[cfg(test)] mod tests` next to code | `cargo test --workspace` | Add a test |
| Integration | `cargo test --test <name>` | `engine/tests/`, `cli/tests/`, etc. | `cargo test --workspace` | Add a test file |
| E2E regression | `cargo test --test e2e_regression` | `engine/tests/e2e_regression.rs` | `cargo test --workspace` | Add a scenario |
| Distro regression | `cargo test --test distro_regression` | `engine/tests/distro_regression.rs` | `cargo test --workspace` | Add a distro path |
| **Property** | `proptest` | `engine/tests/proptest_config.rs` | `cargo test --workspace` | Add a `proptest!` block |
| **Mutation** | `cargo-mutants` | config in `.cargo-mutants.toml` | `scripts/run-mutants.sh` | Kill survivors with new tests |
| **Contract** | `cargo-public-api` | `engine/tests/api_contract.rs` + `engine/tests/public-api.golden` | `FORGEISO_RUN_API_CONTRACT=1 cargo test` | `scripts/regenerate-api-golden.sh` + ADR |
| Coverage gate | `cargo-tarpaulin` | `tarpaulin.toml` | `cargo tarpaulin --workspace` | Lift `fail-under` after tests added |
| Perf regression | `criterion` + `scripts/perf-bench.sh` | `engine/benches/engine_hot_paths.rs` | `scripts/perf-bench.sh compare` (PERF_THRESHOLD=15) | `scripts/perf-bench.sh capture` |

Single-command invocation: `scripts/s-tier-audit.sh` runs the relevant gates for a release-readiness check.

## Why seven layers

Each layer catches a different failure mode:

- **Unit + integration**: behavioural correctness of the code-paths the author thought of.
- **E2E + distro regression**: end-to-end flows still work; specific distro quirks haven't regressed.
- **Property**: invariants hold over inputs the author didn't think of.
- **Mutation**: the test suite actually verifies behaviour vs just executing the code (high coverage with weak assertions = high mutation survivor rate).
- **Contract**: API hasn't drifted accidentally; consumers get a stable surface.
- **Coverage gate**: prevents test-deletion regressions.
- **Perf gate**: prevents silent latency-regression releases.

Removing any layer exposes a failure mode the others don't catch.

## Alternatives considered

- **Property tests only**: cheaper to set up but doesn't quantify whether existing tests are strong (mutation does that).
- **Mutation tests only**: identifies weak assertions but provides no input diversity (property does that).
- **Contract via doc-comments**: would require external tool to extract + diff; cargo-public-api already does this with semver awareness baked in.

## Consequences

- **Positive**: each new bug class has a dedicated layer to add to. A future "the locale charset accepts BiDi marks and corrupts the YAML" bug becomes "add a property test that asserts ASCII-only locale".
- **Positive**: mutation kill score is a leading indicator. When it drops below 80%, new code lacks behavioural tests even if line coverage is fine.
- **Positive**: contract test prevents the "we changed `pub fn foo(x: u32) → fn foo(x: u64)` and the next release broke every consumer" failure mode.
- **Negative**: more CI time. Mutation testing is slow (~30-60 min for engine core). Run on PR-merge or nightly, not per-commit.
- **Negative**: tooling complexity. cargo-public-api needs nightly; cargo-mutants needs a baseline. Documented in `docs/RUNBOOKS.md`, `docs/MUTATION.md`.
- **Process consequence**: every API change to engine requires either no contract diff (refactor) or a `scripts/regenerate-api-golden.sh` + ADR (intentional change). Adds friction; that's the point.

## Implementation reference

Phase 4 of the S+ uplift, 2026-05-01. Branch `refactor/major-gui-overhaul`, commits:

- `08664d0` — proptest layer (13 properties, 0 bugs uncovered first run)
- `8cf551d` — contract layer (5755-LOC golden, 3083 pub items)
- `732bef3` `35017f5` `bf48b27` `24b6159` `e436736` — mutation layer (config + 21 killer tests against initial survivors; 95% kill score on targeted re-run)

Coverage and perf gates predate this ADR (commits `7ec874f` and `17cfca4` respectively).
