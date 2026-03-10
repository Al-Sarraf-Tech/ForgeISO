#!/usr/bin/env bash
# ForgeISO parallel CI runner
#
# Builds all 6 CI images in parallel, then launches every stage simultaneously
# in its own ephemeral container with an isolated Cargo target volume.
# Waits for every job to finish, reports pass/fail for each, then tears down
# ALL volumes and containers.  Exits 0 only when every stage passes.
#
# Usage:
#   bash scripts/ci/run-parallel.sh           # all 6 stages
#   bash scripts/ci/run-parallel.sh c1 c3     # selected stages only
#
# Environment:
#   CI_STAGES=c1,c2,c3,c4,c5,c6,c7   override which stages to run (comma-separated)
#   FORGEISO_CI_VERBOSE=1          stream container stdout/stderr to terminal

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPOSE_FILE="${REPO_ROOT}/docker-compose.ci.yml"
VERBOSE="${FORGEISO_CI_VERBOSE:-0}"

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

# ── Cleanup ───────────────────────────────────────────────────────────────────
cleanup() {
    info "Tearing down ephemeral containers and volumes…"
    docker compose -f "${COMPOSE_FILE}" down -v --remove-orphans 2>/dev/null || true
}
trap cleanup EXIT

# ── Step 1: Build all images in parallel ─────────────────────────────────────
info "Building CI images in parallel (${STAGES[*]})…"
docker compose -f "${COMPOSE_FILE}" build --parallel "${STAGES[@]}"

# ── Step 2: Launch all containers simultaneously ──────────────────────────────
info "Launching ${#STAGES[@]} ephemeral containers in parallel…"

declare -A PIDS       # stage → background PID
declare -A LOG_FILES  # stage → temp log file

for stage in "${STAGES[@]}"; do
    log="$(mktemp /tmp/forgeiso-ci-${stage}-XXXXXX.log)"
    LOG_FILES[$stage]="${log}"

    if [[ "${VERBOSE}" == "1" ]]; then
        docker compose -f "${COMPOSE_FILE}" run --rm --no-deps "${stage}" \
            2>&1 | tee "${log}" &
    else
        docker compose -f "${COMPOSE_FILE}" run --rm --no-deps "${stage}" \
            >"${log}" 2>&1 &
    fi
    PIDS[$stage]=$!
    echo "  Started ${stage} (PID ${PIDS[$stage]})"
done

# ── Step 3: Wait for every job, collect results ───────────────────────────────
echo ""
FAILED=()
PASSED=()

for stage in "${STAGES[@]}"; do
    if wait "${PIDS[$stage]}"; then
        PASSED+=("$stage")
    else
        FAILED+=("$stage")
    fi
done

# ── Step 4: Report ────────────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}══════════════════════════════════ CI Results ══════════════════════════════════${RESET}"
for stage in "${STAGES[@]}"; do
    label=""
    case "${stage}" in
        c1) label="C1 Rust (fmt / clippy / test)" ;;
        c2) label="C2 SBOM + Audit (cargo-deny / cargo-audit / syft)" ;;
        c3) label="C3 GUI (tsc / vite / cargo check)" ;;
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
