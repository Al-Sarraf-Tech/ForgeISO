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
