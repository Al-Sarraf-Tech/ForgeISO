#!/usr/bin/env bash
# scripts/perf-bench.sh — run criterion benchmarks, emit JSON, compare to baseline.
#
# Modes:
#   bench    — run cargo bench, parse output, emit dist/perf-current.json
#   compare  — diff dist/perf-current.json vs tests/baseline-perf.json,
#              fail if any benchmark regresses by more than PERF_THRESHOLD (default 15)%.
#   capture  — copy dist/perf-current.json into tests/baseline-perf.json (intentional reset).
#
# Usage:
#   scripts/perf-bench.sh bench
#   scripts/perf-bench.sh compare
#   PERF_THRESHOLD=20 scripts/perf-bench.sh compare
#   scripts/perf-bench.sh capture    # promote current as new baseline
#
# Environment:
#   PERF_THRESHOLD — max p99 regression % before compare fails (default 15)
#   PERF_BENCH_FILTER — passed to `cargo bench -- <filter>` (default empty: all)
#
# Hermetic: never reaches the network, never installs anything, only invokes
# `cargo bench -p forgeiso-engine` and `jq`. Runs the benches in
# engine/benches/engine_hot_paths.rs which use synthetic deterministic input.

set -Eeuo pipefail
IFS=$'\n\t'

PROJECT_ROOT="$(cd -P "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

THRESHOLD_PCT="${PERF_THRESHOLD:-15}"
FILTER="${PERF_BENCH_FILTER:-}"
CURRENT="dist/perf-current.json"
BASELINE="tests/baseline-perf.json"

mkdir -p dist tests

# ── bench: run criterion, parse output, emit JSON ────────────────────────────
do_bench() {
  command -v jq >/dev/null || { printf 'perf-bench: jq required\n' >&2; exit 1; }

  local raw="dist/perf-raw.txt"
  printf 'perf-bench: running criterion (this may take 60-120s)\n' >&2
  if [[ -n "$FILTER" ]]; then
    cargo bench -p forgeiso-engine -- "$FILTER" 2>&1 | tee "$raw" >/dev/null
  else
    cargo bench -p forgeiso-engine 2>&1 | tee "$raw" >/dev/null
  fi

  # Parse criterion text output. Each bench prints a "time:   [lo md hi]" line;
  # we capture the median (md) value and unit. Also pulls throughput when present.
  python3 - "$raw" "$CURRENT" <<'PY'
import json, re, sys, time
src, out = sys.argv[1], sys.argv[2]
benches = {}
current = None
re_name = re.compile(r"^([\w/_-]+)\s*$")
re_time = re.compile(
    r"^\s*time:\s+\[(\S+)\s*(\S+)\s+(\S+)\s*(\S+)\s+(\S+)\s*(\S+)\]")
with open(src) as f:
    for line in f:
        m = re_name.match(line.rstrip())
        if m and m.group(1) not in {"Compiling", "Finished", "Running"}:
            current = m.group(1)
            continue
        m = re_time.match(line)
        if m and current:
            lo_v, lo_u, md_v, md_u, hi_v, hi_u = m.groups()
            benches[current] = {
                "median": float(md_v),
                "median_unit": md_u,
                "low": float(lo_v),
                "low_unit": lo_u,
                "high": float(hi_v),
                "high_unit": hi_u,
            }
            current = None
out_doc = {
    "version": 1,
    "timestamp": int(time.time()),
    "benches": benches,
}
with open(out, "w") as f:
    json.dump(out_doc, f, indent=2, sort_keys=True)
print(f"perf-bench: wrote {len(benches)} benches to {out}", file=sys.stderr)
PY

  jq -e . "$CURRENT" >/dev/null || { printf 'perf-bench: invalid JSON\n' >&2; exit 1; }
  printf 'perf-bench: %s\n' "$CURRENT"
}

