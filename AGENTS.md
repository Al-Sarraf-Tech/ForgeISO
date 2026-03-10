# ForgeISO — Agent Briefing

> **Single source of truth for AI agents (Claude Code, Codex, etc.)**
> Read this file before making any changes. It describes what the project is,
> how it is structured, how to build and test it, and the rules that must be
> followed.

---

## What ForgeISO Is

ForgeISO is a Rust tool that takes a stock Linux ISO and produces a customised,
unattended-install ISO. It supports Ubuntu (cloud-init autoinstall), Fedora /
RHEL-family (Kickstart), Arch Linux (archinstall JSON), and Linux Mint
(Calamares preseed).

The project ships four user-facing artefacts:

| Binary | Crate | What it is |
|---|---|---|
| `forgeiso` | `cli/` | Full-featured CLI |
| `forgeiso-tui` | `tui/` | Ratatui terminal dashboard |
| `forge-gui` | `forge-gui/` | egui/eframe 0.33 desktop GUI |
| `forge-slint` | `forge-slint/` | Slint 1.15 desktop GUI (current primary) |

---

## Repository Layout

```
engine/          — forgeiso-engine   core library; all ISO logic lives here
cli/             — forgeiso-cli      thin clap CLI → engine calls
tui/             — forgeiso-tui      ratatui terminal UI
forge-gui/       — forge-gui         egui/eframe 0.33 desktop GUI
forge-slint/     — forge-slint       Slint 1.15 desktop GUI
  ui/            — .slint DSL source files (compiled at build time)
  src/           — Rust host code (app.rs, state.rs, worker.rs, persist.rs, main.rs)
gui/             — legacy Tauri/React GUI (separate Cargo workspace; still built by C3 CI)
engine/src/      — orchestrator.rs, config.rs, autoinstall.rs, events.rs,
                   workspace.rs, iso.rs, scanner.rs, report.rs, sources.rs, vm.rs
containers/      — C1–C6 Dockerfiles for CI
scripts/ci/      — c1-rust.sh … c6-e2e.sh run inside the containers
scripts/release/ — make-packages.sh, bump-version.sh
docs/            — runbook-release.md and other docs
deny.toml        — cargo-deny license + advisory policy
```

---

## Version

Current workspace version: **0.2.0** (set in root `Cargo.toml`
`[workspace.package] version`; all crates inherit it with
`version.workspace = true`).

The legacy Tauri GUI (`gui/`) has its own version in `gui/package.json`,
`gui/src-tauri/Cargo.toml`, and `gui/src-tauri/tauri.conf.json` — bump
those separately when releasing.

---

## Build & Test

```bash
# Build every crate
cargo build --workspace --release

# Run all 614 tests
cargo test --workspace

# Run tests for one crate
cargo test -p forgeiso-engine
cargo test -p forgeiso-cli

# Run a single test by name
cargo test -p forgeiso-engine test_generate_with_firewall

# Lint gate (must be clean before any commit)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings

# Auto-format
cargo fmt --all

# License + advisory gate
cargo deny check

# CLI smoke run
cargo run -p forgeiso-cli -- doctor
cargo run -p forgeiso-cli -- inject --source ubuntu.iso --out /tmp/out \
    --username admin --password secret
```

### Legacy Tauri GUI

```bash
cd gui
npm ci && npm run lint && npm run build
cargo check --manifest-path gui/src-tauri/Cargo.toml
```

### Full CI locally (Docker, matches GitHub Actions pre-push hook)

```bash
make ci-local
# or:
docker compose -f docker-compose.ci.yml up --build \
    --abort-on-container-exit --exit-code-from c1
```

---

## CI Pipeline

Six Docker containers run in parallel. **All six must pass** before a push
reaches GitHub (enforced by `.git/hooks/pre-push`).

| ID | Label | Fails on |
|---|---|---|
| C1 | Rust — fmt / clippy / tests | any format, lint, or test failure |
| C2 | SBOM + Audit — cargo-deny / cargo-audit / syft | license violations, CVE advisories |
| C3 | GUI — tsc / vite / cargo check | GUI build or type errors |
| C4 | Security — trivy / syft / grype | high-severity CVEs |
| C5 | Integration — xorriso smoke ISO | ISO build or inject failure |
| C6 | E2E Smoke — QEMU BIOS/UEFI boot | boot test failure (skipped if no `/dev/kvm`) |

