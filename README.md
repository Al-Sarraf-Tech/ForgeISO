# ForgeISO

> Build custom, unattended Linux ISOs on bare metal — no cloud agents, no remote servers, no endpoint configuration.

[![CI](https://github.com/jalsarraf0/ForgeISO/actions/workflows/ci.yml/badge.svg)](https://github.com/jalsarraf0/ForgeISO/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/jalsarraf0/ForgeISO)](https://github.com/jalsarraf0/ForgeISO/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

**Current version: v0.2.0** — early-access baseline. The GUI and engine are functional but not production-hardened.

ForgeISO injects fully automated installation configs into Linux ISOs (Ubuntu, Linux Mint, Fedora, Arch Linux, Rocky, AlmaLinux, CentOS Stream, and more) and produces bootable ISOs that install hands-free. It also inspects, verifies, diffs, scans, and smoke-tests ISOs — all from a single binary on your Linux host.

A pure-Rust egui/eframe desktop GUI (`forge-gui`) provides a wizard interface alongside the CLI.

---

## Status

| Component | State |
|---|---|
| CLI (`forgeiso`) | Stable for Ubuntu and Fedora; Mint/Arch are best-effort |
| GUI (`forge-gui`) | Functional — 3-step inject wizard, 35 ISO presets, Doctor panel |
| TUI (`forgeiso-tui`) | Basic progress view only |
| CI | 6-stage Docker pipeline (Rust, SBOM, GUI, Security, Integration, E2E) |

> This is v0.2.0. Expect rough edges, especially on Mint and Arch. Ubuntu and Fedora unattended installs are CI-tested and reliable.

---

## Distro Support

| Distro | Install method | Unattended | Notes |
|---|---|---|---|
| Ubuntu | cloud-init autoinstall | Full | CI-tested; fully supported |
| Fedora | Kickstart (ks.cfg) | Full | CI-tested; fully supported |
| Linux Mint | preseed.cfg | Full | Not CI-tested with real ISO |
| Arch Linux | archinstall JSON config | Partial | Boot-time trigger; not CI-tested with real ISO |
| Rocky Linux | Kickstart | Full | Preset picker only; same Kickstart path as Fedora |
| AlmaLinux | Kickstart | Full | Preset picker only |
| CentOS Stream | Kickstart | Full | Preset picker only |

See [docs/distro-support.md](docs/distro-support.md) for details.

---

## ISO Presets (35 total)

The GUI preset picker and `--preset` CLI flag provide direct download URLs for:

- **Ubuntu** — Server LTS, Desktop LTS, Mini, Server 22.04, Server 24.10, and more (10 presets)
- **Fedora** — Server, Workstation, KDE, Minimal, Net Install (5 presets)
- **Linux Mint** — Cinnamon, MATE, XFCE (3 presets)
- **Rocky Linux** — Boot, Minimal, DVD (3 presets)
- **AlmaLinux** — Boot, Minimal, DVD (3 presets)
- **CentOS Stream** — Boot, DVD (2 presets)
- **Arch Linux** — x86_64 current (1 preset)
- **RHEL** — Customer portal URL placeholder (1 preset)

---

## Install

Download the latest release from the **[Releases page](https://github.com/jalsarraf0/ForgeISO/releases/latest)**, then:

### Fedora · RHEL · openSUSE
```bash
sudo rpm -ivh forgeiso-0.2.0-1.x86_64.rpm
```

### Debian · Ubuntu · Linux Mint
```bash
sudo dpkg -i forgeiso_0.2.0-1_amd64.deb
sudo apt-get install -f        # resolve xorriso, squashfs-tools, mtools if missing
```

### Any x86-64 Linux (tarball)
```bash
tar -xzf forgeiso-0.2.0-linux-x86_64.tar.gz
sudo install -m755 forgeiso-0.2.0-linux-x86_64/bin/forgeiso /usr/local/bin/
sudo install -m755 forgeiso-0.2.0-linux-x86_64/bin/forgeiso-tui /usr/local/bin/
sudo install -m755 forgeiso-0.2.0-linux-x86_64/bin/forge-gui /usr/local/bin/
```

> **Dependencies:** `xorriso` · `squashfs-tools` · `mtools`
> Optional for smoke testing: `qemu-system-x86_64` · `ovmf`

Verify your download:
```bash
sha256sum -c checksums.txt
```

Check what's available on your host:
```bash
forgeiso doctor
```

---

## Quick start

### GUI (recommended for new users)
```bash
WGPU_BACKEND=gl forge-gui
```

The wizard walks through: **Get ISO** → **Configure** → **Run**. Select a preset from the dropdown to auto-fill the source URL.

> On Intel Arc and other integrated GPUs, `WGPU_BACKEND=gl` avoids Vulkan stability issues.

### Ubuntu (fully unattended cloud-init autoinstall)
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

### Use a built-in preset instead of a local ISO
```bash
forgeiso inject \
  --preset ubuntu-server-lts \
  --out /tmp/out \
  --hostname bastion \
  --username admin --password secret
```

Boot the output ISO. It installs your configuration hands-free (Ubuntu and Fedora) or triggers the configured installer at boot (Mint and Arch).

---

## Commands

| Command | What it does |
|---|---|
| [`inject`](#inject) | Inject install config into an ISO (Ubuntu/Fedora/Mint/Arch) |
| [`verify`](#verify) | Check ISO SHA-256 against official checksums |
| [`inspect`](#inspect) | Read distro/release/arch/hash from an ISO or URL |
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

Generates a distro-appropriate install config and embeds it into the ISO. Use `--distro` to specify the target distro (defaults to Ubuntu). Use `--preset` to download and use a known ISO automatically.

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
| `--distro fedora` | Fedora Kickstart (ks.cfg) |
| `--distro mint` | Linux Mint preseed.cfg via Calamares |
| `--distro arch` | Arch Linux archinstall JSON config |
| `--preset NAME` | Use a built-in ISO preset (conflicts with `--source`) |

### Identity

| Flag | Description |
|---|---|
| `--hostname NAME` | System hostname |
| `--username NAME` | Primary user login |
| `--password PASS` | Password (auto-hashed to SHA-512-crypt) |
| `--password-file FILE` | Read password from file |
| `--password-stdin` | Read password from stdin |
| `--realname NAME` | User display name |

### SSH

| Flag | Description |
|---|---|
| `--ssh-key KEY` | Authorized public key (repeatable) |
| `--ssh-key-file FILE` | Read public key from file (repeatable) |
| `--ssh-password-auth` | Enable SSH password authentication |
| `--no-ssh-password-auth` | Explicitly disable SSH password authentication |

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
| `--sudo-nopasswd` | Grant passwordless sudo (`NOPASSWD:ALL`) |
| `--sudo-command CMD` | Restrict sudo to specific command (repeatable) |

### Firewall

| Flag | Description |
|---|---|
| `--firewall` | Enable firewall (UFW for Ubuntu/Mint, firewalld for Fedora) |
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

Computes the SHA-256 of a local ISO and checks it against the official checksums file. Auto-detects the checksums URL from the ISO metadata for Ubuntu ISOs.

```bash
forgeiso verify --source ubuntu-24.04-server-amd64.iso
```

Override the checksums URL:
```bash
forgeiso verify \
  --source ubuntu-24.04-server-amd64.iso \
  --sums-url https://releases.ubuntu.com/24.04/SHA256SUMS
```

---

## inspect

Reads distro, release, architecture, and SHA-256 from a local ISO or a URL (ForgeISO downloads to `~/.cache/forgeiso` first).

```bash
forgeiso inspect --source ubuntu-24.04-server-amd64.iso
forgeiso inspect --source https://releases.ubuntu.com/24.04/ubuntu-24.04-server-amd64.iso
```

---

## build

Repacks an ISO with a local overlay directory merged into the root.

```bash
forgeiso build \
  --source ubuntu-24.04-server-amd64.iso \
  --out ./artifacts \
  --name my-server \
  --overlay ./my-overlay-dir \
  --profile minimal
```

`--profile` is `minimal` (default) or `desktop`.

---

## diff

Compares two ISOs and lists files that were added, removed, or modified, with size deltas.

```bash
forgeiso diff --base original.iso --target custom.iso
```

---

## scan

Runs SBOM generation, CVE scanning, and secrets detection against ISO contents. Uses whichever of `syft`, `trivy`, `grype` are installed.

```bash
forgeiso scan --source custom.iso
```

---

## test

Boots the ISO in QEMU and verifies it reaches the boot menu. Requires `qemu-system-x86_64` and `ovmf`.

```bash
forgeiso test --iso custom.iso --bios --uefi
```

---

## report

Renders the build report for an output directory.

```bash
forgeiso report --build ./artifacts --format html
forgeiso report --build ./artifacts --format json
```

---

## doctor

```bash
forgeiso doctor
```

Reports availability of `xorriso`, `mtools`, `unsquashfs`, `mksquashfs`, `qemu-system-x86_64`, `trivy`, `syft`, `grype`, and `oscap`. Also shows per-distro inject readiness.

---

## sources

List and resolve built-in ISO presets.

```bash
forgeiso sources list
forgeiso sources show ubuntu-server-lts
forgeiso sources resolve fedora-server
```

---

## vm

Emit hypervisor-specific launch scripts for a built ISO.

```bash
forgeiso vm emit \
  --iso /tmp/out/custom.iso \
  --hypervisor qemu \
  --firmware uefi

forgeiso vm emit \
  --iso /tmp/out/custom.iso \
  --hypervisor virtualbox \
  --json
```

Supported hypervisors: `qemu`, `virtualbox`, `vmware`, `hyperv`, `proxmox`.

---

## Logging

```bash
RUST_LOG=debug forgeiso inject --source ubuntu.iso --out /tmp/out --username admin --password secret
```

Valid levels: `error` · `warn` · `info` · `debug` · `trace`

---

## Build from source

Requires Rust 1.75+ and the system tools listed above.

```bash
git clone https://github.com/jalsarraf0/ForgeISO
cd ForgeISO
cargo build --release -p forgeiso-cli
sudo install -m755 target/release/forgeiso /usr/local/bin/
```

Run tests:
```bash
cargo test --workspace
```

GUI (egui/eframe desktop app):
```bash
cargo build --release -p forge-gui
sudo install -m755 target/release/forge-gui /usr/local/bin/
# Launch (use WGPU_BACKEND=gl on Intel/AMD integrated GPUs)
WGPU_BACKEND=gl forge-gui
```

TUI (ratatui terminal UI):
```bash
cargo build --release -p forgeiso-tui
sudo install -m755 target/release/forgeiso-tui /usr/local/bin/
```

---

## CI

Six ephemeral Docker containers run in parallel on every push. All containers are removed after the job completes.

| Stage | What it tests |
|---|---|
| C1 Rust | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace` |
| C2 SBOM + Audit | `cargo-deny`, `cargo-audit`, `syft` license/CVE check |
| C3 GUI | GUI crate `cargo check` |
| C4 Security | `trivy`, `syft`, `grype` against workspace |
| C5 Integration | Smoke ISO build with xorriso + grub |
| C6 E2E | QEMU BIOS/UEFI boot test (skipped if no `/dev/kvm`) |

---

## License

[Apache-2.0](LICENSE)
