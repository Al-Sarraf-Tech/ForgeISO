# ADR 0006: Post-decomposition module taxonomy

- **Status**: Accepted
- **Date**: 2026-05-01
- **Supersedes**: portion of ADR 0001 that described pre-decomposition file layout

## Context

ADR 0001 established the four-crate workspace (`engine` / `cli` / `tui` / `forge-slint`). Through 2026-04 a handful of files inside `engine` and `forge-slint` grew past 1000 LOC because they were the natural place to add new distros, new validators, new GUI callbacks, etc.:

- `engine/src/config/inject.rs` reached 2034 LOC
- `engine/src/autoinstall/ubuntu.rs` reached 2005 LOC
- `engine/src/sources.rs` reached 1123 LOC
- `engine/src/vm.rs` reached 991 LOC
- `forge-slint/src/main.rs` reached 901 LOC
- `forge-slint/ui/steps/configure.slint` reached 966 LOC
- `tui/src/state.rs` reached 849 LOC

A B-tier report card on 2026-05-01 (per `~/.claude/TIER_RUBRIC.md`) called out file size as the dominant blocker preventing the Code-quality dimension from reaching A (ideal ≤500 LOC, A+ ideal ≤300).

## Decision

Decompose every non-test file >800 LOC into a submodule directory. The contract:

1. Original file at `engine/src/<area>/<name>.rs` becomes a directory `engine/src/<area>/<name>/` containing `mod.rs` + N submodule files.
2. `mod.rs` re-exports everything that was previously `pub` at the original path. `use forgeiso_engine::<area>::<name>::Foo;` continues to work for downstream crates without modification.
3. Submodule files chosen by **concern** (per-distro / per-section / per-runtime), not by alphabetical convenience. The natural decomposition is documented in the file header comment.
4. Test code lives next to the code being tested, either inline (`#[cfg(test)] mod tests`) or in a sibling `tests/` submodule under the decomposed directory.

Naming conventions:

- `<area>/<name>/mod.rs` — re-exports + small orchestrator
- `<area>/<name>/<concern>.rs` — implementation per concern (≤300 LOC ideal)
- `<area>/<name>/tests/<concern>.rs` — test module mirroring the implementation

For Slint UI, the same principle applies via per-tab `*.slint` files:

- `forge-slint/ui/steps/<step>.slint` is the dispatch component
- `forge-slint/ui/steps/<step>/<tab>.slint` is each tab body

For `forge-slint/src/main.rs`, callback wiring extracts to `forge-slint/src/handlers/{common,source,configure,build,check}.rs` with `main.rs` becoming orchestration only.

For `tui/src/state.rs`, per-screen state extracts to `tui/src/state/{nav,fields,source,build,configure}.rs`.

## Alternatives considered

- **Leave as-is**: simpler in the short term but locks Code-quality dimension at B regardless of every other improvement. Rejected.
- **One file per public type**: too granular for engine code where validators and helpers cluster naturally by concern. Rejected.
- **Split by line-count threshold without semantic basis**: would create arbitrary boundaries that confuse future readers. Rejected.

## Consequences

- **Positive**: Code-quality dimension lifts from B to A+ on the rubric. Every non-test file ≤500 LOC; the vast majority ≤300.
- **Positive**: Onboarding path is shorter. A new contributor can read `engine/src/config/inject/validate/network.rs` (73 LOC) instead of scrolling through a 2034-LOC monolith.
- **Positive**: Test mutation killers (ADR 0007) cluster around the decomposed module they target, making the test-to-code mapping obvious.
- **Negative**: ~10% LOC growth from per-file headers and `pub use` re-export chains. Acceptable.
- **Negative**: Some `pub(crate)` visibility had to be promoted to `pub(super)` so neighbour modules in the same submodule can call helpers (e.g. `sanitize_vm_name` in `vm/spec.rs`). External surface unchanged.
- **Process consequence**: Future additions to a decomposed area should add a new submodule file rather than growing an existing one past the 500-LOC line. Enforced by `scripts/s-tier-audit.sh` (future enhancement: line-count linting).

## Implementation reference

Phase 1 of the S+ uplift, 2026-05-01. Branch `refactor/major-gui-overhaul`, commits:

- `2dda50c` `bc0eab1` `fbd8079` `1b7f715` — inject.rs decomposition
- `0189cd6` — autoinstall/ubuntu.rs
- `b2984cf` — sources.rs
- `025b286` — vm.rs
- `20c7d92` — forge-slint/src/main.rs + handlers/
- `c13962d` — configure.slint per-tab
- `d458f36` `9c21d54` `abea243` — tui/src/state.rs

Per-decomposition module trees are listed in `CLAUDE.md` "Module layout (post-decomposition)" section.
