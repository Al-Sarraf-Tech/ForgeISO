# CLAUDE.md — ForgeISO

Ubuntu ISO generation tool. Rust-based.

## Build

```bash
cargo build --release
```

## Test

```bash
cargo test --workspace
```

## Lint

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Rust CI Gate
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Release profile: `codegen-units = 1`, `lto = true`, `strip = true`.

## CI/CD
- Org CI must pass before pushing to personal. Runners: `linux-mega-1`, `wsl2-runner`, `dominus-runner`.

## Observability
- Structured JSON logs at `<FORGEISO_LOG_DIR or $XDG_STATE_HOME/forgeiso>/forgeiso.log.YYYY-MM-DD` (CLI), `forgeiso-tui.log.<date>`, `forgeiso-gui.log.<date>`
- Override level with `RUST_LOG` (default `info`)
- Filter: `jq 'select(.level=="WARN" or .level=="ERROR")' "$HOME/.local/state/forgeiso/forgeiso.log.$(date -I)"`

## Runbooks
- `docs/RUNBOOKS.md` — failure-mode taxonomy by error code (E001-E100) plus operational scenarios

## Benchmarks
- `cargo bench -p forgeiso-engine` — sha256, autoinstall/kickstart/preseed generators, event builder
- HTML report: `target/criterion/report/index.html`
