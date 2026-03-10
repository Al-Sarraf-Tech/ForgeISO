#!/usr/bin/env bash
# C3: desktop GUI build checks — forge-gui plus legacy Tauri/React gui/
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "$ROOT_DIR/.cargo-tmp"
export TMPDIR="$ROOT_DIR/.cargo-tmp"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-18}"
cd "$ROOT_DIR"

echo "▶ [C3] fmt check..."
cargo fmt --manifest-path forge-gui/Cargo.toml --all --check

echo "▶ [C3] clippy..."
cargo clippy -p forge-gui --all-targets -j "${CARGO_BUILD_JOBS}" -- -D warnings

echo "▶ [C3] build (dev)..."
cargo build -p forge-gui -j "${CARGO_BUILD_JOBS}"

echo "▶ [C3] state/worker compile check..."
cargo check -p forge-gui -j "${CARGO_BUILD_JOBS}"

echo "▶ [C3] legacy GUI npm ci..."
npm ci --prefix gui --ignore-scripts

echo "▶ [C3] legacy GUI lint..."
npm run --prefix gui lint

echo "▶ [C3] legacy GUI build..."
npm run --prefix gui build

echo "▶ [C3] legacy Tauri cargo check..."
cargo check --manifest-path gui/src-tauri/Cargo.toml -j "${CARGO_BUILD_JOBS}"

echo "▶ [C3] OK — desktop GUIs build cleanly"
