#!/usr/bin/env bash
# ============================================================================
# download-real-lts.sh — fetch the pinned Ubuntu Server LTS ISO into the
# ForgeISO cache so engine/tests/real_lts_integration.rs can run.
# ============================================================================
#
# DESCRIPTION
#   Downloads the upstream Ubuntu 24.04.4 LTS Server ("Noble Numbat" .4
#   point release) live installer ISO and verifies its SHA-256 against
#   the value pinned in this script. If the cached file is already
#   present and matches the pinned hash, the download is skipped.
#
# USAGE
#   bash tests/fixtures/download-real-lts.sh
#
# ENVIRONMENT
#   FORGEISO_LTS_CACHE_DIR  Override the cache directory. Default:
#                           "$HOME/.cache/forgeiso". Must be on a
#                           filesystem with at least 4 GiB free.
#
# DEPENDENCIES
#   curl, sha256sum
#
# EXIT CODES
#   0   Cache already populated, or download succeeded and sha256 matches
#   1   Download failed or sha256 mismatch — see stderr
#   2   Required tool missing
#
# ROTATING THE PINNED VERSION
#   When upstream cuts a new .x point release of 24.04 (or you bump to
#   the next LTS), update PINNED_FILENAME, PINNED_URL, and PINNED_SHA256
#   below in lockstep with engine/tests/real_lts_integration.rs.
# ============================================================================

set -euo pipefail

PINNED_FILENAME="ubuntu-24.04.4-live-server-amd64.iso"
PINNED_URL="https://releases.ubuntu.com/24.04.4/${PINNED_FILENAME}"
PINNED_SHA256="e907d92eeec9df64163a7e454cbc8d7755e8ddc7ed42f99dbc80c40f1a138433"

CACHE_DIR="${FORGEISO_LTS_CACHE_DIR:-$HOME/.cache/forgeiso}"

log() {
    printf '[download-real-lts] %s\n' "$*" >&2
}

require_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        log "FATAL: required tool '$1' not on PATH"
        exit 2
    fi
}

require_tool curl
require_tool sha256sum

mkdir -p "$CACHE_DIR"
TARGET="${CACHE_DIR}/${PINNED_FILENAME}"

verify_sha() {
    local file="$1"
    local actual
    actual="$(sha256sum "$file" | awk '{print $1}')"
    if [ "$actual" = "$PINNED_SHA256" ]; then
        return 0
    fi
    log "sha256 mismatch for $file"
    log "  expected: $PINNED_SHA256"
    log "  actual:   $actual"
    return 1
}

if [ -f "$TARGET" ]; then
    log "found cached file at $TARGET, verifying sha256"
    if verify_sha "$TARGET"; then
        log "cached ISO matches pinned hash — nothing to do"
        exit 0
    fi
    log "cached ISO is corrupt or rotated upstream; redownloading"
    rm -f "$TARGET"
fi

log "downloading ${PINNED_URL}"
log "  → ${TARGET}"
log "  (~3.2 GiB; this can take several minutes on a typical link)"

# --fail makes curl exit non-zero on HTTP 4xx/5xx instead of writing the
# error body to disk. --location follows the releases.ubuntu.com →
# mirror redirect. --retry covers transient network blips.
curl \
    --fail \
    --location \
    --retry 3 \
    --retry-delay 5 \
    --output "${TARGET}.partial" \
    "${PINNED_URL}"

mv "${TARGET}.partial" "${TARGET}"

if ! verify_sha "$TARGET"; then
    log "downloaded ISO failed sha256 verification — refusing to keep it"
    rm -f "$TARGET"
    exit 1
fi

log "download complete and verified"
log "to run the integration test next:"
log "  FORGEISO_RUN_REAL_LTS=1 cargo test --workspace -- --ignored real_lts_integration"
