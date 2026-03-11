#!/usr/bin/env bash
# cache-images.sh — pre-build ForgeISO CI stage images, store one tar per stage in /tmp.
#
# Layout (shared root, one tar per stage — no stale multi-version pile-up):
#   /tmp/ci-cache/forgeiso/c1.tar   ← image archive
#   /tmp/ci-cache/forgeiso/c1.sha   ← Dockerfile sha256 the tar was built from
#
# A stage is rebuilt only when its Dockerfile has changed.
# Writes are atomic (write to .tmp → mv) so concurrent readers always see
# a complete tar, never a partial file.
#
# Usage:
#   bash scripts/ci/cache-images.sh               # warm/refresh all 7 stages
#   bash scripts/ci/cache-images.sh c1 c3 c7      # specific stages only
#
# Environment:
#   CI_CACHE_ROOT   Shared cache root (default: /tmp/ci-cache)
#   FORGEISO_CI_FORCE=1   Force full rebuild even on cache hit

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CI_CACHE_ROOT="${CI_CACHE_ROOT:-/tmp/ci-cache}"
CACHE_DIR="${CI_CACHE_ROOT}/forgeiso"
FORCE="${FORGEISO_CI_FORCE:-0}"

# ── Colour helpers ────────────────────────────────────────────────────────────
GREEN='\033[0;32m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'
RED='\033[0;31m'; BOLD='\033[1m'; RESET='\033[0m'
ok()    { echo -e "${GREEN}${BOLD}  CACHED${RESET}  $*"; }
hit()   { echo -e "${CYAN}${BOLD}  HIT   ${RESET}  $*"; }
build() { echo -e "${YELLOW}${BOLD}  BUILD ${RESET}  $*"; }
fail()  { echo -e "${RED}${BOLD}  FAIL  ${RESET}  $*"; }
info()  { echo -e "${CYAN}▶${RESET} $*"; }

# ── Stage → Dockerfile map ────────────────────────────────────────────────────
declare -A STAGE_DOCKERFILES=(
    [c1]="C1.rust.Dockerfile"
    [c2]="C2.sbom.Dockerfile"
    [c3]="C3.gui.Dockerfile"
    [c4]="C4.security.Dockerfile"
    [c5]="C5.integration.Dockerfile"
    [c6]="C6.e2e.Dockerfile"
    [c7]="C7.lint.Dockerfile"
)

