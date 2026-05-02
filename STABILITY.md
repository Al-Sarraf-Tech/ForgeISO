# Stability Policy

ForgeISO follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).
This document describes which parts of the project are covered by the
1.x compatibility promise, which parts are explicitly excluded, and how
the boundary is enforced.

> **Status during 0.x.** Pre-1.0 releases are still iterating on the
> public surface. Breaking changes can land in any minor (`0.MINOR.0`)
> release; patch (`0.MINOR.PATCH`) releases remain backward compatible
> within the minor. The contract below becomes binding at the **1.0.0**
> tag.

## Versioning model

A 1.x version is encoded as `MAJOR.MINOR.PATCH`:

| Bump | Meaning |
| ---- | ------- |
| `MAJOR` (1 → 2) | Backward-incompatible change to a covered surface (see below). New majors are rare and announced at least one minor release in advance. |
| `MINOR` (1.x.0 → 1.y.0) | Backward-compatible feature addition. New CLI flags, new public Rust API items, new optional config keys. May deprecate items (with a runtime warning) but never removes them within the same major. |
| `PATCH` (1.x.y → 1.x.z) | Backward-compatible fix. No new features, no deprecation announcements, no public-API additions. |

Pre-release tags use the `-rc.N` and `-beta.N` suffixes; pre-releases
do not carry the stability promise.

## Covered surfaces (1.x compatibility promise)

The following are guaranteed stable within the 1.x line. Any change
that breaks them requires a 2.0 major release and at least one minor
of advance notice via the deprecation process documented in
[DEPRECATION.md](DEPRECATION.md).

### 1. CLI surface — `forgeiso`

- **Subcommands** present at 1.0 (`build`, `validate`, `inspect`, `vm`,
  etc.) keep their names and the documented argument shapes.
- **Long flags** (`--source`, `--output`, `--profile`, ...) keep their
  spelling and semantics. Short flags follow the same rule once
  documented in `--help`.
- **Exit codes** in the documented `forgeiso(1)` exit-code table keep
  their numeric meaning. New codes may be added in a minor; existing
  codes are never reassigned.
- **`--version` output format** is parseable as
  `forgeiso <semver> (<git-short-hash>)`.
- **`stdout` content of non-interactive commands** that produce
  machine-readable output (e.g. `forgeiso inspect --format json`)
  follows a versioned schema. The schema document itself is part of the
  covered surface.

### 2. Public Rust API — `forgeiso-engine` crate

- The set of `pub` items exported from `forgeiso_engine::*` is captured
  by the golden file at `engine/tests/public-api.golden`, regenerated
  via `scripts/regenerate-api-golden.sh`. Any change to that file in a
  way that **removes or renames** an existing entry is a breaking
  change.
- **Trait methods** with default implementations may add more methods
  in a minor (subject to the trait's own sealed-vs-open status; see
  rustdoc on each trait).
- **`#[non_exhaustive]` enums** may gain new variants in a minor —
  consumers must write `_ => ...` arms to remain compatible. This is
  documented per-enum in rustdoc.
- **Error types** (`EngineError` and the `thiserror`-derived variants)
  keep their textual messages stable for log-grep compatibility unless
  the message would mislead. Variant additions follow the
  `#[non_exhaustive]` rule above.

### 3. On-disk file formats

These are produced or consumed by ForgeISO and are covered:

- **InjectConfig YAML** (`docs/schema/inject-config.json` once it
  ships) — every field name and type stable within 1.x.
- **Autoinstall outputs**: `autoinstall.yaml` (Ubuntu),
  `kickstart.cfg` (Fedora/RHEL family), `preseed.cfg` (Debian),
  `airootfs/` (Arch). Schema additions allowed; field removals
  prohibited.
- **`slint-state.json`** at `$XDG_DATA_HOME/forgeiso/slint-state.json`
  — backward-compatible reads. Old files load on a newer release; new
  fields use `serde(default)` so absence is allowed.
- **Build manifest** emitted alongside each generated ISO
  (`<output>.manifest.json`) — schema versioned, additive within 1.x.

