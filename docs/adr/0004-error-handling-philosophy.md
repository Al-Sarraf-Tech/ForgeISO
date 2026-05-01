# ADR 0004: thiserror EngineError taxonomy + anyhow at boundaries

- **Status**: Accepted
- **Date**: 2026-03-15

## Context

ForgeISO operates against a hostile environment: external tools that
may be missing or fail with cryptic exit codes (xorriso, squashfs,
qemu); user input that may be invalid; network sources that may 404;
filesystems that may be read-only; ISOs that may be corrupt or
unrecognised.

Errors must:

1. Be classifiable from outside the engine — operators need stable
   "error codes" to map symptom → runbook entry.
2. Carry enough context for the user to act (path, tool name, status).
3. Not leak as `Box<dyn Error>` or `String` into production logs where
   they lose type information.
4. Not require every front-end to translate generic errors into
   user-facing messages.

## Decision

- Engine code returns `EngineResult<T> = Result<T, EngineError>`.
- `EngineError` is a `thiserror`-derived enum with variants that map to
  documented error codes in `docs/RUNBOOKS.md`:
  - `InvalidConfig(String)`     → E060
  - `PolicyViolation(String)`   → E100
  - `Runtime(String)`           → E020/E030 (generic runtime; tool
    failures, SHA mismatches)
  - `MissingTool(String)`       → E010/E030
  - `PathSafety(String)`        → E050
  - `Network(String)`           → E090
  - `NotFound(String)`          → E001
  - `Io(io::Error)`             → E040 (Permission denied, ENOENT)
  - `SerdeJson` / `SerdeYaml`   → E060 (parse errors on user input)
  - `Reqwest`                   → E090 (download failures)
- Front-ends use `anyhow::Result<()>` at the `main()` boundary (CLI/TUI/
  GUI). They `?`-propagate `EngineError` (which auto-converts via
  `anyhow::Error::from`) and rely on `Display` for user-facing
  rendering.
- The CLI also bridges errors into the JSON tracing channel
  (`obs::spawn_event_tracer`) so log shippers see the same structured
  context.

## Alternatives considered

- **Single `String` error throughout**: cheap to write, impossible to
  classify. Operators end up grepping log lines for substrings; no
  stable code → runbook mapping. Rejected.
- **`anyhow::Error` everywhere in the engine**: erases variant
  information at the API boundary, so callers can't pattern-match on
  error category (e.g., the inject path needs to know whether xorriso
  is missing vs network failed vs config invalid). Rejected for
  library code; kept for `main()` plumbing where the choice doesn't
  matter.
- **`std::io::Error` with custom `ErrorKind`**: would require an
  `ErrorKind::Other` for almost everything we actually report; the
  derived classes would be hostile to documentation. Rejected.
- **Static error codes returned alongside messages (e.g.
  `Result<T, (Code, String)>`)**: more verbose, no compile-time
  exhaustiveness, no `?` ergonomics. Rejected.

## Consequences

- **Positive**: Every engine error has exactly one runbook entry. The
  RUNBOOKS.md taxonomy stays in sync because each variant is named in
  both places.
- **Positive**: Exhaustive `match EngineError { ... }` in front-ends
  catches new variants at compile time. When we added `PathSafety`,
  the compiler told us where to update front-end behavior.
- **Positive**: `EngineError: Send + Sync + 'static` (via `thiserror`
  + `From` impls), so it crosses tokio task boundaries without
  wrapping.
- **Negative**: Adding a variant means touching the runbook (good
  pressure to keep them in sync, but a cost). Mitigation: PR template
  reminds reviewers.
- **Negative**: `anyhow::Error` at the boundary means the very last
  chance to inspect the variant is in `main()`. Front-ends that want
  to render specific error variants differently (e.g., yellow for
  `MissingTool`, red for `Runtime`) must downcast via
  `error.downcast_ref::<EngineError>()`. Acceptable cost.
