#!/usr/bin/env bash
# Build a Linux x86_64 tarball for forgeiso
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
BIN_DIR="$(forgeiso_bin_dir "${ROOT_DIR}")"
RELEASE_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"

forgeiso_require_binary "${BIN_DIR}" "forgeiso"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-tui"

mkdir -p "${RELEASE_DIR}"

STAGE_NAME="forgeiso-${VERSION}-linux-x86_64"
STAGE_DIR="${ROOT_DIR}/dist/${STAGE_NAME}"
ARCHIVE="${RELEASE_DIR}/${STAGE_NAME}.tar.gz"
STAGING="$(mktemp -d)"

rm -rf "${STAGE_DIR}"
trap 'rm -rf "${STAGE_DIR}" "${STAGING}"' EXIT

mkdir -p "${STAGE_DIR}/bin"

cp "${BIN_DIR}/forgeiso"     "${STAGE_DIR}/bin/"
cp "${BIN_DIR}/forgeiso-tui" "${STAGE_DIR}/bin/"
cp "${ROOT_DIR}/scripts/release/forgeiso-desktop" "${STAGE_DIR}/bin/"
# Slint GUI — include if built
if [[ -f "${BIN_DIR}/forge-slint" ]]; then
  cp "${BIN_DIR}/forge-slint" "${STAGE_DIR}/bin/"
fi
cp "${ROOT_DIR}/README.md"   "${STAGE_DIR}/README.md"
printf '%s\n' "${VERSION}" > "${STAGE_DIR}/VERSION"

forgeiso_build_staging "${BIN_DIR}" "${ROOT_DIR}" "${STAGING}"
cp -a "${STAGING}/usr/share" "${STAGE_DIR}/share"

tar -C "${ROOT_DIR}/dist" -czf "${ARCHIVE}" "${STAGE_NAME}"
rm -rf "${STAGE_DIR}" "${STAGING}"

echo "[tarball] OK: ${ARCHIVE}"
ls -lh "${ARCHIVE}"
