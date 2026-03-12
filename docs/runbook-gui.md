# GUI Runbook — forge-slint and forge-gui

This document covers building, running, packaging, and troubleshooting both
desktop GUI frontends for ForgeISO.

---

## Overview

ForgeISO ships two desktop GUIs that wrap the same `forgeiso-engine` crate:

| Binary | Crate | Toolkit | Status |
|---|---|---|---|
| `forge-slint` | `forge-slint/` | Slint 1.15 (declarative DSL) | **Primary** |
| `forge-gui` | `forge-gui/` | egui/eframe 0.33 | Alternate |

Both GUIs implement the same 4-step wizard:

1. **Choose ISO** — select a distro preset or paste a path/URL
2. **Configure** — set hostname, user, network, firewall, packages, etc.
3. **Build** — review settings and create the final ISO
4. **Optional Checks** — checksum and ISO-9660 validation if you want extra assurance

Step 4 is intentionally optional. A successful Build means the wizard is complete.

`forgeiso-tui` mirrors the same guided flow for terminal-first operators. The
desktop and terminal guided interfaces should describe the same steps and
completion semantics.

---

## Prerequisites

### Required system tools

```bash
# Fedora / RHEL
sudo dnf install xorriso squashfs-tools mtools

# Debian / Ubuntu
sudo apt-get install xorriso squashfs-tools mtools

# Arch Linux
sudo pacman -S libisoburn squashfs-tools mtools
```

### Optional (for smoke testing)

```bash
# Fedora
sudo dnf install qemu-system-x86 edk2-ovmf

# Debian / Ubuntu
sudo apt-get install qemu-system-x86 ovmf
```

### For the file picker (forge-slint)

`forge-slint` uses `zenity` first and falls back to `kdialog` on KDE-based systems:

```bash
# Fedora
sudo dnf install zenity

# Debian / Ubuntu
sudo apt-get install zenity

# Arch
sudo pacman -S zenity
```

`kdialog` is an acceptable alternative when `zenity` is not installed:

```bash
# Debian / Ubuntu
sudo apt-get install kdialog

# Fedora
sudo dnf install kdialog
```

---

## Building

Use an adaptive job budget on this 20-core host instead of hardcoding a stale
count:

```bash
export CARGO_BUILD_JOBS=$(( $(nproc) - 2 ))

# Both GUIs + CLI + engine in one pass
cargo build --workspace --release

# forge-slint only
cargo build --release -p forge-slint -j "$CARGO_BUILD_JOBS"

# forge-gui only
cargo build --release -p forge-gui -j "$CARGO_BUILD_JOBS"
```

Binaries land in `target/release/forge-slint` and `target/release/forge-gui`.

### Dev builds (faster, no optimisation)

```bash
cargo build -p forge-slint -j "$CARGO_BUILD_JOBS"
cargo build -p forge-gui -j "$CARGO_BUILD_JOBS"
```

---

## Running

Prefer the launcher first. It selects the best available frontend and falls
back cleanly when you are on a minimal desktop or a non-graphical shell:

```bash
forgeiso-desktop
```

### forge-slint (primary)

```bash
# Direct launch for troubleshooting
./target/release/forge-slint

# From PATH after install
forge-slint
```

#### GPU / display troubleshooting (Intel integrated GPUs)

```bash
# Intel Arc / integrated — override Mesa GL version if rendering is broken
MESA_GL_VERSION_OVERRIDE=3.3 forge-slint

# Force software renderer (slow but always works)
SLINT_BACKEND=software forge-slint

# Force winit + femtovg explicitly
SLINT_BACKEND=winit forge-slint
```

#### Wayland vs X11

`forge-slint` auto-detects the display server. To force one:

```bash
# Force X11 on a Wayland session
DISPLAY=:0 WAYLAND_DISPLAY="" forge-slint

# Force Wayland on an X11 session
WAYLAND_DISPLAY=wayland-0 DISPLAY="" forge-slint
```

#### Clipboard

The `Copy SHA-256` button uses `wl-copy` on Wayland when available, then
falls back to `xclip` and `xsel`:

```bash
# Fedora
sudo dnf install wl-clipboard xclip

# Debian / Ubuntu
sudo apt-get install wl-clipboard xclip
```

#### Headless and minimal systems

`forge-slint` still requires a graphical session. On headless hosts, use:

```bash
forgeiso-tui
# or
forgeiso
```

The packaged launcher `forgeiso-desktop` auto-detects this and falls back to
TUI/CLI when no display server is available.

### forge-gui (egui/eframe)

```bash
# Standard launch
./target/release/forge-gui

# Intel Arc / integrated GPU — avoid Vulkan stability issues
WGPU_BACKEND=gl forge-gui

# Force OpenGL ES explicitly
WGPU_BACKEND=gles forge-gui
```

