# Deprecation Policy

This document describes how ForgeISO retires items from the surfaces
covered by [STABILITY.md](STABILITY.md). It applies once the project
reaches **1.0**; pre-1.0 releases may make breaking changes in any
minor without going through this process.

## Goals

- A user running 1.x today can upgrade to any later 1.y release
  without code or workflow changes.
- A user running 1.x today receives a clear, actionable runtime
  warning at least one minor release before any item they depend on
  is removed.
- A maintainer reading the source can identify every deprecated item
  by a single mechanical search.

## Lifecycle

Each item in a covered surface (CLI flag, public Rust function,
config key, exit code, manifest field, ...) moves through three
states:

```
            announce              remove
   Active ───────────► Deprecated ─────► Removed
                       (1.x onwards)     (next major)
```

| State | What it means |
| ----- | ------------- |
| **Active** | The item is part of the 1.x compatibility promise. Use is encouraged. No warnings. |
| **Deprecated** | Still works as documented. Use is discouraged: a runtime warning is emitted (CLI/TUI/GUI) and a `#[deprecated]` attribute is attached (Rust). The replacement is named in every channel. Deprecation is announced in `CHANGELOG.md` under `### Deprecated`. |
| **Removed** | The item no longer exists. Removal is allowed only in a **major** release. The major release notes list every removed item. |

An item never goes from Active to Removed without passing through
Deprecated.

## Minimum deprecation window

Removal is permitted only when both of the following are true:

1. The item has been in the **Deprecated** state for **at least one
   complete minor release** of the current major (so users running
   1.N.x are exposed to the warning before 2.0 ships).
2. **At least 90 calendar days** have passed since the minor release
   that announced the deprecation.

Whichever condition takes longer governs. If only condition 2 is met
but the minor release containing the announcement has not yet
shipped, the item is not yet Deprecated for purposes of this policy.

## Communication channels

When an item moves to Deprecated, the deprecation is announced
through every channel that surface uses:

| Surface | Announcement form |
| ------- | ----------------- |
| **CLI flag** or **subcommand** | Emit a single-line warning to stderr on use: `warning: --old-flag is deprecated and will be removed in 2.0; use --new-flag instead`. The `--help` output for the flag/subcommand is updated to include `(deprecated; use ...)`. |
| **Public Rust API** | `#[deprecated(since = "1.N.0", note = "use `new_thing` instead; will be removed in 2.0")]` on the item. The replacement name is required. |
| **Config key** (InjectConfig, BuildConfig, slint-state) | A warning is emitted when a deprecated key is parsed: `warning: config key 'foo' is deprecated; use 'bar' instead`. The new key is preferred when both are present. |
| **Exit code** | An exit code can only become deprecated by being aliased to a new code; the old number keeps working but the man-page entry gains `(deprecated alias for N)`. |
| **Manifest field** | A deprecated field continues to be written for one minor (so consumers see the deprecation notice) and is then dropped at the same major as removal. The replacement field is added in the same release. |
| **All of the above** | A `### Deprecated` section in the next `CHANGELOG.md` release lists every deprecation introduced in that release, with the replacement and the planned removal version. |

## Code markers (Rust)

Deprecated public Rust items must use the `#[deprecated]` attribute:

```rust
#[deprecated(
    since = "1.4.0",
    note = "use `InjectConfig::with_profile` instead; will be removed in 2.0"
)]
pub fn legacy_with_profile(...) -> InjectConfig { ... }
```

The `since` field is the **first** version in which the item is
Deprecated. The `note` always names a replacement. Compiler warnings
during the project's own build are silenced via
`#[allow(deprecated)]` only at internal call sites that exist solely
to keep tests for the deprecated path running until removal — never
in production code.

## Code markers (CLI / TUI / GUI)

CLI runtime warnings are emitted via the standard `tracing::warn!`
macro and routed through the same JSON log line format as any other
warning. The warning carries a `deprecation = true` field so that log
consumers can grep:

```json
{"timestamp":"2026-08-01T00:00:00Z","level":"WARN","target":"forgeiso::cli","message":"--old-flag is deprecated; use --new-flag","deprecation":true,"removal_version":"2.0.0","replacement":"--new-flag"}
```

The TUI and GUI surface the same message in-band: the TUI prints a
banner above the active screen, the GUI shows a non-modal toast next
to the StatusBar that auto-dismisses after the next user action.

## Security and correctness exceptions

The minimum-window rule does not apply when **leaving the item
active causes user harm**. Specifically:

- A flag or API that disclosed a token, weakened a verification step,
  or generated an unsafe configuration.
- A config key whose documented behavior diverged from its actual
  behavior in a way that could mislead an operator.

In these cases the item may move directly from Active to Removed in a
patch (`1.x.z`) release. The release notes describe what was removed,
why, and what to use instead. The fix is also published as a GitHub
Security Advisory.

## Tracking

Open deprecations are tracked in two places:

1. The `### Deprecated` section of each release's `CHANGELOG.md`
   entry.
2. A canonical **deprecations table** maintained at the bottom of
   this file, listing every currently-Deprecated item, the version
   that announced it, and the planned removal version.

```
| Item                          | Surface     | Deprecated since | Planned removal | Replacement      |
| ----------------------------- | ----------- | ---------------- | --------------- | ---------------- |
| (none yet — added at 1.0)     |             |                  |                 |                  |
```

When an item's removal release ships, the row moves to the changelog
of that release and is deleted from this table.

## Examples (illustrative; none currently active)

**CLI flag rename:**

> *1.4.0 release notes:*
> Deprecated: `--legacy-source` is deprecated; use `--source` with the
> same value. Will be removed in 2.0.0.

**Public API addition + old item deprecation:**

> *1.5.0 release notes:*
> Added: `InjectConfig::builder()` returns a typed builder.
> Deprecated: `InjectConfig::from_yaml_unchecked` is deprecated; use
> `InjectConfig::builder().from_yaml(...)`. Will be removed in 2.0.0.

**Config key replacement:**

> *1.6.0 release notes:*
> Deprecated config key `output.path` (use `output.directory`). Old
> key continues to load with a warning until 2.0.0.

## See also

- [STABILITY.md](STABILITY.md) — what surfaces are covered by the
  compatibility promise.
- [SECURITY.md](SECURITY.md) — vulnerability reporting and the
  supported-version table.
- [CHANGELOG.md](CHANGELOG.md) — release history; the `### Deprecated`
  and `### Removed` sections are the source of truth for any given
  release.
