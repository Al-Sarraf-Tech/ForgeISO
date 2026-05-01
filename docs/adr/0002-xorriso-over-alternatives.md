# ADR 0002: xorriso as the canonical ISO repack tool

- **Status**: Accepted
- **Date**: 2026-03-15

## Context

To inject autoinstall configs and overlay files into an existing
distribution ISO and produce a bootable output ISO, ForgeISO needs a
tool that can:

1. Read files from inside an ISO-9660 image (extract `/.disk/info`,
   `/.treeinfo`, `/arch/version` for distro detection).
2. Inspect El Torito boot records to know if BIOS, UEFI, or both are
   bootable.
3. Modify ISO contents (drop in `autoinstall/`, `ks.cfg`, `preseed.cfg`,
   patched `grub.cfg`, `isolinux.cfg`).
4. Emit a hybrid ISO9660 + ISOLINUX/grub2 + EFI image that boots cleanly
   on both BIOS and UEFI hardware **and** as a USB stick after `dd`.
5. Be present and stable on Fedora, Debian, Ubuntu, Arch — everywhere
   a ForgeISO operator might run the tool.

## Decision

Standardize on **xorriso** (libisoburn) for all ISO read and write
operations. Probed at runtime via `which::which("xorriso")`; `inspect`
degrades gracefully when absent (reads only the primary volume label),
but `inject` and `build` hard-fail with a clear error code (E030) and
runbook entry.

The xorriso CLI is invoked via `crate::orchestrator::run_command_capture`
and `run_command_lossy` (the latter accepts non-zero exit because
xorriso exits non-zero on ISOs without El Torito records, which is not
an error for inspect).

## Alternatives considered

- **genisoimage / mkisofs**: legacy tools, no longer maintained on most
  distros, no support for hybrid ISO + ISOHYBRID + UEFI in a single
  invocation. Would force ForgeISO to chain `genisoimage` →
  `isohybrid` → `mkimage` for UEFI, which is fragile and version-
  sensitive.
- **libisofs (Rust bindings)**: would let us avoid shelling out, but
  the Rust binding (`isoflate`/`isofuse-rs`) coverage is incomplete for
  El Torito and EFI System Partition handling, and would lock us into
  a smaller userbase that hits unique bugs.
- **mtools + dd**: works for FAT32 EFI partitions but not for the ISO
  9660 outer container; would still need a separate tool for that
  layer.
- **isomaker / isoinfo**: read-only, useless for write paths.
- **Bundling xorriso as a static binary with ForgeISO**: would avoid
  the host-tooling probe but inflates the release artifact and requires
  per-arch builds. xorriso is in every supported distro's package
  index; runtime probing is cheaper.

## Consequences

- **Positive**: xorriso is the de-facto standard for hybrid ISO authoring;
  community knowledge is deep, edge cases are well-documented.
- **Positive**: One tool covers read + write + boot-record manipulation;
  no chaining required.
- **Positive**: xorriso's `-as mkisofs` mode lets us reuse mkisofs flag
  syntax in places where that's clearer (boot catalog placement).
- **Negative**: External-tool dependency. Operators on minimal images
  must `dnf/apt install xorriso` before injecting. Documented in
  `docs/RUNBOOKS.md` E010/E030.
- **Negative**: Some xorriso commands write to stderr even on success;
  we use `run_command_lossy` to capture both streams when exit-code-
  agnostic parsing is required.
- **Negative**: Subprocess overhead per ISO operation (50-200ms per
  invocation). Negligible for the use case (build cycles are minutes,
  not seconds).
