#!/usr/bin/env bash
# C7 — Lint gate (fmt + clippy only, no test compilation)
# Fails fast on any format or clippy warning, without waiting for test builds.
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
mkdir -p "$ROOT_DIR/.cargo-tmp"
export TMPDIR="$ROOT_DIR/.cargo-tmp"

offline_flag=()
if [[ "${CI:-false}" != "true" ]]; then
  offline_flag+=(--offline)
fi

cargo fmt --all --check
cargo clippy --workspace --all-targets "${offline_flag[@]}" -- -D warnings
