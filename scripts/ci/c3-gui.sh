#!/usr/bin/env bash
# C3: desktop GUI build checks — forge-slint (Slint UI)
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "$ROOT_DIR/.cargo-tmp"
export TMPDIR="$ROOT_DIR/.cargo-tmp"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-18}"
cd "$ROOT_DIR"

echo "▶ [C3] fmt check..."
cargo fmt --manifest-path forge-slint/Cargo.toml --all --check

echo "▶ [C3] clippy..."
cargo clippy -p forge-slint --all-targets -j "${CARGO_BUILD_JOBS}" -- -D warnings

echo "▶ [C3] build (dev)..."
cargo build -p forge-slint -j "${CARGO_BUILD_JOBS}"

echo "▶ [C3] state/worker compile check..."
cargo check -p forge-slint -j "${CARGO_BUILD_JOBS}"

echo "▶ [C3] OK — desktop GUI builds cleanly"
