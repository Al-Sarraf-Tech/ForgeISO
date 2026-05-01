# ADR 0005: Distro-specific autoinstall injection (not generic overlay)

- **Status**: Accepted
- **Date**: 2026-03-15

## Context

ForgeISO's central feature is converting an interactive distribution
ISO into an unattended-install ISO that boots, installs the OS with a
predefined config, creates a user, sets up SSH keys, opens firewall
ports, etc. — without any human at the console.

The four supported distros each have a different unattended-install
mechanism, with different file formats, different boot-time triggers,
and different acceptable storage layouts:

- **Ubuntu**: cloud-init `autoinstall.yaml` + `meta-data` + `user-data`
  in a `nocloud-net` source. Triggered by `autoinstall ds=nocloud-net;
  s=/cdrom/casper/` on the kernel command line.
- **Fedora/RHEL**: `ks.cfg` (Kickstart) at the ISO root. Triggered by
  `inst.ks=cdrom:/ks.cfg`.
- **Mint/Debian**: `preseed.cfg` (debian-installer preseed). Triggered
  by `auto=true priority=critical preseed/file=/cdrom/preseed.cfg`.
- **Arch**: archinstall JSON config + a wrapper shell script. Triggered
  by `archiso_script=/arch/boot/run-archinstall.sh` (archiso 2023+).

Each format has dozens of fields. Each boot config (grub.cfg, isolinux,
EFI/BOOT/grub.cfg, syslinux, systemd-boot) needs its own kernel
parameter patch. A "drop a file in /tmp/overlay" approach cannot
correctly produce an ISO that the installer will recognise.

## Decision

Generate **distro-specific autoinstall artifacts** server-side from a
unified `InjectConfig` struct, then patch each ISO's boot
configuration to trigger the corresponding installer.

- `engine/src/autoinstall/ubuntu.rs::generate_autoinstall_yaml` —
  emits cloud-init YAML for Ubuntu/Debian-derived ISOs.
- `engine/src/kickstart.rs::generate_kickstart_cfg` — emits Kickstart
  for Fedora/RHEL.
- `engine/src/mint_preseed.rs::generate_mint_preseed` — emits preseed
  for Mint/Debian-installer-based ISOs.
- `engine/src/orchestrator/inject.rs` — for Arch, emits an archinstall
  JSON + a runner script (no equivalent of preseed exists upstream).

A single `InjectConfig` (built via `InjectConfigBuilder` or by direct
struct init) feeds all four generators. Validation in
`engine/src/config/validation.rs` runs before any IO so syntactic
errors in user input fail fast with E060.

Boot configs are patched in place after the autoinstall file is dropped
into the ISO root: `EFI/BOOT/grub.cfg`, `boot/grub/grub.cfg`,
`isolinux/isolinux.cfg`, `loader/entries/*.conf` are each rewritten
to add the distro-appropriate kernel parameter. ForgeISO does not
assume any single boot loader is present; it patches whichever it finds.

## Alternatives considered

- **Generic file overlay**: drop user files into `/extras/` and trust
  the user to put the right autoinstall content there. Rejected
  because it shifts the burden of knowing each distro's format,
  filename, location, and kernel-param trigger onto the user — that
  is exactly what ForgeISO exists to abstract.
- **Wrap a single distro (e.g., Ubuntu only)**: simpler to maintain
  but leaves Fedora/Mint/Arch users to their own scripts. Doesn't
  match the project's stated scope.
- **Generate a separate config file format and use a runtime
  translator**: adds an indirection layer with its own version skew
  problem (our IR vs upstream installer changes). Rejected; one less
  layer.
- **Use an external tool (e.g., autoinstall-generator)**: would offload
  YAML emission, but the available tools cover Ubuntu only; Kickstart
  and preseed have no equivalent. Inconsistency would be worse than
  hand-rolling.

## Consequences

- **Positive**: Each distro gets unattended-install support that
  matches its native installer's expectations. The output ISO boots
  and completes installation without human input.
- **Positive**: A single `InjectConfig` shape across the CLI flag
  surface, the TUI form, and the GUI wizard means users learn the
  configuration model once.
- **Positive**: Adding a new distro is a known-shape task: write a
  generator, wire its trigger into `inject.rs`, add fixtures to
  `engine/tests/distro_regression.rs`.
- **Negative**: Maintaining four parallel generators means upstream
  installer changes (e.g., Ubuntu autoinstall schema bumps) must be
  tracked per-distro. Mitigation: regression tests pin the generated
  output for known configs; CI catches unintended changes.
- **Negative**: The `InjectConfig` struct has fields that don't apply
  to every distro (e.g., `apt_mirror` is irrelevant for Fedora,
  `dnf_repo` for Mint). Generators ignore irrelevant fields silently;
  validation makes obvious cross-distro mistakes (e.g., `--storage-layout
  zfs` on a non-Ubuntu target) explicit at the boundary.
- **Negative**: Boot-loader patching is fragile — third-party
  remasters with custom grub.cfg layouts may not match the patterns
  in `inject.rs`. Documented in `docs/troubleshooting.md` per distro.
