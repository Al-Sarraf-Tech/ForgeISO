#!/usr/bin/env bash
# scripts/sign-release.sh — sign release artifacts with cosign keyless OIDC.
#
# Generates a detached Sigstore signature + signing certificate for every
# release artifact and SBOM in REL_DIR. Signatures and certificates are
# written to SIG_DIR. The keyless OIDC flow makes a transparency-log entry
# in Rekor, which is the chain of custody an end user verifies against.
#
# This is the desktop-tool equivalent of SLSA L3+ build provenance:
# CLAUDE.md absolute-rule bans `actions/attest-build-provenance`, but the
# underlying need (provenance + tamper-evidence + verifiable signatures)
# is met by cosign sign-blob + Sigstore transparency log entries. See
# docs/SECURITY.md and docs/adr/0010-security-contract-desktop-tool.md.
#
# Modes:
#   default — sign every binary, checksums file, and SBOM in REL_DIR
#
# Usage:
#   scripts/sign-release.sh
#   REL_DIR=release-assets/ SIG_DIR=release-assets/signatures/ scripts/sign-release.sh
#
# Environment:
#   REL_DIR        — directory containing release artifacts (default: release-assets/)
#   SIG_DIR        — directory to write .sig + .pem files (default: REL_DIR/signatures/)
#   COSIGN_BIN     — cosign executable (default: cosign on PATH)
#   COSIGN_EXTRA   — extra flags forwarded to `cosign sign-blob` (default: empty)
#
# Requirements:
#   - cosign >=2.0 on PATH (or COSIGN_BIN)
#   - In CI: a workload identity token (GitHub OIDC handles this automatically;
#     `id-token: write` permission on the workflow job is required).
#   - Locally: a browser to complete the OIDC login flow (one-shot).
#
# Exit codes:
#   0 — every artifact signed successfully
#   1 — cosign missing or REL_DIR missing
#   2 — cosign sign-blob failed for at least one artifact
#
# Hermetic: only network egress is to Sigstore Fulcio (cert issuance) and
# Rekor (transparency log append). No code is downloaded.

set -Eeuo pipefail
IFS=$'\n\t'

PROJECT_ROOT="$(cd -P "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

REL_DIR="${REL_DIR:-release-assets/}"
SIG_DIR="${SIG_DIR:-${REL_DIR%/}/signatures/}"
COSIGN_BIN="${COSIGN_BIN:-cosign}"
COSIGN_EXTRA="${COSIGN_EXTRA:-}"

err() { printf '[sign-release] ERROR: %s\n' "$*" >&2; }
info() { printf '[sign-release] %s\n' "$*"; }

if ! command -v "$COSIGN_BIN" >/dev/null 2>&1; then
  err "cosign not found on PATH (set COSIGN_BIN to override)"
  err "install: https://docs.sigstore.dev/cosign/installation/"
  exit 1
fi

if [[ ! -d "$REL_DIR" ]]; then
  err "REL_DIR does not exist: $REL_DIR"
  exit 1
fi

mkdir -p "$SIG_DIR"

# Build the artifact list. Sign:
#   - any regular file directly under REL_DIR (binaries, packages)
#   - the checksums file (so checksum verification chains to a Sigstore identity)
#   - any SBOM (sbom*.json, *.spdx.json, *.cyclonedx.json) at REL_DIR root
# Skip the SIG_DIR itself and any pre-existing .sig / .pem to keep this
# script idempotent (re-running won't sign signatures of signatures).
declare -a ARTIFACTS=()
while IFS= read -r -d '' file; do
  base="$(basename "$file")"
  case "$base" in
    *.sig | *.pem | *.cert) continue ;;
  esac
  ARTIFACTS+=("$file")
done < <(find "$REL_DIR" -maxdepth 1 -type f -print0)

if [[ ${#ARTIFACTS[@]} -eq 0 ]]; then
  err "no artifacts found in $REL_DIR (nothing to sign)"
  exit 1
fi

info "cosign: $("$COSIGN_BIN" version 2>&1 | head -1 || true)"
info "REL_DIR: $REL_DIR"
info "SIG_DIR: $SIG_DIR"
info "artifacts: ${#ARTIFACTS[@]}"

FAIL=0
for artifact in "${ARTIFACTS[@]}"; do
  name="$(basename "$artifact")"
  sig_path="${SIG_DIR%/}/${name}.sig"
  pem_path="${SIG_DIR%/}/${name}.pem"

  info "signing: $name"
  # shellcheck disable=SC2086 # COSIGN_EXTRA intentionally word-split
  if ! "$COSIGN_BIN" sign-blob \
    --yes \
    --output-signature "$sig_path" \
    --output-certificate "$pem_path" \
    $COSIGN_EXTRA \
    "$artifact"; then
    err "cosign sign-blob failed for $name"
    FAIL=$((FAIL + 1))
    continue
  fi

  if [[ ! -s "$sig_path" || ! -s "$pem_path" ]]; then
    err "signature or certificate empty for $name"
    FAIL=$((FAIL + 1))
  fi
done

if [[ $FAIL -gt 0 ]]; then
  err "$FAIL of ${#ARTIFACTS[@]} artifacts failed to sign"
  exit 2
fi

info "all ${#ARTIFACTS[@]} artifacts signed successfully"
info "verify with: scripts/verify-release.sh"
