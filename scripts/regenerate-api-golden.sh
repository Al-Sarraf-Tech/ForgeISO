#!/usr/bin/env bash
#
# regenerate-api-golden.sh — refresh engine/tests/public-api.golden.
#
# Run this when an intentional public-API change has been made and the
# api_contract test (engine/tests/api_contract.rs) is failing. After
# running it, write an ADR under docs/adr/ explaining the change and
# stage the new golden alongside the ADR in the same commit.
#
# Requires:
#   * cargo install cargo-public-api --locked
#   * rustup toolchain install nightly
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GOLDEN="$REPO_ROOT/engine/tests/public-api.golden"

if ! command -v cargo-public-api >/dev/null 2>&1; then
  echo "error: cargo-public-api not installed." >&2
  echo "install via: cargo install cargo-public-api --locked" >&2
  exit 2
fi

if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
  echo "error: nightly toolchain not installed (cargo-public-api needs rustdoc JSON)." >&2
  echo "install via: rustup toolchain install nightly" >&2
  exit 2
fi

cd "$REPO_ROOT"

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

echo "capturing public API of forgeiso-engine ..." >&2
cargo public-api -p forgeiso-engine >"$tmp"

if [[ ! -s "$tmp" ]]; then
  echo "error: cargo public-api produced empty output; aborting." >&2
  exit 3
fi

mv "$tmp" "$GOLDEN"
trap - EXIT

lines="$(wc -l <"$GOLDEN" | tr -d ' ')"
echo "wrote $GOLDEN ($lines lines)"
echo ""
echo "Next steps:"
echo "  1. Review the diff:    git diff -- $GOLDEN"
echo "  2. Add an ADR under:   docs/adr/NNNN-<title>.md"
echo "  3. Update index:       docs/adr/README.md"
echo "  4. Stage together:     git add $GOLDEN docs/adr/"