C1 image: `rust:1.93-bookworm` + system libs for Slint
(`libxkbcommon-dev libwayland-dev libegl-dev libgl-dev libfontconfig1-dev
libdbus-1-dev libx11-dev libxcb-shape0-dev libxcb-xfixes0-dev`).

---

## Engine Architecture

`ForgeIsoEngine` in `engine/src/orchestrator.rs` is the single entry point
for all operations. It holds a `broadcast::Sender<EngineEvent>` for progress
streaming; callers subscribe with `engine.subscribe()`.

### Key engine modules

| File | Purpose |
|---|---|
| `orchestrator.rs` | `ForgeIsoEngine` struct; all async methods (`build`, `inject_autoinstall`, `verify`, `diff_isos`, `scan`, `test_iso`, `report`, `doctor`, `inspect_source`) |
| `config.rs` | All config structs: `InjectConfig`, `BuildConfig`, `ScanPolicy`, `IsoSource`, `SshConfig`, `NetworkConfig`, `ProxyConfig`, `UserConfig`, `FirewallConfig`, `SwapConfig`, `ContainerConfig`, `GrubConfig` |
| `autoinstall.rs` | Ubuntu cloud-init YAML: `generate_autoinstall_yaml(cfg)`, `merge_autoinstall_yaml(existing, cfg)`, `hash_password(plaintext)` |
| `events.rs` | `EngineEvent` with `EventPhase` and `EventLevel` |
| `workspace.rs` | `Workspace::create(base, run_name)` — UUID subdirs; `safe_join()` prevents path traversal |
| `iso.rs` | `inspect_iso()`, `IsoMetadata`, `ResolvedIso` — reads ISO 9660 headers |
| `sources.rs` | 10 built-in distro presets (`ubuntu-server-lts`, `arch-linux`, etc.) |
| `vm.rs` | `Hypervisor` enum, `VmLaunchSpec`, `emit_launch()` → launch commands |
| `scanner.rs` | Wraps trivy / syft / grype / oscap per `ScanPolicy` |
| `report.rs` | HTML and JSON report rendering |

### IsoSource

`IsoSource::from_raw(s)` auto-detects URL vs local path. Always use this
constructor — never construct `IsoSource` variants directly in new code.

### Config field conventions

- `SshConfig`: `install_server: bool`, `allow_password_auth: Option<bool>`
- `NetworkConfig`: `dns_servers: Vec<String>`, `ntp_servers: Vec<String>`
  (static IP / gateway are top-level fields on `InjectConfig`)
- `FirewallConfig`: `enabled: bool`, `default_policy: String`,
  `allow_ports: Vec<String>`, `deny_ports: Vec<String>`
- `SwapConfig`: `size_mb: u32`, `filename: String`, `swappiness: Option<u32>`
- `ContainerConfig`: `docker: bool`, `podman: bool`, `docker_users: Vec<String>`
- `GrubConfig`: `timeout: Option<u32>`, `cmdline_extra: Vec<String>`,
  `default_entry: Option<String>`
- `InjectConfig`: `out_name: String` (no `output_dir` field; dir is separate
  arg), `extra_packages: Vec<String>`, `extra_late_commands: Vec<String>`

---

## forge-slint GUI Architecture

`forge-slint/` is the primary desktop GUI. Entry binary: `forge-slint`.

### Slint DSL files (`forge-slint/ui/`)

```
app-window.slint      — root AppWindow component; wires all steps
theme.slint           — Palette + Sizes global structs
steps/source.slint    — step 1: distro picker + ISO path
steps/configure.slint — step 2: all inject fields (hostname, user, packages…)
steps/build.slint     — step 3: config summary + build button
steps/check.slint     — step 4: checksum verify + ISO-9660 inspect
components/           — reusable widgets (log-panel, progress-bar, etc.)
```

### Rust host files (`forge-slint/src/`)

