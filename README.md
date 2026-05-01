# ForgeISO

> Turn a stock Linux ISO into a hands-free installer from your own Linux machine.

[![CI](https://github.com/Al-Sarraf-Tech/ForgeISO/actions/workflows/ci-rust.yml/badge.svg)](https://github.com/Al-Sarraf-Tech/ForgeISO/actions/workflows/ci-rust.yml)
[![Release](https://img.shields.io/github/v/release/Al-Sarraf-Tech/ForgeISO)](https://github.com/Al-Sarraf-Tech/ForgeISO/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

**Current version: v0.2.2**

ForgeISO builds unattended installation media for Ubuntu, Fedora-family distros,
Linux Mint, Arch Linux, Debian, openSUSE, Kali Linux, and Pop!_OS. It also
verifies, inspects, diffs, scans, tests, and reports on ISOs from the same
toolset.

No cloud service. No agent. No remote builder. Just your Linux host and the ISO
you want to customize.

---

## Architecture

ForgeISO is a Rust workspace with four crates:

| Crate | Binary | Role |
|---|---|---|
| `forgeiso-engine` | — | Core library: ISO I/O, config, distro generators, scanner, orchestrator, VM helpers |
| `forgeiso-cli` | `forgeiso` | Automation-first CLI; stable flags, JSON output, CI/scripting |
| `forgeiso-tui` | `forgeiso-tui` | Terminal wizard built on ratatui + crossterm |
| `forge-slint` | `forge-slint` | Native desktop GUI built on Slint (winit + femtovg) |

### ISO build pipeline

```
Source ISO (local path or URL)
  │
  ▼
1. Inspect      — read volume ID, detect distro/release/arch, SHA-256
2. Extract      — xorriso extracts filesystem tree into a workspace
3. Generate     — engine writes distro-specific unattended config:
                    Ubuntu  → cloud-init autoinstall YAML
                    Fedora/RHEL/Rocky/Alma/CentOS → Kickstart
                    Mint    → Calamares preseed
                    Arch    → archinstall JSON
4. Overlay      — optional file overlay injected into the root filesystem
5. Repack       — xorriso + mtools + squashfs-tools rebuilds bootable ISO
6. Report       — SHA-256, SBOM metadata, and build summary written to artifacts/
```

`forgeiso-desktop` is a shell dispatcher that launches `forge-slint` (GUI) when
a graphical session is available, then falls back to `forgeiso-tui`, then to
`forgeiso`.

---

## Interfaces

| Interface | Binary | Best for |
|---|---|---|
| Desktop wizard | `forgeiso-desktop` | Graphical guided build |
| Terminal wizard | `forgeiso-tui` | Guided use on a terminal or over SSH |
| Advanced CLI | `forgeiso` | Scripting, CI, repeatable automation, power users |

---

## Supported Distros

| Distro family | Installer format | Presets | Status |
|---|---|---|---|
| Ubuntu (24.04 LTS, 22.04, 20.04, 18.04, 25.10) | cloud-init autoinstall | `ubuntu-server-lts`, `ubuntu-desktop-lts`, `ubuntu-server-jammy`, … | Fully supported |
| Fedora 42 (Server, Workstation, KDE) | Kickstart | `fedora-server`, `fedora-workstation`, `fedora-kde` | Fully supported |
| Rocky Linux, AlmaLinux, CentOS Stream, RHEL | Kickstart | `rocky-linux`, `almalinux`, `centos-stream`, `rhel-custom` | Fully supported |
| Linux Mint 22.3 (Cinnamon, MATE, Xfce) | Calamares preseed | `linux-mint-cinnamon`, `linux-mint-mate`, `linux-mint-xfce` | Supported, best-effort |
| Arch Linux, EndeavourOS, Garuda, Manjaro | archinstall JSON | `arch-linux`, `endeavouros`, `garuda-dr460nized`, … | Supported, best-effort |
| Debian (netinst) | Preseed | `debian-netinst` | Supported, best-effort |
| openSUSE Leap, Tumbleweed | AutoYaST | `opensuse-leap`, `opensuse-tumbleweed` | Supported, best-effort |
| Kali Linux | Preseed | `kali-linux`, `kali-linux-netinst` | Supported, best-effort |
| Pop!_OS 22/24 (Intel, NVIDIA) | Unattended | `pop-os-22-intel`, `pop-os-22-nvidia`, `pop-os-24-intel` | Supported, best-effort |

Ubuntu and Fedora-family unattended installs are the most battle-tested paths.

Run `forgeiso sources list` to see all presets and their download strategies.

---

## Install

Download the latest release from the
[Releases page](https://github.com/Al-Sarraf-Tech/ForgeISO/releases/latest).

### Fedora, Rocky, AlmaLinux, CentOS Stream

```bash
sudo rpm -ivh forgeiso-0.2.2-1.x86_64.rpm
```

### openSUSE

```bash
sudo zypper install ./forgeiso-0.2.2-1.x86_64.rpm
```

### Debian, Ubuntu, Linux Mint

```bash
sudo dpkg -i forgeiso_0.2.2-1_amd64.deb
sudo apt-get install -f
```

### Arch Linux

```bash
sudo pacman -U forgeiso-0.2.2-1-x86_64.pkg.tar.zst
```

### Any x86-64 Linux (tarball)

```bash
tar -xzf forgeiso-0.2.2-linux-x86_64.tar.gz
sudo install -m755 forgeiso-0.2.2-linux-x86_64/bin/forgeiso /usr/local/bin/
sudo install -m755 forgeiso-0.2.2-linux-x86_64/bin/forgeiso-tui /usr/local/bin/
sudo install -m755 forgeiso-0.2.2-linux-x86_64/bin/forgeiso-desktop /usr/local/bin/
sudo install -m755 forgeiso-0.2.2-linux-x86_64/bin/forge-slint /usr/local/bin/
```

**Required system tools:** `xorriso`, `squashfs-tools`, `mtools`

**Optional desktop extras:** `zenity` or `kdialog` for file picking;
`wl-clipboard` or `xclip`/`xsel` for clipboard; `xdg-utils` for "Open Folder".

> If you switch from a tarball install to an RPM/DEB/pacman package, remove the
> stale `/usr/local/bin/forgeiso*` and `/usr/local/bin/forge-slint` binaries
> first — `/usr/local/bin` shadows `/usr/bin` and will hide your packaged upgrade.

---

## Quick Start

### 1. Check your host

```bash
forgeiso doctor
```

### 2. Build your first ISO

Minimal Ubuntu server with a preset:

```bash
forgeiso inject \
  --preset ubuntu-server-lts \
  --out /tmp/out \
  --hostname bastion \
  --username admin \
  --password secret
```

Boot the output ISO — the installer runs without interactive prompts.

### 3. Or use the guided wizards

Desktop:

```bash
forgeiso-desktop
```

Terminal / SSH:

```bash
forgeiso-tui
```

Both wizards follow the same flow: Choose ISO → Configure → Build → Optional Checks.

---

## CLI Reference

### `forgeiso inject` — inject unattended install config into an ISO

Core flags:

| Flag | Description |
|---|---|
| `--preset <NAME>` | Use a built-in source preset (see `forgeiso sources list`) |
| `--source <PATH\|URL>` | Local path or HTTPS URL to the source ISO |
| `--distro <ubuntu\|fedora\|mint\|arch>` | Force installer format |
| `--out <DIR>` | Output directory for the built ISO |
| `--name <FILE>` | Output filename (default: auto-generated) |
| `--hostname <NAME>` | Target machine hostname |
| `--username <NAME>` | Initial user account name |
| `--password <PASS>` | Initial user password |
| `--password-file <PATH>` | Read password from file |
| `--password-stdin` | Read password from stdin |
| `--ssh-key <KEY>` | Authorized SSH public key (repeatable) |
| `--ssh-key-file <PATH>` | Authorized SSH public key file (repeatable) |
| `--package <PKG>` | Extra package to install (repeatable) |
| `--group <GROUP>` | Add user to group (repeatable) |
| `--sudo-nopasswd` | Grant passwordless sudo |
| `--firewall` | Enable firewall |
| `--allow-port <PORT>` | Open firewall port (repeatable) |
| `--docker` | Install and configure Docker |
| `--podman` | Install Podman |
| `--encrypt` | Enable full-disk encryption |
| `--timezone <TZ>` | System timezone |
| `--locale <LOCALE>` | System locale |
| `--late-command <CMD>` | Run command after install, in chroot (repeatable) |
| `--no-user-interaction` | Suppress all install prompts |
| `--expected-sha256 <HEX>` | Verify source ISO hash before building |
| `--json` | Machine-readable JSON output |

Full inject flag reference:

```bash
forgeiso inject --help
```

### `forgeiso build` — build a customized ISO (overlay-only, no autoinstall)

```bash
forgeiso build \
  --source ubuntu-24.04-server-amd64.iso \
  --out /tmp/out \
  --overlay ./my-overlay-dir \
  --profile minimal
```

Flags: `--source`, `--preset`, `--project`, `--out`, `--name`, `--overlay`,
`--volume-label`, `--profile minimal|desktop`, `--expected-sha256`, `--json`.

### `forgeiso verify` — verify ISO against upstream checksums

```bash
forgeiso verify --source ubuntu-24.04-server-amd64.iso
forgeiso verify --source https://example.com/my.iso --sums-url https://example.com/SHA256SUMS
```

### `forgeiso inspect` — read ISO metadata

```bash
forgeiso inspect --source ubuntu-24.04-server-amd64.iso
forgeiso inspect --source ubuntu-24.04-server-amd64.iso --json
```

### `forgeiso diff` — compare two ISOs file-by-file

```bash
forgeiso diff --base original.iso --target custom.iso
```

### `forgeiso scan` — security scan a built ISO artifact

Runs trivy, syft, grype, and oscap where available.

```bash
forgeiso scan --artifact custom.iso
forgeiso scan --artifact custom.iso --policy ./policy.yaml --json
```

### `forgeiso test` — boot-test an ISO in QEMU

```bash
forgeiso test --iso custom.iso --bios
forgeiso test --iso custom.iso --uefi
forgeiso test --iso custom.iso --bios --uefi
```

### `forgeiso report` — generate a build report

```bash
forgeiso report --build ./artifacts --format html
forgeiso report --build ./artifacts --format json
```

### `forgeiso sources` — manage built-in presets

```bash
forgeiso sources list
forgeiso sources list --json
forgeiso sources show ubuntu-server-lts
forgeiso sources resolve fedora-server
```

### `forgeiso vm emit` — generate VM launch commands

```bash
forgeiso vm emit --iso custom.iso --hypervisor qemu --firmware uefi
forgeiso vm emit --iso custom.iso --hypervisor virtualbox --ram 4096 --cpus 4
```

Supported hypervisors: `qemu`, `virtualbox`, `vmware`, `hyperv`, `proxmox`.

### `forgeiso doctor` — check host tooling

```bash
forgeiso doctor
forgeiso doctor --json
```

---

## Build From Source

Requires Rust 1.87+ and system tools: `xorriso`, `squashfs-tools`, `mtools`.

```bash
git clone https://github.com/Al-Sarraf-Tech/ForgeISO
cd ForgeISO
cargo build --release
```

Install binaries:

```bash
sudo install -m755 target/release/forgeiso /usr/local/bin/
sudo install -m755 target/release/forgeiso-tui /usr/local/bin/
sudo install -m755 target/release/forge-slint /usr/local/bin/
sudo install -m755 scripts/release/forgeiso-desktop /usr/local/bin/
```

Common make targets:

| Target | Action |
|---|---|
| `make build` | Release build for all workspace crates |
| `make test` | Run all workspace tests |
| `make lint` | `cargo fmt --check` + `cargo clippy -D warnings` |
| `make ci-base` | Build local CI warm-cache Docker image |
| `make ci-local` | Run all CI stages in parallel ephemeral containers |
| `make package` | Build release tarball |
| `make clean` | `cargo clean` |

---

## Testing

```bash
# All tests
cargo test --workspace

# Engine unit tests only
cargo test -p forgeiso-engine

# With output
cargo test --workspace -- --nocapture
```

Tests are in `engine/src/orchestrator/` (unit) and `engine/tests/` (integration,
distro regression, e2e regression, workspace).

---

## Notes

### Intel integrated graphics fallback

If `forge-slint` has rendering trouble on Intel integrated graphics:

```bash
MESA_GL_VERSION_OVERRIDE=3.3 forge-slint
```

### VM name sanitization

VM names derived from hostnames or build configs are sanitized automatically.
Characters invalid for QEMU, VirtualBox, VMware, or Proxmox are stripped or
replaced before any hypervisor command is issued.

### Download verification

```bash
sha256sum -c SHA256SUMS
```

ForgeISO checks source ISO hashes; it does not use output ISO hashes as a trust
source.

---

## Documentation

- [GUI runbook](docs/runbook-gui.md)
- [Local build and development runbook](docs/runbook-local.md)
- [Release runbook](docs/runbook-release.md)
- [Distro support matrix](docs/distro-support.md)
- [VM testing](docs/vm-testing.md)
- [Security notes](docs/security.md)
- [Troubleshooting](docs/troubleshooting.md)

---

## CI/CD

CI runs on self-hosted runners (`linux-mega-1`, `wsl2-runner`, `dominus-runner`)
via `.github/workflows/ci-rust.yml`, governed by the
[Haskell Orchestrator](https://github.com/Al-Sarraf-Tech/Haskell-Orchestrator).

Pipeline stages: **repo-guard → lint → test → security → sbom → integration → release**.

Security stage: gitleaks, cargo audit, cargo deny, ShellCheck, shfmt, Trivy,
Syft SBOM (SPDX + CycloneDX).

Release is triggered by version tags (`v*.*.*`) and requires test + security to pass.

---

## License

[Apache-2.0](LICENSE)
