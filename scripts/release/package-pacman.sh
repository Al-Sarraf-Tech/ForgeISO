#!/usr/bin/env bash
set -euo pipefail

if ! command -v fpm >/dev/null 2>&1; then
  echo "fpm is required to build pacman package" >&2
  exit 1
fi

VERSION="${1:-$(git describe --tags --abbrev=0 2>/dev/null | sed 's/^v//' || echo 0.1.0)}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${ROOT_DIR}/dist/release"

mkdir -p "${OUT_DIR}"

fpm \
  -s dir \
  -t pacman \
  -n forgeiso \
  -v "${VERSION}" \
  --iteration 1 \
  --architecture x86_64 \
  --license Apache-2.0 \
  --maintainer "Jamal Al-Sarraf <19882582+jalsarraf0@users.noreply.github.com>" \
  --description "Cross-distro ISO customization platform" \
  --url "https://github.com/jalsarraf0/ForgeISO" \
  --depends bash \
  --package "${OUT_DIR}/forgeiso-${VERSION}-1-x86_64.pkg.tar.zst" \
  "${ROOT_DIR}/target/release/forgeiso=/usr/bin/forgeiso" \
  "${ROOT_DIR}/target/release/forgeiso-tui=/usr/bin/forgeiso-tui" \
  "${ROOT_DIR}/target/release/forgeiso-agent=/usr/bin/forgeiso-agent" \
  "${ROOT_DIR}/README.md=/usr/share/doc/forgeiso/README.md" \
  "${ROOT_DIR}/LICENSE=/usr/share/licenses/forgeiso/LICENSE"

echo "Created ${OUT_DIR}/forgeiso-${VERSION}-1-x86_64.pkg.tar.zst"
