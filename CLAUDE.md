# CLAUDE.md

**Project context, architecture, and rules are in [`AGENTS.md`](AGENTS.md).**
Read that file first. Everything below is Claude Code-specific operational guidance.

---

## Parallelism

This host has 18 cores. Always use them:

```bash
export CARGO_BUILD_JOBS=18
```

Or pass `-j 18` to any `cargo` invocation.

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
