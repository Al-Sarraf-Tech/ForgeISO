#!/usr/bin/env bash
# scripts/s-tier-audit.sh — single-command S+ rubric audit for ForgeISO.
#
# Modeled on session-sync's audit script (~/git/session-sync/scripts/s-tier-audit.sh).
# Exits 0 on full pass, 1 on any failure. Run before tagging a release or
# claiming a tier upgrade.
#
# Checks (8 dimensions of ~/.claude/TIER_RUBRIC.md):
#   1. fmt          — cargo fmt --all --check
#   2. lint         — cargo clippy --workspace --all-targets -- -D warnings
#   3. test         — cargo test --workspace
#   4. coverage     — cargo tarpaulin --workspace (gate per tarpaulin.toml)
#   5. build        — cargo build --workspace --release
#   6. perf         — scripts/perf-bench.sh compare (skipped if baseline empty)
#   7. security     — cargo audit + cargo deny
#   8. docs         — verify RUNBOOKS, ADRs >= 5, METRICS, COMPLIANCE, CHANGELOG
#
# Usage: scripts/s-tier-audit.sh [--fast]
#
# --fast skips coverage, release build, and perf (slow steps); useful in
# pre-commit. Run the full audit before tagging.

set -Eeuo pipefail
IFS=$'\n\t'

FAST=0
[[ "${1:-}" == "--fast" ]] && FAST=1

PROJECT_ROOT="$(cd -P "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

start_ts=$(date +%s)
PASS=()
FAIL=()
SKIP=()

run() {
  local name="$1"; shift
  printf '\n=== %s ===\n' "$name"
  if "$@"; then
    PASS+=("$name")
  else
    FAIL+=("$name")
  fi
}

skip() { local name="$1" why="$2"; printf '\n=== %s ===\nSKIPPED (%s)\n' "$name" "$why"; SKIP+=("$name"); }

run "1/8 fmt"   cargo fmt --all --check
run "2/8 lint"  cargo clippy --workspace --all-targets -- -D warnings
run "3/8 test"  cargo test --workspace --quiet

if [[ $FAST -eq 0 ]]; then
  run "4/8 coverage" cargo tarpaulin --workspace
  run "5/8 build"    cargo build --workspace --release --quiet
else
  skip "4/8 coverage" "--fast"
  skip "5/8 build" "--fast"
fi

if [[ $FAST -eq 0 ]]; then
  if [[ -s tests/baseline-perf.json ]] \
      && [[ "$(jq -r '.benches | length' tests/baseline-perf.json 2>/dev/null || echo 0)" -gt 0 ]]; then
    if scripts/perf-bench.sh bench >/dev/null 2>&1 && scripts/perf-bench.sh compare; then
      PASS+=("6/8 perf")
    else
      FAIL+=("6/8 perf")
    fi
  else
    skip "6/8 perf" "baseline empty — run scripts/perf-bench.sh bench && capture"
  fi
else
  skip "6/8 perf" "--fast"
fi

# 7. Security — both must pass
printf '\n=== 7/8 security ===\n'
sec_ok=1
if cargo audit; then printf '  PASS  cargo audit\n'; else printf '  FAIL  cargo audit\n'; sec_ok=0; fi
if cargo deny check bans licenses sources 2>/dev/null; then
  printf '  PASS  cargo deny\n'
else
  printf '  FAIL  cargo deny\n'; sec_ok=0
fi
if [[ $sec_ok -eq 1 ]]; then PASS+=("7/8 security"); else FAIL+=("7/8 security"); fi

# 8. Docs presence
printf '\n=== 8/8 docs ===\n'
docs_ok=1
for f in docs/RUNBOOKS.md docs/COMPLIANCE.md docs/SLO.md docs/adr/README.md CLAUDE.md tarpaulin.toml CHANGELOG.md; do
  if [[ -s "$f" ]]; then
    printf '  PASS  %s\n' "$f"
  else
    printf '  FAIL  %s missing or empty\n' "$f"
    docs_ok=0
  fi
done
adr_count=$(find docs/adr -maxdepth 1 -name '[0-9]*.md' 2>/dev/null | wc -l)
if [[ $adr_count -ge 5 ]]; then
  printf '  PASS  %d ADRs in docs/adr/\n' "$adr_count"
else
  printf '  FAIL  only %d ADRs in docs/adr/ (need >= 5 for S+)\n' "$adr_count"
  docs_ok=0
fi
if [[ $docs_ok -eq 1 ]]; then PASS+=("8/8 docs"); else FAIL+=("8/8 docs"); fi

elapsed=$(( $(date +%s) - start_ts ))
printf '\n--- summary ---\n'
printf 'PASSED  (%d): %s\n' "${#PASS[@]}" "${PASS[*]}"
[[ ${#SKIP[@]} -gt 0 ]] && printf 'SKIPPED (%d): %s\n' "${#SKIP[@]}" "${SKIP[*]}"
if [[ ${#FAIL[@]} -gt 0 ]]; then
  printf 'FAILED  (%d): %s\n' "${#FAIL[@]}" "${FAIL[*]}"
  printf 'elapsed: %ds\n' "$elapsed"
  exit 1
fi
printf 'elapsed: %ds\n' "$elapsed"
printf 'audit: PASS\n'
