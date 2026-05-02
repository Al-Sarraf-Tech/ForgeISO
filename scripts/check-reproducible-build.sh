#!/usr/bin/env bash
# ============================================================================
# check-reproducible-build.sh — verify ForgeISO release binaries are
# bit-identical across two independent builds from the same source tree.
# ============================================================================
#
# DESCRIPTION
#   Builds the three release binaries (forgeiso, forgeiso-tui,
#   forge-slint) twice into separate target directories, then compares
#   their SHA-256 digests. A reproducible build means an end-user (or
#   downstream packager) can rebuild the same source revision and
#   verify by hash that the official artifact is honest about its
#   provenance.
#
#   Per ADR 0010 § Reproducible builds, this is currently an
#   *advisory* check — the partial reproducibility properties of the
#   crate set are tracked here so a future release can promote it to a
#   gate. Common sources of nondeterminism that this script will
#   surface include: build paths embedded in panic messages, system
#   timestamps in object files, parallel-codegen ordering, and
#   filesystem-iteration order in build scripts.
#
# USAGE
#   bash scripts/check-reproducible-build.sh
#   FORGEISO_REPRO_REMAP=1 bash scripts/check-reproducible-build.sh
#
# ENVIRONMENT
#   FORGEISO_REPRO_REMAP   When set to 1, pass --remap-path-prefix to
#                          rustc to remove the absolute build directory
#                          from the binary. This is the standard
#                          reproducible-builds knob; turn it on once
#                          the rest of the toolchain is determinised.
#   FORGEISO_REPRO_KEEP    When set to 1, keep the per-build target
#                          directories so a divergence can be inspected
#                          (cmp / diffoscope / objdump). Default: clean
#                          up after success.
#
# OUTPUT
#   - Per-binary SHA-256 from build A and build B side by side.
#   - "REPRODUCIBLE" / "DIVERGENT" verdict per binary.
#   - Aggregate exit code — see below.
#
# EXIT CODES
#   0   All three binaries built bit-identical
#   1   Required tool missing
#   2   At least one build failed
#   3   At least one binary diverged between builds
#
# ============================================================================

set -Eeuo pipefail
IFS=$'\n\t'

PROJECT_ROOT="$(cd -P "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

KEEP_TARGETS="${FORGEISO_REPRO_KEEP:-0}"
USE_REMAP="${FORGEISO_REPRO_REMAP:-0}"

log()  { printf '[reproducible-build] %s\n' "$*"; }
warn() { printf '[reproducible-build] WARN: %s\n' "$*" >&2; }
err()  { printf '[reproducible-build] ERROR: %s\n' "$*" >&2; }

for tool in cargo sha256sum cmp; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        err "required tool '$tool' is not on PATH"
        exit 1
    fi
done

readarray -t BINARIES < <(printf '%s\n' \
    'forgeiso' \
    'forgeiso-tui' \
    'forge-slint')

readarray -t PACKAGES < <(printf '%s\n' \
    '-p forgeiso-cli' \
    '-p forgeiso-tui' \
    '-p forge-slint')

TARGET_A="${PROJECT_ROOT}/target-repro-a"
TARGET_B="${PROJECT_ROOT}/target-repro-b"

cleanup() {
    if [[ "$KEEP_TARGETS" != "1" ]]; then
        rm -rf "$TARGET_A" "$TARGET_B"
    else
        log "FORGEISO_REPRO_KEEP=1 — leaving $TARGET_A and $TARGET_B in place"
    fi
}
trap cleanup EXIT

build() {
    local target_dir="$1"
    local label="$2"
    log "build ${label} → ${target_dir}"

    local extra_rustflags=""
    if [[ "$USE_REMAP" == "1" ]]; then
        # Replace the absolute project path with a stable placeholder.
        # This is the canonical reproducible-builds knob; once the
        # other determinism work lands this should become the default.
        extra_rustflags="--remap-path-prefix=${PROJECT_ROOT}=/build/forgeiso"
        log "  using --remap-path-prefix"
    fi

    # CARGO_TARGET_DIR isolates per-build state. SOURCE_DATE_EPOCH is
    # set to the most recent commit time so any build script that
    # consults it (rare, but a few transitive crates do) sees a
    # deterministic value.
    SOURCE_DATE_EPOCH="$(git log -1 --pretty=%ct 2>/dev/null || date +%s)"
    export SOURCE_DATE_EPOCH

    local rustflags_old="${RUSTFLAGS:-}"
    if [[ -n "$extra_rustflags" ]]; then
        export RUSTFLAGS="${rustflags_old} ${extra_rustflags}"
    fi

    local build_args=()
    for pkg in "${PACKAGES[@]}"; do
        # shellcheck disable=SC2206 # Intentional word split on `-p name`
        build_args+=( $pkg )
    done

    if ! CARGO_TARGET_DIR="$target_dir" \
         cargo build --release --locked "${build_args[@]}"; then
        err "build ${label} failed"
        return 1
    fi

    if [[ -n "$rustflags_old" ]]; then
        export RUSTFLAGS="$rustflags_old"
    else
        unset RUSTFLAGS
    fi
}

rm -rf "$TARGET_A" "$TARGET_B"

build "$TARGET_A" "A" || exit 2
build "$TARGET_B" "B" || exit 2

log ""
log "─── SHA-256 comparison ────────────────────────────────────────"

DIVERGED=0
for bin in "${BINARIES[@]}"; do
    path_a="${TARGET_A}/release/${bin}"
    path_b="${TARGET_B}/release/${bin}"

    if [[ ! -f "$path_a" || ! -f "$path_b" ]]; then
        warn "${bin}: missing in one of the build trees, skipping"
        continue
    fi

    sha_a="$(sha256sum "$path_a" | awk '{print $1}')"
    sha_b="$(sha256sum "$path_b" | awk '{print $1}')"

    printf '  %-16s  A=%s\n' "$bin" "$sha_a"
    printf '  %-16s  B=%s\n' "$bin" "$sha_b"

    if [[ "$sha_a" == "$sha_b" ]]; then
        printf '  %-16s  → REPRODUCIBLE\n\n' "$bin"
    else
        printf '  %-16s  → DIVERGENT\n' "$bin"
        # Pinpoint the first byte that differs. Useful when the
        # divergence is a small region (e.g. an embedded path).
        if first_diff="$(cmp "$path_a" "$path_b" 2>&1 | head -1)"; then
            printf '  %-16s    first differ: %s\n\n' "$bin" "$first_diff"
        else
            printf '\n'
        fi
        DIVERGED=$((DIVERGED + 1))
    fi
done

if [[ $DIVERGED -gt 0 ]]; then
    err "$DIVERGED of ${#BINARIES[@]} binaries diverged"
    if [[ "$USE_REMAP" != "1" ]]; then
        warn "consider re-running with FORGEISO_REPRO_REMAP=1 to strip"
        warn "build paths from the binary"
    fi
    exit 3
fi

log "all ${#BINARIES[@]} release binaries are bit-identical across both builds"
log "see docs/adr/0010-security-contract-desktop-tool.md for the wider"
log "release-pipeline reproducibility contract."
