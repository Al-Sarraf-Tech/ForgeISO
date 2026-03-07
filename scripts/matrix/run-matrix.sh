#!/usr/bin/env bash
# run-matrix.sh — Runs all matrix cells in parallel ephemeral containers.
# Usage:
#   scripts/matrix/run-matrix.sh [--tier smoke|full] [--cell <name>] [--clean]
#
# --tier smoke   (default) Fast path: doctor + build + ISO-9660 check
# --tier full    Full path: smoke + QEMU boot test if KVM is available
# --cell NAME    Run only one specific cell (e.g. ubuntu-2404-minimal)
# --clean        Destroy all matrix volumes and exit
set -euo pipefail

COMPOSE_FILE="docker-compose.matrix.yml"
TIER="smoke"
CELL=""
CLEAN=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tier)  TIER="$2"; shift 2 ;;
    --cell)  CELL="$2"; shift 2 ;;
    --clean) CLEAN=true; shift ;;
    *) echo "Unknown flag: $1" >&2; exit 1 ;;
  esac
done

if $CLEAN; then
  echo "==> Cleaning matrix volumes..."
  docker compose -f "$COMPOSE_FILE" down -v --remove-orphans 2>/dev/null || true
  docker images --format '{{.Repository}}:{{.Tag}}' | grep '^forgeiso-matrix' | \
    xargs -r docker rmi 2>/dev/null || true
  echo "==> Matrix volumes and images removed."
  exit 0
fi

export MATRIX_TIER="$TIER"

echo "==> ForgeISO distro×version×profile matrix [tier=$TIER]"
echo ""

# Build all images first (parallel)
BUILD_ARGS=(docker compose -f "$COMPOSE_FILE" build --parallel)
if [[ -n "$CELL" ]]; then
  BUILD_ARGS+=("$CELL")
fi
echo "==> Building matrix images..."
"${BUILD_ARGS[@]}"

# Run cells
RUN_ARGS=(docker compose -f "$COMPOSE_FILE" up --abort-on-container-exit --remove-orphans)
if [[ -n "$CELL" ]]; then
  RUN_ARGS+=("$CELL")
fi

echo ""
echo "==> Running matrix cells..."
"${RUN_ARGS[@]}"
EXIT=$?

# Collect results
echo ""
echo "==> Matrix results:"
PASS=0; FAIL=0
for f in artifacts/matrix/*/result.txt; do
  cell=$(basename "$(dirname "$f")")
  result=$(cat "$f" 2>/dev/null || echo "NO_RESULT")
  if [[ "$result" == "PASS" ]]; then
    echo "  PASS  $cell"
    ((PASS++))
  else
    echo "  FAIL  $cell"
    ((FAIL++))
  fi
done

echo ""
echo "==> $PASS passed, $FAIL failed."

# Cleanup ephemeral volumes
docker compose -f "$COMPOSE_FILE" down -v --remove-orphans 2>/dev/null || true

if [[ $FAIL -gt 0 || $EXIT -ne 0 ]]; then
  echo "==> Matrix FAILED." >&2
  exit 1
fi

echo "==> Matrix PASSED."
