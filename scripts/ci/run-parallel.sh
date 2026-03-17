#!/usr/bin/env bash
# ForgeISO parallel CI runner
#
# Builds all 7 CI images in parallel, then launches every stage simultaneously
# in its own ephemeral container with an isolated Cargo target volume.
# Waits for every job to finish, reports pass/fail for each, then tears down
# ALL volumes and containers.  Exits 0 only when every stage passes.
#
# Usage:
#   bash scripts/ci/run-parallel.sh           # all 7 stages
#   bash scripts/ci/run-parallel.sh c1 c3     # selected stages only
#
# Environment:
#   CI_STAGES=c1,c2,c3,c4,c5,c6,c7   override which stages to run (comma-separated)
#   FORGEISO_CI_VERBOSE=1          stream container stdout/stderr to terminal

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPOSE_FILE="${REPO_ROOT}/docker-compose.ci.yml"
VERBOSE="${FORGEISO_CI_VERBOSE:-0}"
TOTAL_CPUS="${FORGEISO_CI_TOTAL_CPUS:-$(nproc)}"
CI_CACHE_ROOT="${CI_CACHE_ROOT:-/tmp/ci-cache}"
CACHE_DIR="${CI_CACHE_ROOT}/forgeiso"

# Stage → Dockerfile (for cache validity check)
declare -A _STAGE_DF=(
    [c1]="C1.rust.Dockerfile"
    [c2]="C2.sbom.Dockerfile"
    [c3]="C3.gui.Dockerfile"
    [c4]="C4.security.Dockerfile"
    [c5]="C5.integration.Dockerfile"
    [c6]="C6.e2e.Dockerfile"
    [c7]="C7.lint.Dockerfile"
)

declare -A CPU_WEIGHT=(
    [c1]=5
    [c2]=1
    [c3]=3
    [c4]=1
    [c5]=4
    [c6]=3
    [c7]=1
)

# ── Colour helpers ────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'
ok()   { echo -e "${GREEN}${BOLD}  PASS${RESET}  $*"; }
fail() { echo -e "${RED}${BOLD}  FAIL${RESET}  $*"; }
info() { echo -e "${CYAN}▶${RESET} $*"; }

# ── Stage list ────────────────────────────────────────────────────────────────
STAGES=(c1 c2 c3 c4 c5 c6 c7)
if [[ -n "${CI_STAGES:-}" ]]; then
    IFS=',' read -ra STAGES <<< "${CI_STAGES}"
