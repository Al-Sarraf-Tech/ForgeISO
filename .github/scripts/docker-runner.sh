#!/usr/bin/env bash
set -euo pipefail

##############################################################################
# docker-runner: Ephemeral Docker container executor with caching & cleanup
##############################################################################

CI_CACHE_ROOT="${CI_CACHE_ROOT:-/tmp/ci-cache}"
CACHE_DIR="${DOCKER_RUNNER_CACHE:-${CI_CACHE_ROOT}/forgeiso}"
IMAGE_CACHE_DIR="${CACHE_DIR}"
CONTAINER_ID="docker-runner-$$-$(date +%s%N | tail -c 7)"

# Ensure cache directory exists
mkdir -p "${IMAGE_CACHE_DIR}"

# Cleanup on exit (always)
cleanup() {
    local exit_code=$?
    [ $exit_code -ne 0 ] && echo "Error: docker-runner exited with code $exit_code" >&2
    docker rm -f "${CONTAINER_ID}" >/dev/null 2>&1 || true
    return $exit_code
}
trap cleanup EXIT

# Parse arguments
if [[ $# -lt 2 ]]; then
    cat >&2 <<EOF
Usage: docker-runner <image> <command> [args...]

Examples:
  docker-runner rust:1.93-bookworm "cargo test --workspace"
  docker-runner python:3.12-slim "python -m pytest"
  docker-runner debian:bookworm-slim "shellcheck script.sh"

Environment variables:
  DOCKER_RUNNER_CACHE  Cache directory (default: /tmp/docker-runner)
  DOCKER_RUNNER_PRUNE  Prune old images after running (default: false)

Features:
  - Ephemeral containers (auto-removed on exit)
  - Image layer caching at ${CACHE_DIR}
  - Automatic cleanup on success/failure
  - Optional image pruning
EOF
    exit 1
fi

IMAGE="$1"
COMMAND="$2"
shift 2
ARGS=("$@")

# Check /tmp cache for a saved tar before pulling from the registry.
# Cache tars are named <image-slug>-<hash>.tar and produced by cache-images.sh.
# If the daemon already has the image (e.g. loaded by run-parallel.sh) this
# block is skipped entirely via `docker image inspect`.
ensure_image() {
    local img="$1"
    # 1. Already present in daemon — nothing to do.
    if docker image inspect "${img}" >/dev/null 2>&1; then
        echo ">>> Image already in daemon: ${img}" >&2
        return 0
    fi

    # 2. Look for a single tar in the CI cache directory.
    # Naming: /tmp/ci-cache/forgeiso/<stage>.tar  (one file per stage, no hash suffix).
    local slug
    slug="$(echo "${img}" | tr '/:' '--')"
    # Try exact slug match, then strip project prefix (forgeiso-c1 → c1)
    local stripped="${slug##forgeiso-}"
    for tar in \
        "${IMAGE_CACHE_DIR}/${slug}.tar" \
        "${IMAGE_CACHE_DIR}/${stripped}.tar"; do
        if [[ -f "${tar}" ]]; then
            echo ">>> Loading from cache: ${tar}" >&2
            docker load -i "${tar}" >/dev/null
            return 0
        fi
    done

    # 3. Fall back to registry pull (with retry).
    echo ">>> Pulling image: ${img}" >&2
    for attempt in {1..3}; do
        if docker pull "${img}"; then
            return 0
        elif [[ $attempt -lt 3 ]]; then
            echo ">>> Retry $attempt/3 — pull failed, retrying…" >&2
            sleep 2
        else
            echo "Error: Failed to pull image after 3 attempts" >&2
            return 1
        fi
    done
}

ensure_image "${IMAGE}"

# Run ephemeral container
echo ">>> Running ephemeral container: ${CONTAINER_ID}" >&2
docker run \
    --rm \
    --name "${CONTAINER_ID}" \
    -v "$(pwd):/workspace" \
    -w "/workspace" \
    -e "CI=${CI:-false}" \
    -e "GITHUB_RUN_ID=${GITHUB_RUN_ID:-}" \
    -e "GITHUB_RUN_ATTEMPT=${GITHUB_RUN_ATTEMPT:-}" \
    "${IMAGE}" \
    /bin/sh -c "${COMMAND} ${ARGS[*]}"

# Optional: Prune old images if requested
if [[ "${DOCKER_RUNNER_PRUNE:-false}" == "true" ]]; then
    echo ">>> Pruning unused Docker images..." >&2
    docker image prune -a --force --filter "until=72h" || true
fi

echo ">>> ✓ Container exited successfully" >&2
