# ADR 0003: TUI + GUI + CLI three front-end strategy

- **Status**: Accepted
- **Date**: 2026-03-15

## Context

ForgeISO's user populations have non-overlapping ergonomic needs:

- **CI / scripting**: requires deterministic flag-driven invocation,
  JSON output (`--json`), exit codes, no TTY interaction. Anything
  resembling a wizard breaks `set -e` pipelines.
- **Operators on remote shells**: have a TTY but no display. Want a
  guided workflow with progress, validation, and an in-app log so they
  don't have to memorize 60+ flags.
- **Desktop users**: want clickable preset cards, visible toggles for
  things like firewall/SSH/sudo, persisted form state across sessions,
  and a polished error surface.

A single UI cannot serve all three without compromise:

- A CLI-with-prompts is unusable in CI.
- A TUI on a headless server is fine, but on a desktop loses the visual
  affordance of preset cards.
- A GUI on a server is impossible without X forwarding and is a poor
  fit for SSH-only ops.

## Decision

Build **three separate binaries**, all sharing the `forgeiso-engine`
library:

- `forgeiso` (CLI) — clap subcommands. Output is human-readable on
  stderr by default; `--json` switches to one-shot JSON on stdout. No
  TTY interaction at any time.
- `forgeiso-tui` — ratatui guided wizard with four steps (Source,
  Configure, Build, Optional Checks). All keyboard-driven. Suitable
  for tmux/screen, SSH sessions, and serial consoles.
- `forge-slint` (`forgeiso-desktop`) — Slint-based wizard with preset
  cards, visible toggles, persisted state via
  `dirs::config_dir()/forgeiso/state.json` (passwords excluded via
  `#[serde(skip)]`). Refuses to start without a display environment.

Each front-end depends on `forgeiso-engine`; none depends on the others.
Engine state changes propagate via the `tokio::sync::broadcast` event
channel (`EngineEvent` / `EventPhase` / `EventKind`).

## Alternatives considered

- **CLI + TUI only**: leaves desktop users with a step-driven terminal,
  which is unfamiliar territory for that audience. Rejected after
  user testing showed friction with the keyboard-driven model.
- **GUI + CLI only**: leaves SSH operators without a guided path; they
  must memorize flags. Rejected.
- **Single binary with `--ui` flag**: discussed in ADR-0001; forces
  every install to carry every UI's deps.
- **Web UI**: would require a server process, port management, browser
  context. Wrong fit for a developer/ops tool that operates on local
  files and produces local artifacts.

## Consequences

- **Positive**: Each UI has a clean ownership model. CLI flag stability
  is a contract for CI users; TUI key bindings are a contract for
  terminal users; GUI form layout is a contract for desktop users.
  Changes in one don't ripple into the others.
- **Positive**: CI builds the CLI with no Slint or ratatui transitive
  deps, keeping CI build time under 2 minutes for the lint job.
- **Positive**: Bug surface is localised — a regression in the GUI's
  preset cards cannot break the CLI's `--json` output.
- **Negative**: Three release artifacts to ship per platform. Mitigated
  by a shared workspace `Cargo.lock` and a single CI workflow that
  builds all three.
- **Negative**: UX changes that cross modalities (e.g., a new preset)
  require touching all three UIs to expose the same affordance.
  Mitigated by keeping the engine as the source of truth (preset
  list lives in `engine/src/sources.rs::all_presets()`); each UI just
  iterates that list.
