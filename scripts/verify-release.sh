#!/usr/bin/env bash
# scripts/verify-release.sh — verify cosign signatures on release artifacts.
#
# For every .sig file under SIG_DIR, run `cosign verify-blob` against the
# original artifact in REL_DIR. Verification proves:
#   1. the artifact bytes are exactly what was signed (tamper-evident),
#   2. the signing identity matches our expected GitHub Actions OIDC subject,
#   3. the signature was logged in Rekor (the public Sigstore transparency log).
#
# This is the consumer-side counterpart to scripts/sign-release.sh. End users
# verifying a downloaded ForgeISO release should run this script (or the
# equivalent cosign command shown in docs/SECURITY.md).
#
# Note: this lives at scripts/verify-release.sh (cosign signatures) — distinct
# from scripts/release/verify-release.sh (release-asset structural checks).
#
# Usage:
#   scripts/verify-release.sh
#   REL_DIR=release-assets/ SIG_DIR=release-assets/signatures/ scripts/verify-release.sh
#
# Environment:
#   REL_DIR              — directory containing release artifacts (default: release-assets/)
#   SIG_DIR              — directory containing .sig + .pem (default: REL_DIR/signatures/)
#   COSIGN_BIN           — cosign executable (default: cosign on PATH)
#   COSIGN_IDENTITY_RE   — regex matching the expected signer identity
#                          (default: GitHub Actions OIDC subject for our repo)
#   COSIGN_ISSUER_RE     — regex matching the OIDC issuer
#                          (default: token.actions.githubusercontent.com)
#
# Exit codes:
#   0 — every signature verified successfully
#   1 — cosign missing, REL_DIR or SIG_DIR missing, or no .sig files
#   2 — at least one signature failed verification

set -Eeuo pipefail
IFS=$'\n\t'

PROJECT_ROOT="$(cd -P "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

REL_DIR="${REL_DIR:-release-assets/}"
SIG_DIR="${SIG_DIR:-${REL_DIR%/}/signatures/}"
COSIGN_BIN="${COSIGN_BIN:-cosign}"
# Default identity regexp matches the GitHub Actions workflow on the
# Al-Sarraf-Tech/ForgeISO repo. Override with COSIGN_IDENTITY_RE for a fork
# or a different release pipeline.
COSIGN_IDENTITY_RE="${COSIGN_IDENTITY_RE:-^https://github\\.com/Al-Sarraf-Tech/ForgeISO/\\.github/workflows/.+@refs/tags/v.+$}"
COSIGN_ISSUER_RE="${COSIGN_ISSUER_RE:-^https://token\\.actions\\.githubusercontent\\.com$}"

err() { printf '[verify-release] ERROR: %s\n' "$*" >&2; }
info() { printf '[verify-release] %s\n' "$*"; }

if ! command -v "$COSIGN_BIN" >/dev/null 2>&1; then
  err "cosign not found on PATH (set COSIGN_BIN to override)"
  err "install: https://docs.sigstore.dev/cosign/installation/"
  exit 1
fi

if [[ ! -d "$REL_DIR" ]]; then
  err "REL_DIR does not exist: $REL_DIR"
  exit 1
fi
if [[ ! -d "$SIG_DIR" ]]; then
  err "SIG_DIR does not exist: $SIG_DIR"
  exit 1
fi

declare -a SIGS=()
while IFS= read -r -d '' sig; do
  SIGS+=("$sig")
done < <(find "$SIG_DIR" -maxdepth 1 -type f -name '*.sig' -print0)

if [[ ${#SIGS[@]} -eq 0 ]]; then
  err "no .sig files found in $SIG_DIR (nothing to verify)"
  exit 1
fi

info "cosign: $("$COSIGN_BIN" version 2>&1 | head -1 || true)"
info "REL_DIR: $REL_DIR"
info "SIG_DIR: $SIG_DIR"
info "identity-regexp: $COSIGN_IDENTITY_RE"
info "issuer-regexp:   $COSIGN_ISSUER_RE"
info "signatures: ${#SIGS[@]}"

FAIL=0
PASS=0
for sig in "${SIGS[@]}"; do
  base="$(basename "$sig" .sig)"
  pem="${SIG_DIR%/}/${base}.pem"
  artifact="${REL_DIR%/}/${base}"

  if [[ ! -f "$artifact" ]]; then
    err "missing artifact for signature: $base (expected $artifact)"
    FAIL=$((FAIL + 1))
    continue
  fi
  if [[ ! -f "$pem" ]]; then
    err "missing certificate for signature: $base (expected $pem)"
    FAIL=$((FAIL + 1))
    continue
  fi

  info "verifying: $base"
  if "$COSIGN_BIN" verify-blob \
    --certificate "$pem" \
    --signature "$sig" \
    --certificate-identity-regexp "$COSIGN_IDENTITY_RE" \
    --certificate-oidc-issuer-regexp "$COSIGN_ISSUER_RE" \
    "$artifact" >/dev/null; then
    PASS=$((PASS + 1))
  else
    err "verification failed for: $base"
    FAIL=$((FAIL + 1))
  fi
done

info "verified: $PASS / ${#SIGS[@]}"
if [[ $FAIL -gt 0 ]]; then
  err "$FAIL signatures failed verification"
  exit 2
fi
info "all signatures verified"
