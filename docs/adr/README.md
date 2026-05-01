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

When adding an ADR, copy the format from any existing entry and number
it sequentially. Don't renumber; reference older ADRs by number when
superseding.
