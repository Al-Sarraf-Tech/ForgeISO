# ADR 0001: Four-crate workspace split (engine + cli + tui + gui)

- **Status**: Accepted
- **Date**: 2026-03-15

## Context

ForgeISO needs to serve three distinct user populations:

1. **CI/scripting users** — want a stable flag-driven CLI that produces
   machine-readable JSON, never blocks on a TTY, exits with a clear
   status code.
2. **Operators on remote/headless boxes** — want a guided terminal
   workflow with progress bars, validation feedback, and an in-app log,
   but no graphical dependency.
3. **Desktop users** — want a wizard with click-through preset selection,
   visible toggles, real-time progress, and persisted form state.

All three populations need the same underlying capabilities: doctor,
inspect, build, scan, test, verify, inject, diff. The capability set is
non-trivial (4900 lines of engine code, dozens of distro-specific
branches, async download/hash machinery).

We needed a structure that:

- Lets a CI workflow build only the CLI (no GUI deps; faster builds).
- Lets the GUI/TUI evolve their UX independently of the CLI's flag
  surface.
- Avoids forcing every front-end to take a transitive dependency on
  Slint, ratatui, indicatif, or each other.
- Keeps the engine testable without any front-end framework loaded.

## Decision

Split the project into **four cargo workspace members**:

- `engine/` — `forgeiso-engine` library: all distro logic, ISO IO,
  scanners, autoinstall generators, event broadcaster. No UI deps. No
  process-exit calls. Returns `EngineResult<T>` everywhere.
- `cli/` — `forgeiso` binary: clap-driven advanced CLI. Depends on
  `engine` + `clap` + `indicatif` + `tracing`. Stable for scripting.
- `tui/` — `forgeiso-tui` binary: ratatui guided wizard. Depends on
  `engine` + `crossterm` + `ratatui`. No graphical deps.
- `forge-slint/` — `forge-slint` binary: Slint-based desktop GUI.
  Depends on `engine` + `slint`. Refuses to start without `$DISPLAY`
  or `$WAYLAND_DISPLAY`.

The engine emits `EngineEvent` over a tokio `broadcast::Sender`; each
front-end subscribes and renders the events its own way. This is the
seam that lets all three UIs share machinery without coupling.

## Alternatives considered

- **Single binary with `--ui {cli,tui,gui}`**: forces every install to
  carry slint/ratatui/clap deps. CI builds bloat; release artifacts
  carry GUI deps no headless user wants. Rejected.
- **Three top-level crates, no workspace**: loses shared `Cargo.lock`,
  shared lints, atomic `cargo test --workspace`. Rejected.
- **Engine as a published crate**: would freeze the API too early. The
  engine is iterated alongside the front-ends; an internal workspace
  member is the right granularity.

## Consequences

- **Positive**: `cargo build -p forgeiso` builds the CLI without
  pulling Slint or ratatui. CI can split the matrix per crate.
- **Positive**: Engine tests run against real distro fixtures without
  any UI scaffolding (see `engine/tests/distro_regression.rs`).
- **Positive**: Front-ends can use independent design philosophies
  (TUI = step wizard, GUI = card grid, CLI = subcommands) without
  merging conflicting UX into a single struct.
- **Negative**: Three places to wire the same observability boilerplate
  (`obs.rs` exists in `cli/src`, `tui/src`, and `forge-slint/src`).
  Mitigated by keeping the modules small and copying the pattern.
- **Negative**: Three places to bump dependency versions. Mitigated by
  using `[workspace.dependencies]` for shared crates.