| File | Purpose |
|---|---|
| `main.rs` | Entry point: builds tokio runtime, creates `ForgeIsoEngine`, wires all 22 Slint callbacks via `win.on_*()`, loads/saves persisted state |
| `app.rs` | `ForgeApp` struct; `thread_local! APP`; `with_app()` accessor; all `spawn_*` methods that call engine async ops and pipe `EngineEvent`s back via `slint::invoke_from_event_loop` |
| `state.rs` | `InjectState`, `VerifyState`, form-level data structures |
| `worker.rs` | `WorkerMsg` enum; zenity file-picker helpers |
| `persist.rs` | `PersistedState` (serde); load/save to `~/.local/share/forge-slint/` |

### Threading model

`Rc<RefCell<ForgeApp>>` lives in a `thread_local!`. Worker closures are
`Send + 'static` and access the app via `APP.with(|cell| …)` — they must
**never** capture the `Rc` directly.

Engine events flow: `engine.subscribe()` → tokio task →
`slint::invoke_from_event_loop` → `APP.with(…).push_log(…)`.

Password field (`InjectState::password`) has `#[serde(skip)]` — never
written to disk.

---

## Adding a New Engine Feature

Follow this checklist in order:

1. `engine/src/config.rs` — add struct field or new struct
2. `engine/src/autoinstall.rs` — add YAML emission in
   `generate_autoinstall_yaml`, `merge_autoinstall_yaml`,
   `build_feature_late_commands`
3. `engine/src/lib.rs` — export if new public type
4. `cli/src/main.rs` — add `clap` flag(s) to `Commands::Inject`
5. `forge-slint/ui/steps/configure.slint` — add UI field
6. `forge-slint/src/state.rs` — add to `InjectState`
7. `forge-slint/src/app.rs` — wire field in `build_inject_config()`
8. `forge-slint/src/main.rs` — restore field in `restore_inject()` if persistent
9. Write a unit test in `engine/src/autoinstall.rs` (inline `#[cfg(test)]`)

---

## Key Rules

- **Protected branch**: `main` requires passing CI. Push via feature branch →
  PR only. The pre-push hook runs full Docker CI.
- **Clippy**: `-D warnings` — zero warnings allowed.
- **Tests**: all 614 must pass. Add tests for every new YAML emission path.
- **Passwords**: never serialise to disk. `#[serde(skip)]` on all password
  fields.
- **Path traversal**: always use `workspace.safe_join()`, never raw
  `PathBuf::join` with user-supplied input.
- **SHA-256 checks**: apply to source ISO only, never to output ISO.
- **Distro inference**: `--preset rocky-linux/almalinux/centos-stream`
  auto-selects Kickstart (`Distro::Fedora`). `--preset debian/opensuse`
  emits a warning (unsupported format, best-effort fallback).
- **deny.toml**: `cargo deny check` must be clean. Suppress advisories only
  with a documented reason and tracked issue.
- **No broad refactors**: keep diffs narrow; do not rename files/functions
  without a strong reason; do not reformat unrelated code.

---

## Distro Preset Tags

| `--preset` value | Distro enum | Installer format |
|---|---|---|
| `ubuntu-server-lts`, `ubuntu-desktop-lts` | Ubuntu | cloud-init autoinstall |
| `linux-mint-cinnamon` | Mint | Calamares preseed |
| `fedora-server`, `fedora-workstation` | Fedora | Kickstart |
| `rocky-linux`, `almalinux`, `centos-stream` | Fedora (Kickstart) | Kickstart |
| `arch-linux` | Arch | archinstall JSON |
| `rhel-custom` | Fedora | Kickstart |
| `debian`, `opensuse` | (fallback) | WARNING: unsupported |

---

## Suppressed Advisories (deny.toml)

| Advisory | Reason |
|---|---|
| RUSTSEC-2025-0119 | `number_prefix` unmaintained; no vuln; indicatif dep, no upgrade |
| RUSTSEC-2024-0436 | `paste` unmaintained; no vuln; rav1e (via Slint femtovg→image) dep, no upgrade |

Allowed non-standard licenses: `OFL-1.1`, `Ubuntu-font-1.0` (egui fonts),
`LicenseRef-Slint-Royalty-free-2.0` (Slint), `MPL-2.0` (option-ext via dirs).
