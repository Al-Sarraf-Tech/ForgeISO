#!/usr/bin/env bash
# Verify that all expected release artifacts exist and are non-empty
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
RELEASE_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"

PASS=0
FAIL=0

check_file() {
  local path="$1"
  local desc="$2"
  if [[ -f "${path}" && -s "${path}" ]]; then
    printf "  ✓ %-50s  %s\n" "${desc}" "$(ls -lh "${path}" | awk '{print $5}')"
    (( PASS++ )) || true
  else
    printf "  ✗ %-50s  MISSING OR EMPTY\n" "${desc}" >&2
    (( FAIL++ )) || true
  fi
}

check_repo_line() {
  local path="$1"
  local pattern="$2"
  local desc="$3"
  if grep -q "${pattern}" "${path}"; then
    printf "  ✓ %-50s  OK\n" "${desc}"
    (( PASS++ )) || true
  else
    printf "  ✗ %-50s  MISMATCH\n" "${desc}" >&2
    (( FAIL++ )) || true
  fi
}

check_tar_member() {
  local archive="$1"
  local pattern="$2"
  local desc="$3"
  local listing
  listing="$(mktemp)"
  if tar -tzf "${archive}" > "${listing}" 2>/dev/null && grep -q "${pattern}" "${listing}"; then
    printf "  ✓ %-50s  OK\n" "${desc}"
    (( PASS++ )) || true
  else
    printf "  ✗ %-50s  MISSING\n" "${desc}" >&2
    (( FAIL++ )) || true
  fi
  rm -f "${listing}"
}

check_rpm_member() {
  local archive="$1"
  local pattern="$2"
  local desc="$3"
  local listing
  listing="$(mktemp)"
  if rpm -qlp "${archive}" > "${listing}" 2>/dev/null && grep -qx "${pattern}" "${listing}"; then
    printf "  ✓ %-50s  OK\n" "${desc}"
    (( PASS++ )) || true
  else
    printf "  ✗ %-50s  MISSING\n" "${desc}" >&2
    (( FAIL++ )) || true
  fi
  rm -f "${listing}"
}

check_deb_member() {
  local archive="$1"
  local pattern="$2"
  local desc="$3"
  local listing
  listing="$(mktemp)"
  if dpkg-deb -c "${archive}" > "${listing}" 2>/dev/null && grep -q "${pattern}" "${listing}"; then
    printf "  ✓ %-50s  OK\n" "${desc}"
    (( PASS++ )) || true
  else
    printf "  ✗ %-50s  MISSING\n" "${desc}" >&2
    (( FAIL++ )) || true
  fi
  rm -f "${listing}"
}

check_pacman_member() {
  local archive="$1"
  local pattern="$2"
  local desc="$3"
  local listing
  listing="$(mktemp)"
  if bsdtar -tf "${archive}" > "${listing}" 2>/dev/null && grep -qx "${pattern}" "${listing}"; then
    printf "  ✓ %-50s  OK\n" "${desc}"
    (( PASS++ )) || true
  else
    printf "  ✗ %-50s  MISSING\n" "${desc}" >&2
    (( FAIL++ )) || true
  fi
  rm -f "${listing}"
}

check_launcher_prefers_sibling_binary() {
  local staging path_bin output
  staging="$(mktemp -d)"
  path_bin="${staging}/path-bin"
  mkdir -p "${path_bin}"
  trap 'rm -rf "${staging}"' RETURN

  cp "${ROOT_DIR}/scripts/release/forgeiso-desktop" "${staging}/forgeiso-desktop"
  cat > "${staging}/forge-slint" <<'EOF'
#!/usr/bin/env bash
printf 'sibling-slint\n'
EOF
  chmod +x "${staging}/forgeiso-desktop" "${staging}/forge-slint"

  cat > "${path_bin}/forge-slint" <<'EOF'
#!/usr/bin/env bash
printf 'path-slint\n'
EOF
  chmod +x "${path_bin}/forge-slint"

  if output="$(env -i DISPLAY=:0 PATH="${path_bin}:/usr/bin:/bin" "${staging}/forgeiso-desktop" 2>/dev/null)" \
    && [[ "${output}" == "sibling-slint" ]]; then
    printf "  ✓ %-50s  OK\n" "Launcher prefers sibling binary over PATH shadow"
    (( PASS++ )) || true
  else
    printf "  ✗ %-50s  FAILED\n" "Launcher prefers sibling binary over PATH shadow" >&2
    (( FAIL++ )) || true
  fi

  rm -rf "${staging}"
  trap - RETURN
}

echo "ForgeISO ${VERSION} — release verification"
echo "Release dir: ${RELEASE_DIR}"
echo ""

check_file "${RELEASE_DIR}/forgeiso-${VERSION}-linux-x86_64.tar.gz"  "Tarball (linux-x86_64)"
check_file "${RELEASE_DIR}/forgeiso-${VERSION}-1.x86_64.rpm"         "RPM (Fedora/RHEL/openSUSE)"
check_file "${RELEASE_DIR}/forgeiso_${VERSION}-1_amd64.deb"          "DEB (Debian/Ubuntu)"
check_file "${RELEASE_DIR}/forgeiso-${VERSION}-1-x86_64.pkg.tar.zst" "Pacman (Arch Linux)"
check_file "${RELEASE_DIR}/checksums.txt"                              "SHA-256 checksums"

