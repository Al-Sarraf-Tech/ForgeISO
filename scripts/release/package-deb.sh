#!/usr/bin/env bash
set -euo pipefail

if ! command -v dpkg-deb >/dev/null 2>&1; then
  echo "dpkg-deb is required" >&2
  exit 1
fi

VERSION="${1:-$(git describe --tags --abbrev=0 2>/dev/null | sed 's/^v//' || echo 0.1.0)}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PKG_DIR="${ROOT_DIR}/dist/deb/forgeiso_${VERSION}_amd64"
OUT_DIR="${ROOT_DIR}/dist/release"

rm -rf "${PKG_DIR}"
mkdir -p "${PKG_DIR}/DEBIAN" "${PKG_DIR}/usr/bin" "${PKG_DIR}/usr/share/doc/forgeiso"
mkdir -p "${OUT_DIR}"

cp "${ROOT_DIR}/target/release/forgeiso" "${PKG_DIR}/usr/bin/"
cp "${ROOT_DIR}/target/release/forgeiso-tui" "${PKG_DIR}/usr/bin/"
cp "${ROOT_DIR}/target/release/forgeiso-agent" "${PKG_DIR}/usr/bin/"
cp "${ROOT_DIR}/README.md" "${ROOT_DIR}/LICENSE" "${PKG_DIR}/usr/share/doc/forgeiso/"

cat > "${PKG_DIR}/DEBIAN/control" <<CONTROL
Package: forgeiso
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Jamal Al-Sarraf <19882582+jalsarraf0@users.noreply.github.com>
Depends: bash, docker.io | podman
Description: Cross-distro ISO customization platform
 ForgeISO provides enterprise ISO customization with CLI, TUI, GUI, and optional remote agent support.
CONTROL

dpkg-deb --build "${PKG_DIR}" "${OUT_DIR}/forgeiso_${VERSION}_amd64.deb"
echo "Created ${OUT_DIR}/forgeiso_${VERSION}_amd64.deb"
