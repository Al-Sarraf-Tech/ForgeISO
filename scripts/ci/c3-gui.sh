#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR/gui"

if [[ ! -d node_modules ]]; then
  npm ci --offline
fi
npm run lint
npm run build

cd "$ROOT_DIR/gui/src-tauri"
cargo check --offline
