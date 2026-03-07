#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "$ROOT_DIR/.cargo-tmp"
export TMPDIR="$ROOT_DIR/.cargo-tmp"
cd "$ROOT_DIR/gui"

if [[ "${CI:-false}" == "true" ]]; then
  # In CI the node_modules volume is always a fresh named volume (empty mount
  # point); the -d check passes even when empty, so always install here.
  npm ci
elif [[ ! -f node_modules/.bin/tsc ]]; then
  npm ci --offline
fi
npm run lint
npm run build

cd "$ROOT_DIR/gui/src-tauri"
cargo check
