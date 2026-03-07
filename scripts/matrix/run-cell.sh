#!/usr/bin/env bash
# run-cell.sh — Executed inside each matrix container.
# Reads MATRIX_DISTRO, MATRIX_VERSION, MATRIX_PROFILE, MATRIX_TIER from env.
set -euo pipefail

DISTRO="${MATRIX_DISTRO:-ubuntu}"
VERSION="${MATRIX_VERSION:-24.04}"
PROFILE="${MATRIX_PROFILE:-minimal}"
TIER="${MATRIX_TIER:-smoke}"
CELL="${DISTRO}-${VERSION}-${PROFILE}"

ARTIFACTS_DIR="/workspace/artifacts/matrix/${CELL}"
mkdir -p "$ARTIFACTS_DIR"

log() { echo "[matrix:${CELL}] $*"; }
fail() { echo "[matrix:${CELL}] FAIL: $*" >&2; exit 1; }

cd /workspace

offline_flag=()
if [[ "${CI:-false}" == "true" ]]; then
  : # online in CI — pull crates as needed
else
  offline_flag+=(--offline)
fi

# ── Build forgeiso-cli ─────────────────────────────────────────────────────────
log "Building CLI (profile=$PROFILE)..."
cargo build -p forgeiso-cli "${offline_flag[@]}" --release 2>&1 | tail -5
CLI="target/release/forgeiso"
[[ -x "$CLI" ]] || fail "CLI binary not found at $CLI"

# ── Doctor check ──────────────────────────────────────────────────────────────
log "Running doctor..."
"$CLI" doctor --json > "$ARTIFACTS_DIR/doctor.json" 2>&1 || true
log "Doctor complete."

# ── Smoke: build + inspect a synthetic ISO ────────────────────────────────────
if command -v grub-mkrescue >/dev/null 2>&1 || command -v grub2-mkrescue >/dev/null 2>&1; then
  log "Building smoke ISO..."
  SMOKE_DIR="$ARTIFACTS_DIR/smoke"
  mkdir -p "$SMOKE_DIR"

  eval "$(scripts/test/make-smoke-iso.sh "$SMOKE_DIR")"

  "$CLI" inspect --source "$ISO" --json > "$ARTIFACTS_DIR/inspect.json" 2>&1
  log "Inspect complete."

  "$CLI" build \
    --source "$ISO" \
    --out "$SMOKE_DIR/out" \
    --name "matrix-${CELL}" \
    --overlay "$OVERLAY" \
    --profile "$PROFILE" \
    --json > "$ARTIFACTS_DIR/build.json" 2>&1

  BUILT_ISO="$SMOKE_DIR/out/matrix-${CELL}.iso"
  [[ -f "$BUILT_ISO" ]] || fail "Expected built ISO at $BUILT_ISO"
  log "Build complete: $BUILT_ISO"

  # ISO-9660 header check (no xorriso needed)
  MAGIC=$(xxd -p -l 5 -s $((16 * 2048 + 1)) "$BUILT_ISO" 2>/dev/null || true)
  if [[ "$MAGIC" == "4344303031" ]]; then
    log "ISO-9660 header: OK (CD001)"
  else
    fail "ISO-9660 header missing in built ISO (got: $MAGIC)"
  fi

  # ── Full tier: QEMU boot test ────────────────────────────────────────────────
  if [[ "$TIER" == "full" ]]; then
    if command -v qemu-system-x86_64 >/dev/null 2>&1 && [[ -e /dev/kvm ]]; then
      log "Running QEMU BIOS boot test..."
      "$CLI" test --iso "$BUILT_ISO" --bios --json > "$ARTIFACTS_DIR/boot-test.json" 2>&1
      log "Boot test complete."
    else
      log "QEMU/KVM not available — skipping boot test."
    fi
  fi
else
  log "grub-mkrescue not available — skipping ISO build (doctor only)."
fi

log "Cell PASSED."
echo "PASS" > "$ARTIFACTS_DIR/result.txt"
