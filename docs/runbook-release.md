# ForgeISO Release Runbook

This runbook documents the complete process for cutting a versioned ForgeISO release,
from version bump through published GitHub Release with signed packages.

---

## Release Checklist

```
[ ] 1. Bump version in all locations
[ ] 2. Update PKGBUILD sha256sum placeholder
[ ] 3. Commit and push feature branch
[ ] 4. Open PR — wait for all 7 CI stages to pass
[ ] 5. Merge PR via squash
[ ] 6. Create and push annotated git tag
[ ] 7. GitHub Actions release job fires — wait for completion
[ ] 8. Verify all release assets and checksums
[ ] 9. Smoke-install from the published RPM
```

---

## 1. Bump Version

Release versions are split across the Rust workspace and the legacy Tauri GUI:

```
Cargo.toml                  [workspace.package] version = "X.Y.Z"
gui/package.json            version = "X.Y.Z"
gui/src-tauri/Cargo.toml    version = "X.Y.Z"
gui/src-tauri/tauri.conf.json version = "X.Y.Z"
```

Use the bump-version script for the Rust workspace version:

```bash
bash scripts/release/bump-version.sh 0.2.1
```

The script will:
1. Update `Cargo.toml` workspace version
2. Update `packaging/PKGBUILD` when that file is present
3. Regenerate `Cargo.lock`
4. Print a summary of changed files

Update the legacy Tauri GUI version files separately when that frontend is part of the release.

Verify binary version after build:

```bash
./target/release/forgeiso --version
# forgeiso 0.2.1
```

---

## 2. CI Stages

| Stage | Label | What fails | Artifact |
|---|---|---|---|
| C1 | Rust | fmt / clippy / tests | — |
| C2 | SBOM + Audit | license violations, HIGH/CRITICAL CVEs | `sbom.cdx.json`, `sbom.spdx.json`, `audit.json` |
| C3 | GUI | GUI build failures | — |
| C4 | Security | SBOM generation (best-effort) | trivy, grype, gitleaks reports |
| C5 | Integration | Integration test failures | — |
| C6 | E2E Smoke | Boot smoke test failures | — |
| C7 | Lint | fmt / clippy regressions (fast fail) | — |

C2 is the enforcement gate. The advisory database is fetched live in CI.
Run locally with:

```bash
# Full C2 stage in Docker
docker build -t forgeiso-c2 -f containers/C2.sbom.Dockerfile . \
  && docker run --rm -e CI=true -v "$PWD:/workspace" forgeiso-c2 \
     bash -c "scripts/ci/c2-sbom.sh"

# Just cargo-deny
cargo deny check

# Just cargo-audit
cargo audit
```

---

## 3. Local Packaging

Build all packages locally before tagging:

```bash
# Build release binaries first
cargo build --release -p forgeiso-cli -p forgeiso-tui -p forge-slint

# Build RPM + DEB + pacman + tarball + checksums
bash scripts/release/make-packages.sh 0.2.1

# Verify
cd dist/release
sha256sum -c checksums.txt
ls -lh
```

Outputs in `dist/release/`:

| File | Format | Distro |
|---|---|---|
| `forgeiso-0.2.1-1.x86_64.rpm` | RPM | Fedora / RHEL / openSUSE |
| `forgeiso_0.2.1-1_amd64.deb` | DEB | Debian / Ubuntu / Mint |
| `forgeiso-0.2.1-1-x86_64.pkg.tar.zst` | pacman | Arch Linux |
| `forgeiso-0.2.1-linux-x86_64.tar.gz` | tarball | Any x86-64 Linux |
| `checksums.txt` | SHA-256 | — |

---

## 4. Tagging and Publishing

After the PR is merged and local packages verify cleanly:

```bash
# Sync local main
git fetch origin main && git reset --hard origin/main

# Create annotated tag
git tag -a v0.2.1 -m "Release v0.2.1"

# Push tag — this triggers the GitHub Actions release job
git push origin v0.2.1
```

The release pipeline at [`.github/workflows/release-build.yml`](../.github/workflows/release-build.yml)
fires on the tag push and runs against a `[self-hosted, rust-slim]`
runner. The job:

1. Verifies the runner has cargo / rustc / xorriso / mksquashfs /
   unsquashfs / fpm / syft / cosign / sha256sum on `$PATH` (fails fast
   with a clear error if any are missing).
2. Builds CLI, TUI, and `forge-slint` from the tagged commit
   (`cargo build --release --locked`).
3. Confirms the built binary's `--version` matches the tag.
4. Runs [`scripts/release/make-packages.sh`](../scripts/release/make-packages.sh)
   (RPM + DEB + pacman + tarball + checksums).
