#!/usr/bin/env bash
# ForgeISO image cache wrapper.
# Delegates to the universal ~/scripts/cache-image.sh for single-image operations,
# or runs scripts/ci/cache-images.sh to warm all 7 CI stages at once.
#
# Usage:
#   bash scripts/cache-image.sh                  # warm all 7 CI stages
#   bash scripts/cache-image.sh c1               # single stage
#   bash scripts/cache-image.sh c1 c3 c7         # selected stages
#
# All images stored in /tmp/ci-cache/forgeiso/ — one tar per stage.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MULTI="${REPO_ROOT}/scripts/ci/cache-images.sh"
UNIVERSAL="${HOME}/scripts/cache-image.sh"

# ── Multi-stage path (default) ────────────────────────────────────────────────
if [[ -x "${MULTI}" ]]; then
    exec "${MULTI}" "$@"
fi

# ── Fallback: single stage via universal script ───────────────────────────────
if [[ ! -x "${UNIVERSAL}" ]]; then
    echo "ERROR: universal cache-image.sh not found at ${UNIVERSAL}" >&2
    exit 1
fi

declare -A STAGE_DOCKERFILES=(
    [c1]="C1.rust.Dockerfile"
    [c2]="C2.sbom.Dockerfile"
    [c3]="C3.gui.Dockerfile"
    [c4]="C4.security.Dockerfile"
    [c5]="C5.integration.Dockerfile"
    [c6]="C6.e2e.Dockerfile"
    [c7]="C7.lint.Dockerfile"
)

if [[ $# -gt 0 ]]; then
    STAGES=("$@")
else
    STAGES=(c1 c2 c3 c4 c5 c6 c7)
fi
for stage in "${STAGES[@]}"; do
    df="${STAGE_DOCKERFILES[$stage]:-}"
    [[ -n "${df}" ]] || { echo "Unknown stage: ${stage}" >&2; exit 1; }
    "${UNIVERSAL}" \
        forgeiso \
        "forgeiso-${stage}" \
        "${REPO_ROOT}/containers/${df}" \
        "${REPO_ROOT}"
done
