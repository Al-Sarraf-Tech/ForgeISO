# Runbook: Local Bare-Metal Build

## Interface Choice

- `forgeiso-desktop`: guided desktop workflow for normal users
- `forgeiso-tui`: guided terminal workflow with the same 4-step model
- `forgeiso`: advanced CLI for scripting, CI, and explicit control

## Prerequisites

- Linux host
- Rust stable toolchain
- Node 22+ for the GUI build
- Local tools as needed: `xorriso`, `unsquashfs`, `mksquashfs`, `qemu-system-x86_64`, `oscap`, `trivy`, `syft`, `grype`

## Workflow

The CLI is the advanced surface. Guided users should prefer `forgeiso-desktop`
or `forgeiso-tui`, then follow the same conceptual flow:
Choose ISO → Configure → Build → Optional Checks.

1. Check the host:

```bash
cargo run -p forgeiso-cli -- doctor
```

2. Inspect a source ISO or URL:

```bash
cargo run -p forgeiso-cli -- inspect --source /path/to/base.iso
```

3. Build locally:

```bash
cargo run -p forgeiso-cli -- build --source /path/to/base.iso --out ./artifacts --name demo-build
```

4. Optional local scan:

```bash
cargo run -p forgeiso-cli -- scan --artifact ./artifacts/demo-build.iso
```

5. Optional local smoke test:

```bash
cargo run -p forgeiso-cli -- test --iso ./artifacts/demo-build.iso --bios --uefi
```
