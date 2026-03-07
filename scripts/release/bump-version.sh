#!/usr/bin/env bash
# Bump ForgeISO version in all required locations.
# Usage: bash scripts/release/bump-version.sh <NEW_VERSION>
# Example: bash scripts/release/bump-version.sh 1.0.0
set -euo pipefail

NEW_VERSION="${1:-}"
if [[ -z "${NEW_VERSION}" ]]; then
  echo "ERROR: version argument required" >&2
  echo "Usage: $0 <version>  (e.g. $0 1.0.0)" >&2
  exit 1
fi

# Validate semver format
if ! [[ "${NEW_VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "ERROR: version must be in semver format X.Y.Z (got '${NEW_VERSION}')" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"

echo "Bumping ForgeISO to v${NEW_VERSION}"

# ── 1. Cargo.toml workspace version ─────────────────────────────────────────
CARGO_TOML="${ROOT_DIR}/Cargo.toml"
OLD_VERSION="$(grep -E '^version = ' "${CARGO_TOML}" | head -1 | sed 's/version = "\(.*\)"/\1/')"

if [[ "${OLD_VERSION}" == "${NEW_VERSION}" ]]; then
  echo "  Cargo.toml already at ${NEW_VERSION}, skipping"
else
  sed -i "s/^version = \"${OLD_VERSION}\"/version = \"${NEW_VERSION}\"/" "${CARGO_TOML}"
  echo "  Cargo.toml: ${OLD_VERSION} → ${NEW_VERSION}"
fi

# ── 2. PKGBUILD ──────────────────────────────────────────────────────────────
PKGBUILD="${ROOT_DIR}/packaging/PKGBUILD"
if [[ -f "${PKGBUILD}" ]]; then
  OLD_PKGVER="$(grep '^pkgver=' "${PKGBUILD}" | cut -d= -f2)"
  if [[ "${OLD_PKGVER}" == "${NEW_VERSION}" ]]; then
    echo "  PKGBUILD already at ${NEW_VERSION}, skipping"
  else
    sed -i "s/^pkgver=.*/pkgver=${NEW_VERSION}/" "${PKGBUILD}"
    # Reset sha256sums to SKIP (will be updated after release tarball is published)
    sed -i "s/^sha256sums=.*/sha256sums=('SKIP')  # Replace with actual sha256 after release/" "${PKGBUILD}"
    echo "  PKGBUILD: ${OLD_PKGVER} → ${NEW_VERSION} (sha256sums reset to SKIP)"
  fi
fi

# ── 3. GUI — package.json ────────────────────────────────────────────────────
GUI_PKG="${ROOT_DIR}/gui/package.json"
if [[ -f "${GUI_PKG}" ]]; then
  OLD_GUI="$(python3 -c "import sys,json; print(json.load(open('${GUI_PKG}'))['version'])")"
  if [[ "${OLD_GUI}" == "${NEW_VERSION}" ]]; then
    echo "  gui/package.json already at ${NEW_VERSION}, skipping"
  else
    sed -i "s/\"version\": \"${OLD_GUI}\"/\"version\": \"${NEW_VERSION}\"/" "${GUI_PKG}"
    echo "  gui/package.json: ${OLD_GUI} → ${NEW_VERSION}"
  fi
fi

# ── 4. GUI — src-tauri/Cargo.toml ───────────────────────────────────────────
GUI_CARGO="${ROOT_DIR}/gui/src-tauri/Cargo.toml"
if [[ -f "${GUI_CARGO}" ]]; then
  OLD_GUI_CARGO="$(grep -E '^version = ' "${GUI_CARGO}" | head -1 | sed 's/version = "\(.*\)"/\1/')"
  if [[ "${OLD_GUI_CARGO}" == "${NEW_VERSION}" ]]; then
    echo "  gui/src-tauri/Cargo.toml already at ${NEW_VERSION}, skipping"
  else
    sed -i "0,/^version = \"${OLD_GUI_CARGO}\"/{s/^version = \"${OLD_GUI_CARGO}\"/version = \"${NEW_VERSION}\"/}" "${GUI_CARGO}"
    echo "  gui/src-tauri/Cargo.toml: ${OLD_GUI_CARGO} → ${NEW_VERSION}"
  fi
fi

# ── 5. GUI — tauri.conf.json ─────────────────────────────────────────────────
TAURI_CONF="${ROOT_DIR}/gui/src-tauri/tauri.conf.json"
if [[ -f "${TAURI_CONF}" ]]; then
  OLD_TAURI="$(python3 -c "import sys,json; print(json.load(open('${TAURI_CONF}')).get('version',''))")"
  if [[ "${OLD_TAURI}" == "${NEW_VERSION}" ]]; then
    echo "  gui/src-tauri/tauri.conf.json already at ${NEW_VERSION}, skipping"
  else
    python3 - <<PYEOF
import json, sys
with open('${TAURI_CONF}') as f:
    d = json.load(f)
d['version'] = '${NEW_VERSION}'
with open('${TAURI_CONF}', 'w') as f:
    json.dump(d, f, indent=2)
    f.write('\n')
PYEOF
    echo "  gui/src-tauri/tauri.conf.json: ${OLD_TAURI} → ${NEW_VERSION}"
  fi
fi

# ── 6. Regenerate Cargo.lock ─────────────────────────────────────────────────
echo "  Regenerating Cargo.lock..."
(cd "${ROOT_DIR}" && cargo generate-lockfile --quiet)
echo "  Cargo.lock updated"

# ── 7. Summary ──────────────────────────────────────────────────────────────
echo ""
echo "Version bump complete: v${OLD_VERSION:-unknown} → v${NEW_VERSION}"
echo ""
echo "Changed files:"
echo "  ${CARGO_TOML}"
[[ -f "${PKGBUILD}" ]]   && echo "  ${PKGBUILD}"
[[ -f "${GUI_PKG}" ]]    && echo "  ${GUI_PKG}"
[[ -f "${GUI_CARGO}" ]]  && echo "  ${GUI_CARGO}"
[[ -f "${TAURI_CONF}" ]] && echo "  ${TAURI_CONF}"
echo "  ${ROOT_DIR}/Cargo.lock"
echo ""
echo "Next steps:"
echo "  1. cargo build --release -p forgeiso-cli  # verify it compiles"
echo "  2. git add Cargo.toml Cargo.lock packaging/PKGBUILD gui/package.json gui/src-tauri/Cargo.toml gui/src-tauri/tauri.conf.json"
echo "  3. git commit -m 'chore: bump version to v${NEW_VERSION}'"
echo "  4. Push branch + open PR"
echo "  5. After merge: git tag -a v${NEW_VERSION} -m 'Release v${NEW_VERSION}' && git push origin v${NEW_VERSION}"
