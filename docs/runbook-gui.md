# GUI Runbook — forge-slint

This document covers building, running, packaging, and troubleshooting the
desktop GUI frontend for ForgeISO.

---

## Overview

ForgeISO ships a desktop GUI (`forge-slint`) that wraps the `forgeiso-engine` crate:

| Binary | Crate | Toolkit | Status |
|---|---|---|---|
| `forge-slint` | `forge-slint/` | Slint 1.15 (declarative DSL) | **Primary** |

The GUI implements a 4-step wizard:

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

### For the file picker

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

# Full workspace in one pass
cargo build --workspace --release

# forge-slint only
cargo build --release -p forge-slint -j "$CARGO_BUILD_JOBS"
```

Binary lands in `target/release/forge-slint`.

### Dev builds (faster, no optimisation)

```bash
cargo build -p forge-slint -j "$CARGO_BUILD_JOBS"
```

---

## Running

Prefer the launcher first. It selects the best available frontend and falls
back cleanly when you are on a minimal desktop or a non-graphical shell:

```bash
forgeiso-desktop
```

### Direct launch

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

---

## Persistent State

State is saved to:
```
~/.local/share/forgeiso/slint-state.json
```

- All form fields are persisted **except passwords** (`#[serde(skip)]`).
- State is loaded at startup and saved when the window closes.
- To reset: `rm ~/.local/share/forgeiso/slint-state.json`

---

## Installing

After building:

```bash
sudo install -m755 target/release/forgeiso /usr/local/bin/
sudo install -m755 target/release/forgeiso-tui /usr/local/bin/
sudo install -m755 target/release/forge-slint /usr/local/bin/
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
```

The C3 CI container validates `forge-slint`. Workspace-wide checks are covered
by C1 (clippy + tests) and C7 (lint-only fast gate).

---

## Packaging

Release packages are built by `scripts/release/make-packages.sh`:

```bash
bash scripts/release/make-packages.sh 0.2.1
```

This produces RPM, DEB, pacman `.pkg.tar.zst`, tarball, and checksums
under `dist/release/`. The GUI binary is included when it is built.

---

## Common Issues

| Symptom | Cause | Fix |
|---|---|---|
| Black window / no rendering | Mesa/GPU driver issue | `MESA_GL_VERSION_OVERRIDE=3.3 forge-slint` |
| File picker shows a status-bar error | Neither `zenity` nor `kdialog` is installed | `sudo dnf install zenity` |
| Clipboard copy fails | No `wl-copy`, `xclip`, or `xsel` helper is installed | `sudo dnf install wl-clipboard xclip` |
| Open Folder shows a status-bar error | `xdg-open`/`gio` helper missing or failed | `sudo dnf install xdg-utils` |
| GUI will not launch over SSH | No graphical session is available | Use `forgeiso-desktop`, `forgeiso-tui`, or `forgeiso` |
| Build fails: missing libs | Slint system deps not installed | See Prerequisites above |
| `SLINT_BACKEND` env ignored | Slint build compiled without that backend | Rebuild without `--no-default-features` |
| State not persisted | Write error on `~/.local/share/forgeiso/` | Check directory permissions |
| Password field reset on reopen | By design (`#[serde(skip)]`) | Re-enter password each session |