ALL_STAGES=(c1 c2 c3 c4 c5 c6 c7)
STAGES=("${ALL_STAGES[@]}")
if [[ $# -gt 0 ]]; then
    STAGES=("$@")
fi

mkdir -p "${CACHE_DIR}"

# ── Helpers ───────────────────────────────────────────────────────────────────
dockerfile_hash() { sha256sum "$1" | awk '{print $1}'; }
image_tag()       { echo "forgeiso-${1}"; }
tar_path()        { echo "${CACHE_DIR}/${1}.tar"; }
sha_path()        { echo "${CACHE_DIR}/${1}.sha"; }

cache_valid() {
    local stage="$1"
    local dockerfile="${REPO_ROOT}/containers/${STAGE_DOCKERFILES[$stage]}"
    local sha_file
    sha_file="$(sha_path "${stage}")"
    local tar_file
    tar_file="$(tar_path "${stage}")"

    [[ -f "${tar_file}" ]] || return 1
    [[ -f "${sha_file}" ]] || return 1
    [[ "$(cat "${sha_file}")" == "$(dockerfile_hash "${dockerfile}")" ]] || return 1
    return 0
}

# ── Partition: hits vs needs-build ───────────────────────────────────────────
declare -a CACHE_HITS=()
declare -a NEED_BUILD=()

for stage in "${STAGES[@]}"; do
    if [[ -z "${STAGE_DOCKERFILES[$stage]:-}" ]]; then
        echo "WARNING: unknown stage '${stage}' — skipping" >&2
        continue
    fi
    if [[ "${FORCE}" != "1" ]] && cache_valid "${stage}"; then
        CACHE_HITS+=("${stage}")
    else
        NEED_BUILD+=("${stage}")
    fi
done

# ── Load cache hits (parallel) ────────────────────────────────────────────────
if [[ ${#CACHE_HITS[@]} -gt 0 ]]; then
    info "Loading ${#CACHE_HITS[@]} cached image(s) …"
    declare -A LOAD_PIDS=()
    for stage in "${CACHE_HITS[@]}"; do
        tar_file="$(tar_path "${stage}")"
        (
            docker load -i "${tar_file}" >/dev/null 2>&1
            hit "${stage}  ← $(du -sh "${tar_file}" | cut -f1)  [${tar_file}]"
        ) &
        LOAD_PIDS[$stage]=$!
    done
    for stage in "${!LOAD_PIDS[@]}"; do
        wait "${LOAD_PIDS[$stage]}" || { fail "${stage} failed to load"; exit 1; }
    done
    echo ""
fi

# ── Full builds for misses (parallel, atomic write) ───────────────────────────
if [[ ${#NEED_BUILD[@]} -gt 0 ]]; then
    info "Full-building ${#NEED_BUILD[@]} stage(s): ${NEED_BUILD[*]} …"
    declare -A BUILD_PIDS=()
    declare -A BUILD_LOGS=()

    for stage in "${NEED_BUILD[@]}"; do
        dockerfile="${REPO_ROOT}/containers/${STAGE_DOCKERFILES[$stage]}"
        img="$(image_tag "${stage}")"
        tar_file="$(tar_path "${stage}")"
        sha_file="$(sha_path "${stage}")"
        log="$(mktemp /tmp/ci-build-${stage}-XXXXXX.log)"
        BUILD_LOGS[$stage]="${log}"

        (
            # Full build — no layer reuse from prior builds
            docker build \
                --no-cache \
                --tag "${img}" \
                --file "${dockerfile}" \
                "${REPO_ROOT}" \
                >"${log}" 2>&1

            # Atomic write: save to .tmp then rename so readers never see a partial tar
            docker save "${img}" -o "${tar_file}.tmp"
            mv -f "${tar_file}.tmp" "${tar_file}"

            # Record the Dockerfile hash this tar was built from
            dockerfile_hash "${dockerfile}" > "${sha_file}"
        ) &
        BUILD_PIDS[$stage]=$!
        build "${stage}  building …"
    done

    echo ""
    FAILED=0
    for stage in "${NEED_BUILD[@]}"; do
        tar_file="$(tar_path "${stage}")"
        if wait "${BUILD_PIDS[$stage]}"; then
            ok "${stage}  → $(du -sh "${tar_file}" | cut -f1)  [${tar_file}]"
            rm -f "${BUILD_LOGS[$stage]}"
        else
            fail "${stage}"
            echo "  Log: ${BUILD_LOGS[$stage]}"
            tail -20 "${BUILD_LOGS[$stage]}" | sed 's/^/  | /'
            FAILED=$((FAILED + 1))
        fi
    done

    if [[ $FAILED -gt 0 ]]; then
        echo ""
        echo -e "${RED}${BOLD}cache-images: ${FAILED} build(s) failed.${RESET}" >&2
        exit 1
    fi
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}══ ${CACHE_DIR} ══${RESET}"
for f in "${CACHE_DIR}"/*.tar; do
    [[ -f "${f}" ]] || continue
    stage="$(basename "${f}" .tar)"
    sha_file="${CACHE_DIR}/${stage}.sha"
    hash_short="$(cut -c1-12 "${sha_file}" 2>/dev/null || echo "?")"
    echo "  $(du -sh "${f}" | cut -f1)  ${stage}.tar  [df-sha: ${hash_short}]"
done
echo ""
echo -e "${GREEN}${BOLD}All stages ready.${RESET}"
