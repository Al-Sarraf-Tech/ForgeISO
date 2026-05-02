# Test-release orchestrator

`scripts/test-releases.sh` is a hermetic, end-to-end regression harness for the
ForgeISO inject + repack pipeline. It exercises every `IsoPreset` defined in
`engine/src/sources/catalog/*` against synthetic source ISOs without ever
reaching the network or downloading upstream installer media.

## Intent

The build pipeline has a wide blast radius:
`engine/src/orchestrator/inject/{configure,place}.rs` writes distro-specific
config (cloud-init, kickstart, preseed, archinstall) into a freshly-extracted
ISO tree, patches GRUB / isolinux / systemd-boot / EFI grub.cfg, and then asks
xorriso to repack the tree into a hybrid ISO9660 image while preserving the
original El Torito boot record. Any one of those steps can silently produce a
non-bootable artifact when the per-distro source layout changes.

This script catches those regressions in CI / pre-release without:
- waiting hours for upstream ISO downloads,
- consuming bandwidth or saturating mirrors,
- requiring real installer media on disk,
- needing root or a hypervisor (no actual VM boot test — that is what
  `forgeiso test --bios --uefi` is for).

It is the "does the build pipeline still produce a structurally valid ISO for
every preset we ship?" gate.

## What gets tested

For each preset in the catalog the script:

1. Generates a synthetic source ISO via `tests/fixtures/synthetic-iso.sh
   <family> <out.iso>`. Per-family layout matches what `place.rs` expects
   (e.g. Ubuntu casper + `.disk/info`, Fedora `images/pxeboot` +
   `boot/grub2/grub.cfg`, Arch `syslinux/archiso_sys.conf` + `loader/entries`).
2. Builds a fully-populated `forgeiso inject` argv with realistic test values
   for hostname, user, SSH keys, DNS, NTP, static IP, gateway, timezone,
   locale, keyboard, packages, groups, sudo, firewall, services, GRUB,
   sysctl, late-commands — not engine defaults. Per-distro repos and mirrors
   are added when the distro flag matches (apt for ubuntu, dnf for fedora,
   pacman for arch).
3. Invokes the installed `forgeiso` binary (`~/.cargo/bin/forgeiso` by
   default; override with `--binary <path>`) with a 300 s timeout per preset
   (override with `--threshold <seconds>` or env `PERF_THRESHOLD`).
4. Asserts the output ISO exists, re-hashes it via `sha256sum`, parses the
   engine's JSON output and verifies the engine's reported source SHA-256
   matches the on-disk source.
5. Re-scans the produced ISO via `forgeiso scan` (degraded to WARN if
   external tools like trivy/syft are missing — those are scan-time, not
   build-time, dependencies).
6. Records elapsed time, output size, and verdict.

The final summary is a per-preset PASS / FAIL / SKIP table, e.g.:

```
PRESET                       STATUS DETAIL                  TIME(s)      SIZE(B)
----------------------------------------------------------------------------------
ubuntu-server-lts            PASS   ok                            7       901120
fedora-server                PASS   scan-degraded                 6       780288
arch-linux                   FAIL   inject-rc-1                   2            0
opensuse-leap                SKIP   timeout-300s                300            0
----------------------------------------------------------------------------------
total=33  PASS=31  FAIL=1  SKIP=1
```

Exit status is the failure count (0 = all passed). SKIPs (timeouts) do not
fail the run.

## Modes

| Flag                          | Effect                                                 |
|-------------------------------|--------------------------------------------------------|
| `--list`                      | List every preset id and exit (no work performed).     |
| `--preset <id>`               | Run a single preset by kebab-case id.                  |
| `--parallel <N>`              | Run up to N presets concurrently (default 1).          |
| `--keep-artifacts`            | Keep the per-preset workdir under `tests/test-builds/`.|
| `--binary <path>`             | Override the forgeiso CLI path.                        |
| `--threshold <seconds>`       | Per-preset timeout (default 300 s).                    |
| `-h`, `--help`                | Show usage banner.                                     |

Per-preset workdirs land in `tests/test-builds/<preset-id>/` and are removed
on success unless `--keep-artifacts` is passed. The `tests/test-builds/`
directory is git-ignored except for its `.gitignore`.

## Adding a new preset

When a new preset lands in `engine/src/sources/catalog/<family>.rs`:

1. Pick the right synthetic family. The fixture script supports
   `ubuntu`, `debian`, `fedora`, `arch`, `opensuse`, `mint` — pick the one
   whose top-level layout matches what your preset's `place.rs` branch
   expects. Most new entries reuse an existing family.
2. If the preset needs a structurally different source ISO layout (e.g.
   a new bootloader, a new live-image marker file, an unfamiliar `.disk/`
   layout) add a new `populate_<family>()` function in
   `tests/fixtures/synthetic-iso.sh` and a new `case "$FAMILY"` arm.
3. Append the preset id to the `PRESETS` array in
   `scripts/test-releases.sh`. Each entry is a pipe-separated triple:
   `<preset-id>|<inject-distro-flag>|<synthetic-family>`. The distro flag
   is one of `ubuntu`, `fedora`, `arch`, `mint`, or empty (default Ubuntu).
4. Run `./scripts/test-releases.sh --preset <new-id>` locally to confirm
   it passes before pushing.

## Interpreting failures

| Detail                | Meaning                                                                                          |
|-----------------------|--------------------------------------------------------------------------------------------------|
| `fixture-failed`      | `tests/fixtures/synthetic-iso.sh` could not produce the source ISO. Check xorriso is installed.  |
| `inject-rc-<N>`       | `forgeiso inject` exited with status N. Inspect `tests/test-builds/<id>/run.log` for stderr.     |
| `no-output`           | inject succeeded but produced no `.iso` file. Look at the JSON in `run.log` for the artifact path.|
| `sha-mismatch`        | The engine's reported source SHA-256 disagrees with `sha256sum` of the source. Indicates a bug.  |
| `timeout-<N>s`        | Preset exceeded the per-preset timeout. Mark as SKIP, not FAIL — usually a CI-environment issue. |
| `scan-degraded` (PASS)| Inject succeeded; `forgeiso scan` could not run (missing trivy/syft/grype). Not a build problem. |

For any FAIL, the per-preset log is at `tests/test-builds/<preset-id>/run.log`
when run with `--keep-artifacts`. The log contains the fixture stdout, the
full forgeiso stderr, and the BuildResult JSON.

## What this script does NOT do

- It does not boot the produced ISO in a VM. Use `forgeiso test --bios --uefi`
  (or `scripts/vm-launch.sh`) for that — it needs a real installer payload
  inside the source ISO, which our synthetics deliberately omit.
- It does not validate the cloud-init / kickstart / preseed / archinstall
  YAML semantically. Use the unit tests under
  `engine/src/autoinstall/`, `engine/src/kickstart.rs`, etc. for that.
- It does not exercise the GUI or TUI front-ends. Use `forgeiso-build` and
  the front-end smoke tests for those.
- It does not download upstream media or check upstream URL liveness. Use
  `scripts/cache-image.sh` and the inspect/verify subcommands for that.

## Real-world soak testing

This harness is a hermetic structural test. Before a release, it should be
complemented with at least one full upstream-media build per family
(`forgeiso inject --preset ubuntu-server-lts --out /tmp/...`) followed by
`forgeiso test --iso ... --bios --uefi` to confirm the produced ISO actually
boots its installer. That step is left to the release workflow because it
needs network access and several GB of disk per family.
