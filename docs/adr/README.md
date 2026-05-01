# Architecture Decision Records

ADRs document load-bearing decisions in ForgeISO. Each one captures the
context, the decision, the alternatives, and the consequences — so future
readers (including future maintainers and any AI agent picking up work)
can tell why the code looks the way it does.

Format: `NNNN-short-kebab-title.md`. Status: Accepted | Superseded by
ADR-NNNN | Deprecated.

| #    | Title                                                          | Status   |
| ---- | -------------------------------------------------------------- | -------- |
| [0001](0001-four-crate-workspace.md)  | Four-crate workspace split (engine + cli + tui + gui)        | Accepted |
| [0002](0002-xorriso-over-alternatives.md) | xorriso as the canonical ISO repack tool                  | Accepted |
| [0003](0003-three-front-ends.md)      | TUI + GUI + CLI three front-end strategy                     | Accepted |
| [0004](0004-error-handling-philosophy.md) | thiserror EngineError taxonomy + anyhow at boundaries     | Accepted |
| [0005](0005-iso-injection-strategy.md) | Distro-specific autoinstall injection (not generic overlay) | Accepted |
| [0006](0006-post-decomposition-module-taxonomy.md) | Module-directory layout for files >800 LOC                   | Accepted |
| [0007](0007-testing-strategy-seven-layer.md) | Seven-layer testing (unit + integration + e2e + property + mutation + contract + perf) | Accepted |
| [0008](0008-reliability-contract-desktop-tool.md) | Reliability contract — desktop-tool adaptation of S-rubric service criteria | Accepted |
| [0009](0009-spec-driven-development-workflow.md) | Spec-driven development workflow (brainstorm → spec → ADR → plan → review → implement) | Accepted |
| [0010](0010-security-contract-desktop-tool.md) | Security contract — cosign keyless as desktop-tool SLSA equivalent | Accepted |
| [0011](0011-observability-contract-desktop-tool.md) | Observability contract — desktop-tool adaptation of S+ rubric | Accepted |
| [0012](0012-cancellation-and-circuit-breakers.md) | CancellationToken plumbing + per-tool circuit breakers (extends 0008) | Accepted |

When adding an ADR, copy the format from any existing entry and number
it sequentially. Don't renumber; reference older ADRs by number when
superseding.
