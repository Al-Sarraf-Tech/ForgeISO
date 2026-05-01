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

## Architecture Decisions
- `docs/adr/README.md` — index of load-bearing decisions; read these before changing the workspace layout, ISO repack tool, front-end strategy, error taxonomy, or distro injection logic

## Benchmarks + Perf gate
- `cargo bench -p forgeiso-engine` — sha256, autoinstall/kickstart/preseed generators, event builder
- HTML report: `target/criterion/report/index.html`
- `scripts/perf-bench.sh bench|compare|capture` — fail PR on >15% p99 regression vs `tests/baseline-perf.json`

## SLOs + Compliance
- `docs/SLO.md` — per-op SLOs (sha256 p99 <50ms, generators <20ms, event <1µs, build <5min, GUI cold start <2s, theme toggle <50ms)
- `docs/COMPLIANCE.md` — NIST 800-53 / CIS v8 / SOC 2 control mapping (self-attestation)

## Single-command audit
- `scripts/s-tier-audit.sh` — fmt + lint + test + coverage + build + perf + security + docs (8 dimensions). `--fast` for pre-commit.

## Module layout (post-decomposition, S+ uplift 2026-05-01)
- `engine/src/config/inject/` — InjectConfig + per-concern validators (identity/system/packages/network/ssh/storage/grub/output) + per-concern test modules
- `engine/src/autoinstall/ubuntu/` — generate.rs + merge.rs + tests.rs
- `engine/src/sources/` — catalog (presets) + preset_id + preset + strategy
- `engine/src/vm/` — launch + qemu + spec + hypervisor + per-runtime modules (proxmox/hyperv/vmware/vbox/firmware/ovmf)
- `forge-slint/src/handlers/` — per-step callback wiring (source/configure/build/check/common); main.rs is orchestration only
- `forge-slint/ui/steps/configure/` — per-tab files (tab_identity/tab_access/tab_system/tab_services/tab_storage)
- `tui/src/state/` — App + per-screen submodules (nav/fields/source/build/configure)
