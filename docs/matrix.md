# ForgeISO Distro Validation Matrix

The matrix system validates ForgeISO across multiple Linux distributions, versions, and build profiles using ephemeral Docker containers. Each cell (distro × version × profile) runs in full isolation with its own Cargo target volume.

## Quick start

```bash
# Smoke pass — all cells, fast (no QEMU)
make matrix

# Full pass — smoke + QEMU boot test if KVM available
make matrix-full

# Single cell
make matrix MATRIX_CELL=ubuntu-2404-minimal

# Clean all volumes and cached images
make matrix-clean
```

## Matrix cells

| Cell name | Distro | Version | Profile |
|---|---|---|---|
| `ubuntu-2404-minimal` | Ubuntu | 24.04 LTS | minimal |
| `ubuntu-2404-desktop` | Ubuntu | 24.04 LTS | desktop |
| `ubuntu-2204-minimal` | Ubuntu | 22.04 LTS | minimal |
| `fedora-40-minimal` | Fedora | 40 | minimal |
| `arch-rolling-minimal` | Arch Linux | rolling | minimal |
| `mint-22-minimal` | Linux Mint | 22 | minimal |

## Tiers

### `smoke` (default)
1. Build `forgeiso-cli` in release mode
2. Run `doctor` — reports tool availability in the container
3. Create a synthetic grub-based ISO with `scripts/test/make-smoke-iso.sh`
4. Run `build` with the chosen profile and overlay
5. Validate ISO-9660 header (`CD001` at sector 16) in the output artifact

### `full`
Everything in `smoke`, plus:
6. If `qemu-system-x86_64` and `/dev/kvm` are available: BIOS + UEFI boot test via `forgeiso test`

Full tier requires KVM on the host. Without KVM the boot test step is skipped with a warning rather than failing.

## Artifacts

Each cell writes results to `artifacts/matrix/<cell-name>/`:

| File | Contents |
|---|---|
| `result.txt` | `PASS` or absent (failure) |
| `doctor.json` | Raw doctor output |
| `inspect.json` | ISO inspect result |
| `build.json` | Build result |
| `boot-test.json` | QEMU boot test result (full tier only) |
| `smoke/` | Synthetic ISO tree and built artifacts |

## Implementation

| File | Purpose |
|---|---|
| `docker-compose.matrix.yml` | One service per matrix cell; all use isolated named volumes |
| `containers/matrix.Dockerfile` | Single shared image built with `DISTRO_LABEL`/`VERSION_LABEL`/`PROFILE_LABEL` build args |
| `scripts/matrix/run-cell.sh` | Per-cell logic executed inside the container |
| `scripts/matrix/run-matrix.sh` | Host-side orchestrator: build images → run cells → collect results → clean volumes |

## Adding a new cell

1. Add a service entry to `docker-compose.matrix.yml` following the existing pattern. Use isolated volume names derived from the cell name.
2. Add the two volumes (`<cell>-target`, `<cell>-cargo`) to the `volumes:` block at the bottom of the file.
3. Update the cell table in this document.

No changes to `run-cell.sh` or `run-matrix.sh` are needed — they read distro/version/profile from environment variables.

## ISO-9660 compliance validation

`run-cell.sh` validates the ISO-9660 primary volume descriptor directly after each build by reading 5 bytes at offset `(16 × 2048) + 1` in the output ISO and comparing them to the ASCII string `CD001`. This check runs in every tier without requiring `xorriso`.

The GUI's Verify stage provides a richer ISO-9660 compliance report (El Torito, boot entries, BIOS/UEFI detection) using the `validate_iso9660` engine method backed by `xorriso` when available.