if [[ -f "${RELEASE_DIR}/checksums.txt" ]]; then
  (
    cd "${RELEASE_DIR}"
    if sha256sum -c checksums.txt >/dev/null 2>&1; then
      printf "  ✓ %-50s  OK\n" "SHA-256 verification"
      (( PASS++ )) || true
    else
      printf "  ✗ %-50s  FAILED\n" "SHA-256 verification" >&2
      (( FAIL++ )) || true
    fi
  )
fi

check_repo_line "${ROOT_DIR}/README.md" "Current version: v${VERSION}" "README version banner"
check_repo_line "${ROOT_DIR}/README.md" "/usr/local/bin" "README tarball shadowing guidance"
check_repo_line "${ROOT_DIR}/packaging/PKGBUILD" "pkgver=${VERSION}" "PKGBUILD version"
check_launcher_prefers_sibling_binary
check_repo_line "${ROOT_DIR}/scripts/release/forgeiso-desktop" 'slint_bin="$(_resolve_binary forge-slint)"' "Launcher resolves forge-slint first"
check_repo_line "${ROOT_DIR}/scripts/release/common.sh" "Exec=forgeiso-desktop" "Desktop file launches forgeiso-desktop"

TARBALL="${RELEASE_DIR}/forgeiso-${VERSION}-linux-x86_64.tar.gz"
RPM="${RELEASE_DIR}/forgeiso-${VERSION}-1.x86_64.rpm"
DEB="${RELEASE_DIR}/forgeiso_${VERSION}-1_amd64.deb"
PACMAN="${RELEASE_DIR}/forgeiso-${VERSION}-1-x86_64.pkg.tar.zst"

if [[ -f "${TARBALL}" ]]; then
  check_tar_member "${TARBALL}" '.*/bin/forge-slint$' "Tarball includes forge-slint"
  check_tar_member "${TARBALL}" '.*/bin/forgeiso-desktop$' "Tarball includes launcher"
  check_tar_member "${TARBALL}" '.*/share/applications/forgeiso.desktop$' "Tarball includes desktop file"
  check_tar_member "${TARBALL}" '.*/share/pixmaps/forgeiso.png$' "Tarball includes icon"
  check_tar_member "${TARBALL}" '.*/share/man/man1/forgeiso.1.gz$' "Tarball includes man page"
  check_tar_member "${TARBALL}" '.*/share/bash-completion/completions/forgeiso$' "Tarball includes bash completion"
fi
if [[ -f "${RPM}" ]]; then
  check_rpm_member "${RPM}" "/usr/bin/forge-slint" "RPM includes forge-slint"
  check_rpm_member "${RPM}" "/usr/bin/forgeiso-desktop" "RPM includes launcher"
  check_rpm_member "${RPM}" "/usr/share/applications/forgeiso.desktop" "RPM includes desktop file"
  check_rpm_member "${RPM}" "/usr/share/pixmaps/forgeiso.png" "RPM includes icon"
  check_rpm_member "${RPM}" "/usr/share/man/man1/forgeiso.1.gz" "RPM includes man page"
  check_rpm_member "${RPM}" "/usr/share/bash-completion/completions/forgeiso" "RPM includes bash completion"
fi
if [[ -f "${DEB}" ]]; then
  check_deb_member "${DEB}" './usr/bin/forge-slint' "DEB includes forge-slint"
  check_deb_member "${DEB}" './usr/bin/forgeiso-desktop' "DEB includes launcher"
  check_deb_member "${DEB}" './usr/share/applications/forgeiso.desktop' "DEB includes desktop file"
  check_deb_member "${DEB}" './usr/share/pixmaps/forgeiso.png' "DEB includes icon"
  check_deb_member "${DEB}" './usr/share/man/man1/forgeiso.1.gz' "DEB includes man page"
  check_deb_member "${DEB}" './usr/share/bash-completion/completions/forgeiso' "DEB includes bash completion"
fi
if [[ -f "${PACMAN}" ]]; then
  check_pacman_member "${PACMAN}" "usr/bin/forge-slint" "Pacman package includes forge-slint"
  check_pacman_member "${PACMAN}" "usr/bin/forgeiso-desktop" "Pacman package includes launcher"
  check_pacman_member "${PACMAN}" "usr/share/applications/forgeiso.desktop" "Pacman package includes desktop file"
  check_pacman_member "${PACMAN}" "usr/share/pixmaps/forgeiso.png" "Pacman package includes icon"
  check_pacman_member "${PACMAN}" "usr/share/man/man1/forgeiso.1.gz" "Pacman package includes man page"
  check_pacman_member "${PACMAN}" "usr/share/bash-completion/completions/forgeiso" "Pacman package includes bash completion"
fi

echo ""
echo "Results: ${PASS} passed, ${FAIL} failed"

if (( FAIL > 0 )); then
  echo "Release verification FAILED — run make-packages.sh to rebuild." >&2
  exit 1
fi

echo "✓ Release verification passed for ForgeISO ${VERSION}"