5. Generates `sbom.cdx.json` (CycloneDX) and `sbom.spdx.json` (SPDX) via
   `syft`.
6. Signs every release asset and the SBOMs via
   [`scripts/sign-release.sh`](../scripts/sign-release.sh) — cosign
   keyless OIDC against Sigstore Fulcio, with Rekor transparency-log
   entries. The `id-token: write` permission on the workflow job is
   what makes the keyless flow work without a stored signing key.
7. Runs [`scripts/verify-release.sh`](../scripts/verify-release.sh) as a
   smoke test against its own output so a broken signing pipeline fails
   the release before any artifact is published.
8. Re-generates a complete `SHA256SUMS` covering binaries, SBOMs, and
   signatures.
9. Publishes everything to a GitHub Release at the tag via
   `softprops/action-gh-release` (SHA-pinned).

The desktop-tool security contract this satisfies is documented in
[ADR 0010](adr/0010-security-contract-desktop-tool.md).

Monitor progress:

```bash
gh run watch --exit-status
```

To re-run a release manually (e.g. transient Sigstore outage), use the
`workflow_dispatch` entry point and pass the existing tag:

```bash
gh workflow run release-build.yml -f tag=v0.2.1
```

---

## 5. Post-Release Verification

### Verify checksums

```bash
VERSION=0.2.1
gh release download v${VERSION} -D /tmp/forgeiso-verify-${VERSION}
cd /tmp/forgeiso-verify-${VERSION}
sha256sum -c checksums.txt
```

### Smoke-install RPM

```bash
sudo rpm -e forgeiso 2>/dev/null || true
sudo rpm -ivh forgeiso-${VERSION}-1.x86_64.rpm
forgeiso --version
forgeiso doctor
```

### Smoke-install DEB

```bash
sudo dpkg -r forgeiso 2>/dev/null || true
sudo dpkg -i forgeiso_${VERSION}-1_amd64.deb
sudo apt-get install -f        # pull in xorriso, squashfs-tools, mtools if missing
sudo apt-get install -y zenity wl-clipboard xdg-utils   # optional GUI helpers
forgeiso --version
forgeiso doctor
```

### Tarball upgrade hygiene

If you tested a tarball before installing a distro package, clear old
`/usr/local/bin` ForgeISO binaries first:

```bash
sudo rm -f /usr/local/bin/forgeiso \
  /usr/local/bin/forgeiso-tui \
  /usr/local/bin/forge-slint \
  /usr/local/bin/forgeiso-desktop
```

This avoids `/usr/local/bin` shadowing newer package-managed binaries in
`/usr/bin`.

---

## 6. Updating PKGBUILD sha256sums

After the tarball is published, update the Arch Linux PKGBUILD with the real checksum:

```bash
VERSION=0.2.1
TARBALL="forgeiso-${VERSION}-linux-x86_64.tar.gz"

# Get checksum from the published checksums.txt
SHA=$(gh release download v${VERSION} -p checksums.txt --clobber -D /tmp \
      && grep "${TARBALL}" /tmp/checksums.txt | awk '{print $1}')

# Update PKGBUILD
sed -i "s/sha256sums=.*/sha256sums=('${SHA}')/" packaging/PKGBUILD
echo "Updated PKGBUILD sha256sums to: ${SHA}"

# Regenerate .SRCINFO (requires makepkg on Arch)
# makepkg --printsrcinfo > packaging/.SRCINFO
```

---

## 7. Dependency Policy (deny.toml)

`deny.toml` at the repo root controls what C2 enforces:

- **Advisories**: HIGH and CRITICAL CVEs → `deny` (fail build)
- **Unmaintained/unsound crates** → `warn` (report, don't fail)
- **Licenses**: Only Apache-2.0, MIT, BSD-*, ISC, and similar permissive licenses allowed
- **GPL/LGPL/AGPL** → `deny` (fail build)
- **Duplicate crates** → `warn` with explicit ecosystem-split exceptions

To update the advisory database locally:

```bash
cargo deny fetch
```

To check against policy:

```bash
cargo deny check advisories    # just CVEs
cargo deny check licenses      # just license compliance
cargo deny check bans          # just duplicate/wildcard bans
cargo deny check               # all checks
```

---

## 8. Version Locations Reference

| File | Field | Notes |
|---|---|---|
| `Cargo.toml` | `[workspace.package] version` | Single source of truth for Rust |
| `packaging/PKGBUILD` | `pkgver` | AUR/Arch package |
| `Cargo.lock` | auto-generated | Updated by `cargo build` |

GUI versions (updated separately when GUI ships):
- `gui/package.json` → `version`
- `gui/src-tauri/Cargo.toml` → `version`
- `gui/src-tauri/tauri.conf.json` → `version`
