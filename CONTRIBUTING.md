# Contributing to ForgeISO

Thanks for considering a contribution. ForgeISO is a Rust workspace that
turns upstream Linux installer ISOs into unattended-install variants,
shipped as a CLI (`forgeiso`), a TUI (`forgeiso-tui`), and a desktop GUI
(`forge-slint`).

## Before you start

- Read [`README.md`](README.md) — particularly the "Architecture" and
  "Supported Distros" sections.
- Skim [`docs/adr/README.md`](docs/adr/README.md). The 12 ADRs document
  every load-bearing decision; if your change touches one (workspace
  layout, error model, module taxonomy, testing strategy, reliability
  contract, security contract, observability contract, profile catalog),
  read the relevant ADR before opening the PR.
- Check the [issue tracker](https://github.com/Al-Sarraf-Tech/ForgeISO/issues)
  for an existing report. Open a new issue first if your change is
  user-visible or non-trivial — alignment is faster than a rejected PR.

## Development setup

### Requirements

- Rust **stable** ≥ 1.87 (`rustup show` shows your active toolchain).
- `xorriso`, `mksquashfs`, `unsquashfs`, `mkisofs` for ISO operations.
- `qemu-system-x86_64` for VM smoke tests (optional).
- Linux host. macOS/Windows builds are not supported per project policy.

### Toolchain bootstrap

```bash
git clone https://github.com/Al-Sarraf-Tech/ForgeISO.git
cd ForgeISO
cargo build --workspace
cargo test --workspace
```

### Recommended dev tools

```bash
cargo install cargo-tarpaulin    # coverage gate
cargo install cargo-mutants      # mutation testing
cargo install cargo-public-api   # API contract regen (needs nightly)
cargo install cargo-audit        # vuln scan
```

## Code style

- `cargo fmt --all` before every commit. CI fails on diff.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  must pass. CI fails on any warning.
- Files target ≤500 LOC after the Phase 7 decomposition. New monoliths
  are bounced — split by concern.
- No `unsafe` blocks in shipping paths without an ADR justifying them.
- Public API: every `pub` item should carry a `///` doc comment when
  its purpose is not obvious from the signature.

## Test discipline

ForgeISO uses a seven-layer testing strategy (ADR 0007). Your change
should land tests at the layer it touches:

- **Unit** (`#[cfg(test)] mod tests` next to the source) — pure logic
- **Property** (`engine/tests/proptest_config.rs`) — invariants on
  inputs you can describe with `proptest` strategies
- **Integration** (`engine/tests/*_regression.rs`) — multi-module flows
- **Chaos** (`engine/tests/chaos.rs`) — fault-injection
- **Contract** (`engine/tests/api_contract.rs`) — public-API surface
  golden file
- **Mutation** (`scripts/run-mutants.sh`) — kill-score on critical
  paths
- **Perf** (`scripts/perf-bench.sh`) — regression gate vs
  `tests/baseline-perf.json`

Run the standard CI gate locally before pushing:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

For a fuller sweep:

```bash
bash scripts/s-tier-audit.sh --fast    # pre-commit version
bash scripts/s-tier-audit.sh           # full audit (slow)
```

## Commit + PR conventions

- **Conventional Commits**: `feat:`, `fix:`, `docs:`, `test:`, `chore:`,
  `refactor:`, `perf:`, `ci:`. Breaking changes use `feat!:` or include
  `BREAKING CHANGE:` in the footer.
- **No AI co-author attribution.** Per the Al-Sarraf-Tech repo policy
  (CLAUDE.md absolute), commits and PRs do not include `Co-Authored-By`
  lines or PR metadata referencing AI assistants.
- **One logical change per commit.** Easier to revert, easier to review.
- **PR title** = the commit title for single-commit PRs; otherwise a
  one-line summary of the work.
- **PR description**:
  - **Summary** — what changes and why
  - **Test plan** — what you ran locally
  - **Risk** — what could break, what you checked

## Branching

- Feature branches off `main` at `Al-Sarraf-Tech/ForgeISO`.
- Branch protection allows direct pushes to `main` for the maintainer;
  external contributors open PRs.
- Conventional branch names: `feat/<short-name>`, `fix/<short-name>`,
  `refactor/<short-name>`, `docs/<short-name>`.

## Release process

See [`docs/runbook-release.md`](docs/runbook-release.md). Releases are
cut by the maintainer on `Al-Sarraf-Tech/ForgeISO`, signed via cosign
keyless, and published as GitHub Releases.

## Code of Conduct

This project adopts the [Contributor Covenant 2.1](CODE_OF_CONDUCT.md).
By participating, you agree to abide by its terms.

## License

By contributing to ForgeISO, you agree that your contributions are
licensed under the Apache License 2.0 (see [`LICENSE`](LICENSE)).