### 4. Generated ISO contract

- A built ISO is bootable by the same boot media and emulators
  (BIOS + UEFI) the 1.0 release supports.
- The volume label format and the SHA-256 manifest format do not
  change in incompatible ways within 1.x.
- The cosign-signed-blob layout described in
  [`docs/SECURITY.md`](docs/SECURITY.md) is stable; verification
  scripts written for 1.0 keep working through 1.x.

## Explicitly NOT covered

The following surfaces may change in any release without notice and
are **not** part of the compatibility promise:

- **Internal modules** — anything reachable only through `pub(crate)`,
  `pub(super)`, or items not re-exported at the crate root. Reading
  the `engine/src/...` source tree directly is at your own risk.
- **CLI debug/diagnostic output** — text emitted under `--verbose`,
  `RUST_LOG`-controlled tracing, or stderr progress reporting.
- **Log format details** — the JSON-line schema described in
  [`docs/RUNBOOKS.md`](docs/RUNBOOKS.md) is *additive* (new fields may
  appear) but not strictly versioned. Field removals are avoided where
  possible but not formally promised.
- **TUI key-bindings** — `forgeiso-tui` is best-effort. Keymaps may
  change between minors. The TUI is a convenience front-end, not a
  scriptable interface.
- **GUI visual design** — `forge-slint` layout, theme tokens, icon
  set, colors, animations. Behavior (the underlying state machine,
  the on-disk persistence) remains covered as documented above.
- **Benchmark numbers** in `tests/baseline-perf.json`. The perf gate
  is an internal CI guardrail, not a published latency contract.
- **Build output binary sizes and hashes** — these depend on the Rust
  toolchain version, not the project. Reproducible-build properties
  are aspirational and tracked in
  [ADR 0010](docs/adr/0010-security-contract-desktop-tool.md), not yet
  promised.
- **Test fixtures** under `tests/fixtures/` and `engine/tests/` — these
  exist to validate the engine and may change to fit new tests.

## Contract enforcement

- The **public-Rust-API golden** (`engine/tests/public-api.golden`) is
  regenerated only by an explicit, reviewer-visible commit. Any PR
  whose `cargo public-api` diff changes the golden is flagged for
  semver review.
- **CLI surface** changes are caught by `cli/tests/*` integration
  tests that pin every documented flag.
- **Schema changes** to `InjectConfig` and `BuildConfig` are caught by
  `engine/tests/proptest_config.rs` round-trip tests plus targeted
  unit tests in `engine/src/config/inject/tests/`.
- The **chaos suite** (`engine/tests/chaos.rs`) verifies the
  reliability contract from
  [ADR 0008](docs/adr/0008-reliability-contract-desktop-tool.md), which
  is itself part of the covered surface.

## Release cadence

- **Minor releases** target a roughly monthly cadence once the project
  reaches 1.0. The actual cadence depends on landed work — there is no
  promise of a release in any given month.
- **Patch releases** are cut on demand for security fixes or critical
  regressions. There is no minimum interval.
- **Major releases** are rare. A 2.0 will not ship without at least
  one preceding 1.x minor that announces the intent and starts the
  deprecation clock for any removed items.

## Supported versions

Tracking is duplicated in the table maintained at
[`SECURITY.md`](SECURITY.md). The shorthand:

- During 0.x: only the latest minor receives fixes.
- After 1.0: the latest two minors of the current major receive fixes;
  the previous major receives security fixes for 6 months after a new
  major ships.

## See also

- [DEPRECATION.md](DEPRECATION.md) — how features are retired across
  minors and majors.
- [SECURITY.md](SECURITY.md) — vulnerability reporting and the
  supported-version table.
- [CHANGELOG.md](CHANGELOG.md) — release-by-release history. Any
  breaking change in a major release is enumerated under
  `### Removed` or `### Changed (breaking)`.
- [docs/adr/](docs/adr/) — load-bearing design decisions that anchor
  the covered surfaces.