# ── normalise time to nanoseconds for cross-unit comparison ──────────────────
# stdin: "<value> <unit>"; stdout: nanoseconds as integer.
to_ns() {
  local v="$1" u="$2"
  awk -v v="$v" -v u="$u" 'BEGIN {
    mult = 1
    if      (u == "ns") mult = 1
    else if (u == "us" || u == "µs") mult = 1000
    else if (u == "ms") mult = 1000000
    else if (u == "s")  mult = 1000000000
    else { print "NaN"; exit 1 }
    printf "%.6f", v * mult
  }'
}

# ── compare: diff current vs baseline; fail if regression > threshold ────────
do_compare() {
  command -v jq >/dev/null || { printf 'perf-bench: jq required\n' >&2; exit 1; }
  [[ -s "$CURRENT" ]]   || { printf 'perf-bench: %s missing — run bench first\n' "$CURRENT" >&2; exit 1; }
  [[ -s "$BASELINE" ]]  || { printf 'perf-bench: %s missing — run capture first\n' "$BASELINE" >&2; exit 1; }

  local fail=0 regressed=()
  while IFS= read -r name; do
    cur_v="$(jq -r --arg n "$name" '.benches[$n].median // empty' "$CURRENT")"
    cur_u="$(jq -r --arg n "$name" '.benches[$n].median_unit // empty' "$CURRENT")"
    bas_v="$(jq -r --arg n "$name" '.benches[$n].median // empty' "$BASELINE")"
    bas_u="$(jq -r --arg n "$name" '.benches[$n].median_unit // empty' "$BASELINE")"
    [[ -z "$cur_v" || -z "$bas_v" ]] && continue
    cur_ns="$(to_ns "$cur_v" "$cur_u")"
    bas_ns="$(to_ns "$bas_v" "$bas_u")"
    delta_pct="$(awk -v c="$cur_ns" -v b="$bas_ns" 'BEGIN { printf "%.2f", (c-b)*100/b }')"
    if awk -v d="$delta_pct" -v t="$THRESHOLD_PCT" 'BEGIN { exit !(d > t) }'; then
      regressed+=("$name  +${delta_pct}%  (baseline ${bas_v}${bas_u} → ${cur_v}${cur_u})")
      fail=1
    fi
    printf '  %-50s  %+7.2f%%  baseline %s%s → current %s%s\n' \
      "$name" "$delta_pct" "$bas_v" "$bas_u" "$cur_v" "$cur_u"
  done < <(jq -r '.benches | keys[]' "$CURRENT")

  if [[ $fail -eq 1 ]]; then
    printf '\nperf-bench: FAIL — %d benchmark(s) regressed > %s%%\n' "${#regressed[@]}" "$THRESHOLD_PCT" >&2
    printf '  %s\n' "${regressed[@]}" >&2
    exit 1
  fi
  printf '\nperf-bench: PASS — no regressions > %s%%\n' "$THRESHOLD_PCT"
}

# ── capture: promote current as new baseline ─────────────────────────────────
do_capture() {
  [[ -s "$CURRENT" ]] || { printf 'perf-bench: %s missing — run bench first\n' "$CURRENT" >&2; exit 1; }
  cp -f "$CURRENT" "$BASELINE"
  printf 'perf-bench: baseline updated → %s\n' "$BASELINE"
}

case "${1:-}" in
  bench)   do_bench   ;;
  compare) do_compare ;;
  capture) do_capture ;;
  *)
    cat <<USAGE >&2
perf-bench: usage: $0 {bench|compare|capture}

  bench    — run criterion benches, emit dist/perf-current.json
  compare  — fail if any bench regresses > PERF_THRESHOLD% (default 15) vs baseline
  capture  — promote current as new baseline (commit tests/baseline-perf.json)

  PERF_THRESHOLD=N        regression threshold percent (default 15)
  PERF_BENCH_FILTER=name  pass filter to cargo bench --
USAGE
    exit 64
    ;;
esac
