# ForgeISO

> Turn a stock Linux ISO into a hands-free installer from your own Linux machine.

[![CI](https://github.com/Al-Sarraf-Tech/ForgeISO/actions/workflows/ci.yml/badge.svg)](https://github.com/Al-Sarraf-Tech/ForgeISO/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Al-Sarraf-Tech/ForgeISO)](https://github.com/Al-Sarraf-Tech/ForgeISO/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

**Current version: v0.2.1**

ForgeISO builds unattended installation media for Ubuntu, Fedora-family distros,
Linux Mint, and Arch Linux. It can also verify, inspect, diff, scan, test, and
report on ISOs from the same toolset.

No cloud service. No agent. No remote builder. Just your Linux host and the ISO
you want to customize.

## Why People Use It

- Build a fully unattended server or workstation installer.
- Add a user, SSH keys, packages, firewall rules, services, and post-install commands.
- Reuse the same product through a desktop GUI, terminal UI, or automation-first CLI.

## Pick Your Interface

| Interface | Binary | Best for |
|---|---|---|
| Desktop wizard | `forgeiso-desktop` | Most people trying ForgeISO for the first time |
| Terminal wizard | `forgeiso-tui` | Guided use on a terminal or remote shell |
| Advanced CLI | `forgeiso` | Scripting, CI, repeatable automation, power users |

`forgeiso-desktop` prefers the Slint GUI (`forge-slint`), then falls
back to the TUI or CLI depending on what your system supports.

## Start Here

### 1. Install ForgeISO

Download the latest release from the
[Releases page](https://github.com/Al-Sarraf-Tech/ForgeISO/releases/latest).

#### Fedora, Rocky, AlmaLinux, CentOS Stream

```bash
sudo rpm -ivh forgeiso-0.2.1-1.x86_64.rpm
```

#### openSUSE

```bash
sudo zypper install ./forgeiso-0.2.1-1.x86_64.rpm
```

#### Debian, Ubuntu, Linux Mint

```bash
sudo dpkg -i forgeiso_0.2.1-1_amd64.deb
sudo apt-get install -f
```

#### Arch Linux

```bash
sudo pacman -U forgeiso-0.2.1-1-x86_64.pkg.tar.zst
```

#### Any x86-64 Linux (tarball)

```bash
tar -xzf forgeiso-0.2.1-linux-x86_64.tar.gz
sudo install -m755 forgeiso-0.2.1-linux-x86_64/bin/forgeiso /usr/local/bin/
sudo install -m755 forgeiso-0.2.1-linux-x86_64/bin/forgeiso-tui /usr/local/bin/
sudo install -m755 forgeiso-0.2.1-linux-x86_64/bin/forgeiso-desktop /usr/local/bin/
sudo install -m755 forgeiso-0.2.1-linux-x86_64/bin/forge-slint /usr/local/bin/
```

> Required system tools: `xorriso`, `squashfs-tools`, and `mtools`
>
> Helpful desktop extras: `zenity` or `kdialog` for file picking,
> `wl-clipboard` or `xclip`/`xsel` for copy, and `xdg-utils` for "Open Folder"

If you switch from a tarball install in `/usr/local/bin` to an RPM, DEB, or
pacman-style package, remove the old `/usr/local/bin/forgeiso*`,
`/usr/local/bin/forge-slint` binaries first.
`/usr/local/bin` shadows `/usr/bin`, so stale tarball binaries can hide your
packaged upgrade.

### 2. Check Your Host

```bash
forgeiso doctor
```

### 3. Launch the Guided Flow

```bash
forgeiso-desktop
```

The guided flow is:

1. Choose ISO
2. Configure
3. Build
4. Optional Checks

Build is the completion point. Optional Checks are extra assurance, not a
required step.

If you are on a headless machine or over SSH, start with:

```bash
forgeiso-tui
```

## Your First ISO

### Easiest path: built-in preset

```bash
forgeiso inject \
  --preset ubuntu-server-lts \
  --out /tmp/out \
  --hostname bastion \
  --username admin \
  --password secret
```

Boot the output ISO and the installer will run without interactive prompts.

### Desktop users

- Start `forgeiso-desktop`
- Pick a preset or browse to an ISO
- Fill in hostname, user, password, and optional packages
- Click Build

### Terminal users

```bash
forgeiso-tui
```

The TUI uses the same guided flow and terminology as the desktop wizard.

### Automation users

```bash
forgeiso inject \
  --source ubuntu-24.04-server-amd64.iso \
  --out /tmp/out \
  --hostname bastion \
  --username admin \
  --password secret \
  --group sudo \
  --firewall \
  --allow-port 22 \
  --docker \
  --no-user-interaction
```

## Supported Distros

| Distro family | Installer format | Status |
|---|---|---|
| Ubuntu | cloud-init autoinstall | Fully supported |
| Fedora | Kickstart | Fully supported |
| Rocky, AlmaLinux, CentOS Stream | Kickstart | Fully supported through Fedora-family path |
| Linux Mint | Calamares preseed | Supported, best-effort |
| Arch Linux | archinstall JSON | Supported, best-effort |

Ubuntu and Fedora-family unattended installs are the most battle-tested paths.

## Useful Commands

```bash
forgeiso verify --source ubuntu-24.04-server-amd64.iso
forgeiso inspect --source ubuntu-24.04-server-amd64.iso
forgeiso diff --base original.iso --target custom.iso
forgeiso scan --artifact custom.iso
forgeiso test --iso custom.iso --bios --uefi
forgeiso report --build ./artifacts --format html
forgeiso sources list
forgeiso vm emit --iso custom.iso --hypervisor qemu --firmware uefi
```

For the full CLI surface:

```bash
forgeiso --help
forgeiso inject --help
forgeiso sources --help
```

## Common Notes

### Graphical session requirements

`forgeiso-desktop` works best on a normal Linux desktop session. On minimal or
headless systems it falls back to `forgeiso-tui` or `forgeiso`.

If the primary GUI has rendering trouble on Intel integrated graphics:

```bash
MESA_GL_VERSION_OVERRIDE=3.3 forgeiso-desktop
```

### VM name sanitization

VM names derived from hostnames or build configs are sanitized before being
passed to any hypervisor command. Characters that are invalid for QEMU,
VirtualBox, VMware, or Proxmox VM names are stripped or replaced automatically.

### Download verification

```bash
sha256sum -c checksums.txt
```

This verifies the downloaded source ISO or release assets. ForgeISO itself only
checks source ISO hashes, never the output ISO hash as a trust source.

## Learn More

- [GUI runbook](docs/runbook-gui.md)
- [Local build and development runbook](docs/runbook-local.md)
- [Release runbook](docs/runbook-release.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Security notes](docs/security.md)
- [Distro support matrix](docs/distro-support.md)

## Build From Source

Requires Rust 1.87+ plus `xorriso`, `squashfs-tools`, and `mtools`.

```bash
git clone https://github.com/Al-Sarraf-Tech/ForgeISO
cd ForgeISO
cargo build --release
```

Install the main binaries:

```bash
sudo install -m755 target/release/forgeiso /usr/local/bin/
sudo install -m755 target/release/forgeiso-tui /usr/local/bin/
sudo install -m755 scripts/release/forgeiso-desktop /usr/local/bin/
sudo install -m755 target/release/forge-slint /usr/local/bin/
```

## Contributors

See [AGENTS.md](AGENTS.md) for project architecture, build rules, and agent
conventions.

## CI/CD & Orchestration

This project is governed by the [Haskell Orchestrator](https://github.com/Al-Sarraf-Tech/Haskell-Orchestrator) — a Haskell-based multi-agent CI/CD governance framework for pre-push validation, code quality enforcement, and release management across the Al-Sarraf-Tech organization.

## License

[Apache-2.0](LICENSE)
