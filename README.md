# ForgeISO

> Build custom, unattended Linux ISOs on bare metal — no cloud agents, no remote servers, no endpoint configuration.

[![CI](https://github.com/jalsarraf0/ForgeISO/actions/workflows/ci.yml/badge.svg)](https://github.com/jalsarraf0/ForgeISO/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/jalsarraf0/ForgeISO)](https://github.com/jalsarraf0/ForgeISO/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

**Current version: v0.2.0** — early-access baseline. The GUI and engine are functional but not production-hardened.

ForgeISO injects fully automated installation configs into Linux ISOs (Ubuntu, Linux Mint, Fedora, Arch Linux, Rocky, AlmaLinux, CentOS Stream, and more) and produces bootable ISOs that install hands-free. It also inspects, verifies, diffs, scans, and smoke-tests ISOs — all from a single binary on your Linux host.

---

## Status

| Component | State |
|---|---|
| CLI (`forgeiso`) | Stable for Ubuntu and Fedora; Mint/Arch are best-effort |
| GUI (`forge-slint`) | Primary desktop GUI — Slint 1.15 wizard interface |
| GUI (`forge-gui`) | Alternate desktop GUI — egui/eframe 0.33 |
| TUI (`forgeiso-tui`) | Basic progress view only |
| CI | 6-stage Docker pipeline (Rust, SBOM, GUI, Security, Integration, E2E) |

> Ubuntu and Fedora unattended installs are CI-tested and reliable. Mint and Arch are best-effort.

---

## Distro Support

| Distro | Install method | Unattended | Notes |
|---|---|---|---|
| Ubuntu | cloud-init autoinstall | Full | CI-tested; fully supported |
| Fedora | Kickstart (ks.cfg) | Full | CI-tested; fully supported |
| Linux Mint | preseed.cfg | Full | Not CI-tested with real ISO |
| Arch Linux | archinstall JSON | Partial | Boot-time trigger; not CI-tested with real ISO |
| Rocky Linux | Kickstart | Full | Preset picker; same path as Fedora |
| AlmaLinux | Kickstart | Full | Preset picker |
| CentOS Stream | Kickstart | Full | Preset picker |

---

## ISO Presets

The GUI preset picker and `--preset` CLI flag provide direct download URLs for 35 ISOs across Ubuntu, Fedora, Linux Mint, Rocky Linux, AlmaLinux, CentOS Stream, and Arch Linux. Selecting a preset auto-fills the source URL; ForgeISO downloads and caches the ISO automatically.

```bash
forgeiso sources list
forgeiso sources show ubuntu-server-lts
```

---

## Install

Download the latest release from the **[Releases page](https://github.com/jalsarraf0/ForgeISO/releases/latest)**:

### Fedora · RHEL · openSUSE
```bash
sudo rpm -ivh forgeiso-0.2.0-1.x86_64.rpm
```

### Debian · Ubuntu · Linux Mint
```bash
sudo dpkg -i forgeiso_0.2.0-1_amd64.deb
sudo apt-get install -f        # pull in xorriso, squashfs-tools, mtools if missing
```

### Any x86-64 Linux (tarball)
```bash
tar -xzf forgeiso-0.2.0-linux-x86_64.tar.gz
sudo install -m755 forgeiso-0.2.0-linux-x86_64/bin/forgeiso /usr/local/bin/
sudo install -m755 forgeiso-0.2.0-linux-x86_64/bin/forge-slint /usr/local/bin/
```

> **Required tools:** `xorriso` · `squashfs-tools` · `mtools`
> **Optional (smoke testing):** `qemu-system-x86_64` · `ovmf`

Verify your download:
```bash
sha256sum -c checksums.txt
```

Check host prerequisites:
```bash
forgeiso doctor
```

---

## Quick Start

### GUI (recommended for new users)
```bash
forge-slint
```

The wizard walks through: **Choose ISO** → **Configure** → **Build** → **Verify**.

> On Intel integrated GPUs, set `MESA_GL_VERSION_OVERRIDE=3.3` if you see rendering issues.

### Ubuntu (fully unattended)
```bash
forgeiso inject \
  --source ubuntu-24.04-server-amd64.iso \
  --out /tmp/out \
  --hostname bastion \
  --username admin --password secret \
  --group sudo --firewall --allow-port 22 \
  --docker --no-user-interaction
```

### Fedora (Kickstart)
```bash
forgeiso inject \
  --source Fedora-Server-dvd-x86_64-40-1.14.iso \
  --out /tmp/out \
  --distro fedora \
  --hostname fedora-server \
  --username admin --password secret \
  --no-user-interaction
```

### Use a built-in preset (no local ISO needed)
```bash
forgeiso inject \
  --preset ubuntu-server-lts \
  --out /tmp/out \
  --hostname bastion \
  --username admin --password secret
```

Boot the output ISO. It installs hands-free.

---

## Commands

| Command | What it does |
|---|---|
| [`inject`](#inject) | Inject install config into an ISO |
| [`verify`](#verify) | Check ISO SHA-256 against official checksums |
| [`inspect`](#inspect) | Read distro / release / arch / hash from an ISO or URL |
| [`build`](#build) | Repack an ISO with a local overlay directory |
| [`diff`](#diff) | Compare two ISOs — added, removed, modified files |
| [`scan`](#scan) | SBOM + CVE + secrets scan on an ISO |
| [`test`](#test) | BIOS/UEFI boot smoke test via QEMU |
| [`report`](#report) | Render a build report as HTML or JSON |
| [`doctor`](#doctor) | Check host prerequisites |
| [`sources`](#sources) | List, show, and resolve built-in ISO presets |
| [`vm`](#vm) | Emit hypervisor launch scripts for a built ISO |

---

## inject

Generates a distro-appropriate install config and embeds it into the ISO.

```bash
forgeiso inject \
  --source <path-or-url.iso> \
  --out /tmp/out \
  --distro ubuntu|fedora|mint|arch \
  [OPTIONS]
```

Pass `--autoinstall user-data.yaml` (Ubuntu only) to merge flags into an existing YAML instead of generating from scratch.

### Distro selection

| Flag | Description |
|---|---|
| `--distro ubuntu` | Ubuntu cloud-init autoinstall (default) |
| `--distro fedora` | Fedora Kickstart |
| `--distro mint` | Linux Mint preseed.cfg |
| `--distro arch` | Arch Linux archinstall JSON |
| `--preset NAME` | Use a built-in ISO preset (conflicts with `--source`) |

### Identity

| Flag | Description |
|---|---|
| `--hostname NAME` | System hostname |
| `--username NAME` | Primary user login |
| `--password PASS` | Password (hashed to SHA-512-crypt) |
| `--password-file FILE` | Read password from file |
| `--password-stdin` | Read password from stdin |
| `--realname NAME` | User display name |

### SSH

| Flag | Description |
|---|---|
| `--ssh-key KEY` | Authorized public key (repeatable) |
| `--ssh-key-file FILE` | Read public key from file (repeatable) |
| `--ssh-password-auth` | Enable SSH password authentication |
| `--no-ssh-password-auth` | Disable SSH password authentication |

### Networking

| Flag | Description |
|---|---|
| `--static-ip CIDR` | Static IPv4 address, e.g. `10.0.0.5/24` |
| `--gateway IP` | Default route |
| `--dns IP` | DNS server (repeatable) |
| `--ntp-server HOST` | NTP server (repeatable) |
| `--http-proxy URL` | HTTP proxy |
| `--https-proxy URL` | HTTPS proxy |
| `--no-proxy HOST` | Proxy exception (repeatable) |

### User & access

| Flag | Description |
|---|---|
| `--group NAME` | Add user to group, e.g. `sudo`, `docker` (repeatable) |
| `--shell PATH` | Login shell, e.g. `/bin/zsh` |
| `--sudo-nopasswd` | Grant passwordless sudo |
| `--sudo-command CMD` | Restrict sudo to specific command (repeatable) |

### Firewall

| Flag | Description |
|---|---|
| `--firewall` | Enable firewall (UFW / firewalld) |
| `--firewall-policy POLICY` | Default incoming policy: `allow` \| `deny` \| `reject` |
| `--allow-port PORT` | Open port, e.g. `22/tcp`, `443` (repeatable) |
| `--deny-port PORT` | Block port (repeatable) |

### Storage

| Flag | Description |
|---|---|
| `--storage-layout NAME` | Partition layout: `lvm` \| `direct` \| `zfs` |
| `--encrypt` | Enable LUKS full-disk encryption |
| `--encrypt-passphrase PASS` | Encryption passphrase |
| `--encrypt-passphrase-file FILE` | Read passphrase from file |
| `--swap-size MB` | Create swap file of this size |
| `--swap-file PATH` | Swap file path (default `/swapfile`) |
| `--swappiness 0-100` | VM swappiness kernel parameter |
| `--mount FSTAB_LINE` | Raw fstab entry (repeatable) |

### System

| Flag | Description |
|---|---|
| `--timezone TZ` | e.g. `America/Chicago` |
| `--locale LOCALE` | e.g. `en_US.UTF-8` |
| `--keyboard-layout CODE` | e.g. `us` |
| `--apt-mirror URL` | Custom APT mirror (Ubuntu/Mint) |
| `--apt-repo REPO` | Add PPA or deb repo (repeatable; Ubuntu/Mint) |
| `--dnf-mirror URL` | Custom DNF mirror base URL (Fedora) |
| `--package NAME` | Extra package to install (repeatable) |

### Services & kernel

| Flag | Description |
|---|---|
| `--enable-service NAME` | Enable systemd service after install (repeatable) |
| `--disable-service NAME` | Disable systemd service after install (repeatable) |
| `--sysctl KEY=VALUE` | Kernel parameter written to `/etc/sysctl.d` (repeatable) |

### Containers

| Flag | Description |
|---|---|
| `--docker` | Install Docker CE |
| `--podman` | Install Podman |
| `--docker-user NAME` | Add user to `docker` group (repeatable) |

### Boot

| Flag | Description |
|---|---|
| `--grub-timeout SEC` | GRUB menu timeout in seconds |
| `--grub-cmdline PARAM` | Append kernel parameter (repeatable) |
| `--grub-default ENTRY` | Default GRUB entry |

### Commands & automation

| Flag | Description |
|---|---|
| `--run-command CMD` | Run command post-install (repeatable) |
| `--late-command CMD` | Cloud-init late-command (repeatable; Ubuntu only) |
| `--no-user-interaction` | Fully automated install, no prompts |
| `--name NAME` | Output ISO filename (without `.iso`) |
| `--volume-label LABEL` | ISO volume label |
| `--expected-sha256 HASH` | Reject source ISO if SHA-256 does not match |
| `--json` | Print result as JSON |

---

## verify

```bash
forgeiso verify --source ubuntu-24.04-server-amd64.iso
forgeiso verify --source ubuntu-24.04-server-amd64.iso \
  --sums-url https://releases.ubuntu.com/24.04/SHA256SUMS
```

---

## inspect

```bash
forgeiso inspect --source ubuntu-24.04-server-amd64.iso
forgeiso inspect --source https://releases.ubuntu.com/24.04/ubuntu-24.04-server-amd64.iso
```

---

## build

```bash
forgeiso build \
  --source ubuntu-24.04-server-amd64.iso \
  --out ./artifacts \
  --name my-server \
  --overlay ./my-overlay-dir \
  --profile minimal
```

---

## diff

```bash
forgeiso diff --base original.iso --target custom.iso
```

---

## scan

```bash
forgeiso scan --source custom.iso
```

---

## test

```bash
forgeiso test --iso custom.iso --bios --uefi
```

---

## report

```bash
forgeiso report --build ./artifacts --format html
forgeiso report --build ./artifacts --format json
```

---

## doctor

```bash
forgeiso doctor
```

Reports availability of `xorriso`, `mtools`, `unsquashfs`, `mksquashfs`,
`qemu-system-x86_64`, `trivy`, `syft`, `grype`, and `oscap`.

---

## sources

```bash
forgeiso sources list
forgeiso sources show ubuntu-server-lts
forgeiso sources resolve fedora-server
```

---

## vm

```bash
forgeiso vm emit --iso /tmp/out/custom.iso --hypervisor qemu --firmware uefi
forgeiso vm emit --iso /tmp/out/custom.iso --hypervisor virtualbox --json
```

Supported hypervisors: `qemu`, `virtualbox`, `vmware`, `hyperv`, `proxmox`.

---

## Logging

```bash
RUST_LOG=debug forgeiso inject --source ubuntu.iso --out /tmp/out \
    --username admin --password secret
```

---

## Build from Source

Requires Rust 1.75+ and `xorriso`, `squashfs-tools`, `mtools`.

```bash
git clone https://github.com/jalsarraf0/ForgeISO
cd ForgeISO
cargo build --release
```

Install binaries:
```bash
sudo install -m755 target/release/forgeiso /usr/local/bin/
sudo install -m755 target/release/forge-slint /usr/local/bin/
sudo install -m755 target/release/forge-gui /usr/local/bin/
sudo install -m755 target/release/forgeiso-tui /usr/local/bin/
```

Run tests:
```bash
cargo test --workspace        # 614 tests
cargo deny check              # license + advisory gate
```

---

## CI

Six ephemeral Docker containers run in parallel on every push.

| Stage | What it checks |
|---|---|
| C1 Rust | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace` |
| C2 SBOM + Audit | `cargo-deny`, `cargo-audit`, `syft` — license / CVE gate |
| C3 GUI | legacy Tauri GUI `cargo check` + `npm run build` |
| C4 Security | `trivy`, `syft`, `grype` against workspace |
| C5 Integration | Smoke ISO build with xorriso + grub |
| C6 E2E | QEMU BIOS/UEFI boot test (skipped if no `/dev/kvm`) |

---

## For Contributors and AI Agents

See [`AGENTS.md`](AGENTS.md) — the single source of truth for project
architecture, conventions, and rules. Claude Code and OpenAI Codex both read
this file automatically.

---

## License

[Apache-2.0](LICENSE)
