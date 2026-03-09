# ForgeISO Architecture

## Current model

ForgeISO is a Linux-only, bare-metal ISO remastering tool.

All product workflows run locally through the Rust engine:
- CLI for automation
- TUI for terminal operators
- GUI for desktop users

There is no product-side server process, remote agent, or container runtime dependency.

## Engine responsibilities

- Resolve an ISO source from a local path or user-provided URL
- Inspect the ISO and detect distro metadata from the image itself
- Validate local host prerequisites
- Extract and repack supported Linux ISO layouts with local tools
- Apply local overlay content into the ISO or unpacked rootfs
- Run local scan, test, and report steps
- Emit structured events for UI logging

## CI/CD boundary

CI may still use ephemeral containers for repeatable pipeline stages. Those containers are not part of the shipped product workflow.

## Distro injection model

ForgeISO supports four distro injection paths:

| Distro | Mechanism | Maturity |
|---|---|---|
| Ubuntu | cloud-init nocloud datasource | Production-ready, CI-tested |
| Fedora | Kickstart ks.cfg | Production-ready, CI-tested |
| Linux Mint | preseed.cfg via Calamares | Experimental, not CI-tested |
| Arch Linux | archinstall JSON + archiso_script= | Experimental, not CI-tested |

The distro dispatch is inside `ForgeIsoEngine::inject_autoinstall()` in `engine/src/orchestrator.rs`. Each branch generates the appropriate config file and patches the ISO boot entries.

See [distro-support.md](distro-support.md) for the full per-distro capability matrix.