fi
if [[ $# -gt 0 ]]; then
    STAGES=("$@")
fi

rebalance_cpus() {
    local running=()
    local total_weight=0

    for stage in "${STAGES[@]}"; do
        local cid="${CONTAINERS[$stage]:-}"
        [[ -n "${cid}" ]] || continue
        if docker inspect -f '{{.State.Running}}' "${cid}" 2>/dev/null | grep -qx true; then
            running+=("${stage}")
            total_weight=$((total_weight + ${CPU_WEIGHT[$stage]:-1}))
        fi
    done

    if [[ ${#running[@]} -eq 0 || ${total_weight} -eq 0 ]]; then
        return
    fi

    info "Rebalancing CPU budget across ${#running[@]} running stage(s) (total ${TOTAL_CPUS} cores)…"
    for stage in "${running[@]}"; do
        local cid="${CONTAINERS[$stage]}"
        local weight="${CPU_WEIGHT[$stage]:-1}"
        local cpus
        local shares

        cpus="$(awk -v total="${TOTAL_CPUS}" -v weight="${weight}" -v sum="${total_weight}" \
            'BEGIN { printf "%.2f", (total * weight) / sum }')"
        shares="$(awk -v total="${TOTAL_CPUS}" -v weight="${weight}" -v sum="${total_weight}" \
            'BEGIN { printf "%d", int((1024 * total * weight) / sum) }')"

        docker update --cpus "${cpus}" --cpu-shares "${shares}" "${cid}" >/dev/null
        echo "  ${stage}: cpus=${cpus} cpu-shares=${shares}"
    done
}

# ── Cleanup ───────────────────────────────────────────────────────────────────
cleanup() {
    info "Tearing down ephemeral containers and volumes…"
    docker compose -f "${COMPOSE_FILE}" down -v --remove-orphans 2>/dev/null || true
}
trap cleanup EXIT

# ── Step 1: Load from /tmp cache; build only what's missing ──────────────────
# Cache layout: /tmp/ci-cache/forgeiso/<stage>.tar + <stage>.sha
# One tar per stage (no hash-versioned multi-file pile-up).
# Writes are atomic (tmp → mv) so readers never see a partial file.
mkdir -p "${CACHE_DIR}"

_cache_valid() {
    local stage="$1"
    local df_name="${_STAGE_DF[$stage]:-}"
    [[ -n "${df_name}" ]] || return 1
    local df="${REPO_ROOT}/containers/${df_name}"
    local tar="${CACHE_DIR}/${stage}.tar"
    local sha="${CACHE_DIR}/${stage}.sha"
    [[ -f "${tar}" && -f "${sha}" ]] || return 1
    [[ "$(sha256sum "${df}" | awk '{print $1}')" == "$(cat "${sha}")" ]] || return 1
    return 0
}

info "Resolving CI images — cache: ${CACHE_DIR}"
declare -a _NEED_BUILD=()

for stage in "${STAGES[@]}"; do
    tar="${CACHE_DIR}/${stage}.tar"
    if _cache_valid "${stage}" && [[ -f "${tar}" ]]; then
        info "[${stage}] Loading from cache ($(du -sh "${tar}" | cut -f1))…"
        docker load -i "${tar}" >/dev/null
    else
        _NEED_BUILD+=("${stage}")
    fi
done

if [[ ${#_NEED_BUILD[@]} -gt 0 ]]; then
    info "Building ${#_NEED_BUILD[@]} uncached stage(s) in parallel: ${_NEED_BUILD[*]}…"
    docker compose -f "${COMPOSE_FILE}" build --parallel "${_NEED_BUILD[@]}"

    # Atomic save to cache: write to .tmp then mv — no partial-read window
    for stage in "${_NEED_BUILD[@]}"; do
        df_name="${_STAGE_DF[$stage]:-}"
        [[ -n "${df_name}" ]] || continue
        df="${REPO_ROOT}/containers/${df_name}"
        tar="${CACHE_DIR}/${stage}.tar"
        sha="${CACHE_DIR}/${stage}.sha"
        (
            docker save "forgeiso-${stage}" -o "${tar}.tmp" 2>/dev/null \
                && mv -f "${tar}.tmp" "${tar}" \
                && sha256sum "${df}" | awk '{print $1}' > "${sha}" \
                && echo "  [${stage}] Saved to cache ($(du -sh "${tar}" | cut -f1))"
        ) &
    done
    wait  # all saves complete before containers start
fi

# ── Step 2: Launch all containers simultaneously ──────────────────────────────
info "Launching ${#STAGES[@]} ephemeral containers in parallel with a ${TOTAL_CPUS}-core budget…"

declare -A PIDS       # stage → background PID
declare -A WAIT_PIDS  # stage → docker wait PID
declare -A CONTAINERS # stage → container id
declare -A LOG_FILES  # stage → temp log file
declare -A STATUS_FILES

for stage in "${STAGES[@]}"; do
    log="$(mktemp /tmp/forgeiso-ci-${stage}-XXXXXX.log)"
    LOG_FILES[$stage]="${log}"
    STATUS_FILES[$stage]="$(mktemp /tmp/forgeiso-ci-${stage}-status-XXXXXX)"

    cid="$(docker compose -f "${COMPOSE_FILE}" run -d --rm --no-deps "${stage}")"
    CONTAINERS[$stage]="${cid}"

    if [[ "${VERBOSE}" == "1" ]]; then
        docker logs -f "${cid}" 2>&1 | tee "${log}" &
    else
        docker logs -f "${cid}" >"${log}" 2>&1 &
    fi
    PIDS[$stage]=$!

    docker wait "${cid}" >"${STATUS_FILES[$stage]}" &
    WAIT_PIDS[$stage]=$!
    echo "  Started ${stage} (container ${cid})"
done

rebalance_cpus

# ── Step 3: Wait for every job, collect results ───────────────────────────────
echo ""
FAILED=()
PASSED=()
PENDING=("${STAGES[@]}")

while [[ ${#PENDING[@]} -gt 0 ]]; do
    wait -n "${WAIT_PIDS[@]}" || true

    next_pending=()
    for stage in "${PENDING[@]}"; do
        wait_pid="${WAIT_PIDS[$stage]}"
        if kill -0 "${wait_pid}" 2>/dev/null; then
            next_pending+=("${stage}")
            continue
        fi

        wait "${PIDS[$stage]}" || true
        status="$(tr -d '\r\n' < "${STATUS_FILES[$stage]}")"
        if [[ "${status}" == "0" ]]; then
            PASSED+=("$stage")
        else
            FAILED+=("$stage")
        fi
        rebalance_cpus
    done
    PENDING=("${next_pending[@]}")
done

# ── Step 4: Report ────────────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}══════════════════════════════════ CI Results ══════════════════════════════════${RESET}"
for stage in "${STAGES[@]}"; do
    label=""
    case "${stage}" in
        c1) label="C1 Rust (fmt / clippy / test)" ;;
        c2) label="C2 SBOM + Audit (cargo-deny / cargo-audit / syft)" ;;
        c3) label="C3 GUI (forge-gui + legacy Tauri build)" ;;
        c4) label="C4 Security (trivy / syft / grype)" ;;
        c5) label="C5 Integration (build + inject smoke)" ;;
        c6) label="C6 E2E Smoke (QEMU boot)" ;;
        c7) label="C7 Lint (fmt / clippy)" ;;
        *)  label="${stage}" ;;
    esac

    if printf '%s\n' "${PASSED[@]}" | grep -qx "${stage}"; then
        ok "${label}"
    else
        fail "${label}"
        echo "     Log: ${LOG_FILES[$stage]}"
        echo "     ── tail ──────────────────────────────────────────────────────"
        tail -20 "${LOG_FILES[$stage]}" | sed 's/^/     | /'
        echo "     ──────────────────────────────────────────────────────────────"
    fi
done
echo -e "${BOLD}════════════════════════════════════════════════════════════════════════════════${RESET}"

# ── Step 5: Cleanup temp logs ─────────────────────────────────────────────────
for stage in "${STAGES[@]}"; do
    [[ -f "${LOG_FILES[$stage]}" ]] && rm -f "${LOG_FILES[$stage]}"
    [[ -f "${STATUS_FILES[$stage]}" ]] && rm -f "${STATUS_FILES[$stage]}"
done

# ── Step 6: Exit code ─────────────────────────────────────────────────────────
if [[ ${#FAILED[@]} -gt 0 ]]; then
    echo ""
    echo -e "${RED}${BOLD}CI FAILED — ${#FAILED[@]} stage(s) failed: ${FAILED[*]}${RESET}"
    exit 1
fi

echo ""
echo -e "${GREEN}${BOLD}CI PASSED — all ${#PASSED[@]} stages passed.${RESET}"
exit 0