---

## Persistent State

### forge-slint

State is saved to:
```
~/.local/share/forgeiso/slint-state.json
```

- All form fields are persisted **except passwords** (`#[serde(skip)]`).
- State is loaded at startup and saved when the window closes.
- To reset: `rm ~/.local/share/forgeiso/slint-state.json`

### forge-gui

State is saved via the eframe storage API to:
```
~/.local/share/forgeiso/  (Linux XDG)
```

Persist key: `"forgeiso_v1"`. If the schema breaks, bump to `"forgeiso_v2"`
in `forge-gui/src/app.rs`.

---

## Installing

After building:

```bash
sudo install -m755 target/release/forgeiso /usr/local/bin/
sudo install -m755 target/release/forgeiso-tui /usr/local/bin/
sudo install -m755 target/release/forge-slint /usr/local/bin/
sudo install -m755 target/release/forge-gui   /usr/local/bin/
sudo install -m755 scripts/release/forgeiso-desktop /usr/local/bin/
```

From the RPM/DEB package, binaries are placed in `/usr/bin/` automatically.
Prefer launching `forgeiso-desktop` so the best available frontend is selected.
Install `zenity` or `kdialog`, `wl-clipboard` or `xclip`/`xsel`, and
`xdg-utils` explicitly on minimal desktop systems when your package manager
does not pull them in automatically.

If you later install an RPM/DEB/pacman package, remove stale `/usr/local/bin`
ForgeISO binaries first. `/usr/local/bin` shadows `/usr/bin`, so manual
tarball installs can mask packaged upgrades.

---

## Linting and CI Checks

```bash
# Format check
cargo fmt --all --check

# Lint (zero warnings allowed)
cargo clippy --workspace --all-targets -j 18 -- -D warnings

# forge-gui specific (C3 CI gate)
cargo fmt --manifest-path forge-gui/Cargo.toml --all --check
cargo clippy -p forge-gui --all-targets -j 18 -- -D warnings
cargo build -p forge-gui -j 18
```

The C3 CI container validates `forge-gui`. `forge-slint` is covered by C1
(workspace clippy + tests) and C7 (lint-only fast gate).

---

## Packaging

Release packages are built by `scripts/release/make-packages.sh`:

```bash
bash scripts/release/make-packages.sh 0.2.1
```

This produces RPM, DEB, pacman `.pkg.tar.zst`, tarball, and checksums
under `dist/release/`. Both GUI binaries are included when they are built.

---

## Legacy Tauri GUI (`gui/`)

The `gui/` directory contains an older Tauri 2 + React frontend. It is
kept for CI validation (C3 builds it) but **not recommended for end users**.

### Dependencies

```bash
cd gui
npm ci                 # install Node deps
npm run lint           # TypeScript lint
npm run build          # Vite + Tauri bundle (Linux)
```

Requires Node.js 20+ and Rust (for the Tauri backend).

### Key versions (as of v0.2.1)

| Package | Version |
|---|---|
| `@tauri-apps/api` | 2.10.1 |
| `react` | ^18.3.1 |
| `@tauri-apps/cli` | 2.10.1 |
| TypeScript | ^5.8.3 |
| Vite | ^6.3.5 |
| `tauri` (Rust) | 2.10.3 |

To check the Rust backend only (no npm required):

```bash
cargo check --manifest-path gui/src-tauri/Cargo.toml
```

---

## Common Issues

| Symptom | Cause | Fix |
|---|---|---|
| Black window / no rendering | Mesa/GPU driver issue | `MESA_GL_VERSION_OVERRIDE=3.3 forge-slint` |
| `wgpu` crash on launch | Vulkan not available for forge-gui | `WGPU_BACKEND=gl forge-gui` |
| File picker shows a status-bar error | Neither `zenity` nor `kdialog` is installed | `sudo dnf install zenity` |
| Clipboard copy fails | No `wl-copy`, `xclip`, or `xsel` helper is installed | `sudo dnf install wl-clipboard xclip` |
| Open Folder shows a status-bar error | `xdg-open`/`gio` helper missing or failed | `sudo dnf install xdg-utils` |
| GUI will not launch over SSH | No graphical session is available | Use `forgeiso-desktop`, `forgeiso-tui`, or `forgeiso` |
| Build fails: missing libs | Slint system deps not installed | See Prerequisites above |
| `SLINT_BACKEND` env ignored | Slint build compiled without that backend | Rebuild without `--no-default-features` |
| State not persisted | Write error on `~/.local/share/forgeiso/` | Check directory permissions |
| Password field reset on reopen | By design (`#[serde(skip)]`) | Re-enter password each session |
