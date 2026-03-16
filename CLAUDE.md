# CLAUDE.md

**Project context, architecture, and rules are in [`AGENTS.md`](AGENTS.md).**
Read that file first. Everything below is Claude Code-specific operational guidance.

---

## Organizational Directive (Claude Only)

> **This directive applies ONLY when Claude Code is in use — it is a standing operational policy, not a suggestion.**
>
> Claude operates in this repository as a structured internal engineering organization: single point of contact, adaptive team complexity (Tier 0–4), mandatory review on all work, batch processing, and parallelization where safe. Full directive: `~/.claude/CLAUDE.md`.

---

## Parallelism

This host has 20 cores. Use them adaptively — leave headroom for the OS and other services:

```bash
# Adaptive: use all but 2 cores (or at least 1)
JOBS=$(( $(nproc) - 2 ))
JOBS=$(( JOBS < 1 ? 1 : JOBS ))
export CARGO_BUILD_JOBS=$JOBS
```

Or pass `-j $(( $(nproc) - 2 ))` to any `cargo` invocation. Do not hardcode a core count.

## Mandatory CI Gate

Before committing or pushing, the following must be clean:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -j 18 -- -D warnings
cargo test --workspace -j 18
cargo deny check
```

Run full Docker CI locally with:
```bash
make ci-local
```

All 6 stages (C1–C6) must pass. The pre-push hook enforces this automatically.

---

## Protected Branch

`main` is protected. Push via feature branch → PR only. Never push directly
to `main`. Merge with:
```bash
gh pr merge <N> --squash --admin --delete-branch
```

---

## Coding Rules

- Keep diffs narrow and reviewable.
- Do not rename files, functions, or variables without a strong reason.
- Do not reformat unrelated code.
- Do not add docstrings, comments, or type hints to code you didn't change.
- Do not introduce new dependencies without justification.
- Zero clippy warnings (`-D warnings`).
- 614 tests must remain passing; add tests for every new YAML emission path.

---

## Security Rules

- Never serialise passwords to disk. All password fields use `#[serde(skip)]`.
- Always use `workspace.safe_join()` — never raw `PathBuf::join` with user input.
- SHA-256 verification applies to the **source ISO only**, never the output ISO.
- `InjectConfig::validate()` must reject unsafe input on all structured fields.

---

## Adding a New Feature

Follow the checklist in `AGENTS.md` → "Adding a New Engine Feature".

---

## Version Bumping

Version is set once in root `Cargo.toml` `[workspace.package] version`.
The legacy Tauri GUI has separate versions in `gui/package.json`,
`gui/src-tauri/Cargo.toml`, and `gui/src-tauri/tauri.conf.json`.

---

## deny.toml Policy

- Only permissive licenses in the global allow list.
- MPL-2.0 and `LicenseRef-Slint-Royalty-free-2.0` are explicitly allowed
  (see deny.toml for rationale).
- Suppress advisories only with a documented reason.

---

## Toolchain

| Tool | Path | Version |
|---|---|---|
| rustc | `/usr/bin/rustc` | 1.93.1 (Fedora dnf) |
| cargo | `/usr/bin/cargo` | 1.93.1 (Fedora dnf) |
| rustfmt | `/usr/bin/rustfmt` | 1.93.1 |
| rust-analyzer | `/usr/bin/rust-analyzer` | 1.93.1 |
| node (Tauri GUI) | `/usr/bin/node` | v22.22.0 |

Rust is system-installed via dnf — do not invoke `rustup` to switch toolchains.
`~/.cargo/bin/` is in PATH (cargo-audit, cargo-deny).


---

## CI/CD Pipeline (Enforced)

This repository's CI/CD pipeline is **generated and managed by the Haskell CI Orchestrator** (`~/git/haskell-ci-orchestrator`). Do not manually edit `.github/workflows/ci.yml` — changes will be overwritten on the next sync.

**Directives:**
- All CI/CD runs through the unified `ci.yml` pipeline (lint → test → security → sbom → docker → integration → release)
- **Never release for macOS or Windows** — linux-only releases, no macOS/Windows runners
- **Never use the Gentoo runner** — all jobs target `[self-hosted, unified-all]`
- **Never touch `haskell-money` or `haskell-ref`** — hard-denied by the orchestrator
- Pipeline changes go through the orchestrator catalog (`CI.Catalog`), not direct YAML edits
- The orchestrator validates, generates, and syncs workflows across all 15 repos
