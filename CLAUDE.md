# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build all workspace crates (CLI + TUI)
cargo build --workspace --release

# Run all tests
cargo test --workspace

# Run tests for a single crate
cargo test -p forgeiso-engine
cargo test -p forgeiso-cli

# Run a single test by name
cargo test -p forgeiso-engine test_generate_with_firewall

# Lint (must pass before commits)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings

# Auto-format
cargo fmt --all

# Run CLI in dev mode
cargo run -p forgeiso-cli -- doctor
cargo run -p forgeiso-cli -- inject --source ubuntu.iso --out /tmp/out --username admin --password secret
```

### GUI (Tauri + React)
```bash
cd gui
npm ci
npm run lint
npm run build

# Rust backend check only (no npm needed)
cargo check --manifest-path gui/src-tauri/Cargo.toml
```

### CI (Docker, matches GitHub Actions)
```bash
# Full CI suite — exit code comes from C1 (Rust tests)
make ci-local

# Or directly:
docker compose -f docker-compose.ci.yml up --build --abort-on-container-exit --exit-code-from c1
```

### Release Packages
```bash
# Build RPM + DEB + pacman .pkg.tar.zst + tarball + checksums
bash scripts/release/make-packages.sh 0.3.1

# Install RPM locally
sudo rpm -ivh dist/release/forgeiso-0.3.1-1.x86_64.rpm
```

## Architecture

### Workspace layout

```
engine/   — core library (forgeiso-engine): all ISO logic, no I/O side effects at module boundaries
cli/      — thin CLI wrapper (forgeiso-cli): clap arg parsing → engine calls
tui/      — terminal UI (forgeiso-tui): ratatui dashboard
gui/      — Tauri desktop app: React frontend + Rust backend (separate Cargo workspace)
```

### Engine crate (`engine/src/`)

The engine is the only crate that performs real work. All other crates depend on it.

- **`orchestrator.rs`** — `ForgeIsoEngine` struct. The single entry point for all operations. Holds a `broadcast::Sender<EngineEvent>` for progress streaming. All async methods (`build`, `inject_autoinstall`, `verify`, `diff_isos`, `scan`, `test_iso`, `report`, `doctor`, `inspect_source`) live here.
- **`config.rs`** — All configuration structs: `InjectConfig`, `BuildConfig`, `ScanPolicy`, `TestingPolicy`, `IsoSource`, `SshConfig`, `NetworkConfig`, `ProxyConfig`, `UserConfig`, `FirewallConfig`, `SwapConfig`, `ContainerConfig`, `GrubConfig`. `IsoSource::from_raw()` auto-detects URL vs path.
- **`autoinstall.rs`** — Ubuntu cloud-init YAML generation. Three public functions: `generate_autoinstall_yaml(cfg)` builds YAML from scratch; `merge_autoinstall_yaml(existing, cfg)` merges CLI flags into an existing file; `hash_password(plaintext)` produces SHA-512-crypt hashes. Contains 30 unit tests.
- **`events.rs`** — `EngineEvent` with `EventPhase` (Configure, Doctor, Download, Verify, Inject, Diff, Build, Scan, Test, Report, Complete) and `EventLevel`. Subscribers call `engine.subscribe()` to get a `broadcast::Receiver`.
- **`workspace.rs`** — `Workspace::create(base, run_name)` creates a UUID-named working directory with `input/`, `work/`, `output/`, `reports/`, `scans/`, `logs/` subdirs. `safe_join()` prevents path traversal.
- **`iso.rs`** — `inspect_iso()`, `IsoMetadata`, `ResolvedIso`. Reads ISO 9660 headers to detect distro, release, arch, SHA-256.
- **`scanner.rs`** — wraps trivy, syft, grype, oscap based on `ScanPolicy`.
- **`report.rs`** — HTML and JSON report rendering.

### CLI crate (`cli/src/main.rs`)

Single file. Uses `clap` with `#[derive(Parser)]`. All subcommands are in one `Commands` enum. The `inject` variant has ~60 fields covering the full Wave 2 feature set. After arg parsing, it constructs the engine config structs and calls the appropriate `ForgeIsoEngine` method. Subscribes to the event broadcast channel and logs to stderr.

Password resolution precedence: `--password-stdin` > `--password-file` > `--password`.

### GUI (`gui/`)

- **`gui/src/App.tsx`** — Single-page React app with 4 tabs: Build, Inject, Verify, Diff. Inject tab has ~50 fields managed by `useReducer` with an `InjectState` type. Multi-value fields (SSH keys, DNS servers, ports, etc.) use newline-separated textareas converted by a `lines(s)` helper.
- **`gui/src-tauri/src/main.rs`** — Tauri command handlers. `InjectRequest` struct mirrors the frontend state and maps to `InjectConfig`. The `start_event_stream` command subscribes to engine events and emits them as `forgeiso-log` Tauri events to the frontend.
- **`gui/src/styles.css`** — All styles. Dark theme (slate palette). Key classes: `.section`/`.section-header`/`.chevron` for accordions; `.diff-entry.added/.removed/.modified` for diff view; `.tool-pill.ok/.warn` for doctor card; `.badge.phase-*` for log event badges.

### CI containers (`containers/`, `scripts/ci/`)

Six Docker containers run in parallel via `docker compose`. The pre-push hook and `make ci-local` use `--exit-code-from c1` so the exit code is always C1's (Rust fmt/clippy/tests):

| Container | What it tests |
|---|---|
| C1 | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace` |
| C3 | `npm run lint`, `npm run build`, `cargo check` on GUI |
| C4 | trivy + syft security scan on the workspace |
| C5 | Integration: builds a smoke ISO with xorriso+grub, verifies artifacts |
| C6 | E2E: same flow + QEMU BIOS/UEFI boot test if KVM available |

C2 is present (Go prototype) but not wired into `docker-compose.ci.yml`.

### Adding a new engine feature

The pattern every feature follows:
1. Add struct/field to `engine/src/config.rs`
2. Add YAML generation to `engine/src/autoinstall.rs` (`generate_autoinstall_yaml` + `merge_autoinstall_yaml` + `build_feature_late_commands`)
3. Export from `engine/src/lib.rs`
4. Add CLI flags to `cli/src/main.rs` `Commands::Inject` variant
5. Add Tauri command field to `gui/src-tauri/src/main.rs` `InjectRequest` and wire to `InjectConfig`
6. Add form field to `gui/src/App.tsx` `InjectState` + the appropriate accordion section

### Key constraints

- Engine must compile with `cargo check --manifest-path gui/src-tauri/Cargo.toml` — the GUI's `src-tauri/Cargo.toml` is a **separate** workspace that depends on `forgeiso-engine` via `path = "../../engine"`.
- `main` branch is protected: push via PR only. The pre-push hook runs full Docker CI before `git push` can proceed.
- All tests must pass and clippy must be clean (`-D warnings`) before merging.
- Version is set once in the root `Cargo.toml` `[workspace.package]` and inherited by all crates with `version.workspace = true`. GUI versions (`gui/package.json`, `gui/src-tauri/Cargo.toml`, `gui/src-tauri/tauri.conf.json`) must be bumped separately.
