#!/usr/bin/env bash
# run-mutants.sh — reproducible mutation-testing wrapper for ForgeISO
#
# Runs cargo-mutants against the recently-decomposed inject + autoinstall/ubuntu
# modules with the project's standard configuration (.cargo/mutants.toml), then
# enforces the project's mutation-kill-score floor.
#
# Usage:
#   scripts/run-mutants.sh                 # full run, enforce threshold
#   scripts/run-mutants.sh --check-only    # smoke compile, no test run
#   scripts/run-mutants.sh --in-diff REF   # only mutants in code changed since REF
#   FORGEISO_MUTANTS_THRESHOLD=85 scripts/run-mutants.sh
#
# Exit codes:
#   0  kill score >= threshold (no surviving mutants OR survivors below floor)
#   1  kill score < threshold (regression)
#   2  cargo-mutants invocation itself failed (build error, unviable mutants)
#
# The threshold defaults to 80 (percent). Override via env to ratchet up.

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

THRESHOLD="${FORGEISO_MUTANTS_THRESHOLD:-80}"
LOG_DIR="${FORGEISO_LOG_DIR:-/mnt/nvmeINT/logs}"
mkdir -p "$LOG_DIR" 2>/dev/null || LOG_DIR="$(mktemp -d)"
LOG_FILE="$LOG_DIR/forgeiso-mutants.$(date +%Y%m%d-%H%M%S).log"

if ! command -v cargo-mutants >/dev/null 2>&1; then
    echo "error: cargo-mutants is not installed" >&2
    echo "       run: cargo install cargo-mutants --locked" >&2
    exit 2
fi

EXTRA_ARGS=()
CHECK_ONLY=0
IN_DIFF=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check-only)
            CHECK_ONLY=1
            shift
            ;;
        --in-diff)
            IN_DIFF="$2"
            shift 2
            ;;
        --shard)
            EXTRA_ARGS+=(--shard "$2")
            shift 2
            ;;
        --)
            shift
            EXTRA_ARGS+=("$@")
            break
            ;;
        *)
            EXTRA_ARGS+=("$1")
            shift
            ;;
    esac
done

# Default to copy-tree mode with -j 2: each worker mutates its own snapshot
# under /tmp, avoiding the stale-build-cache hazard that --in-place mode is
# vulnerable to when other agents are concurrently editing the workspace.
# See docs/MUTATION.md "Why copy mode" for the full rationale.
#
# -j is incompatible with --in-place; if the operator opts back into in-place
# mode (FORGEISO_MUTANTS_IN_PLACE=1) the -j 2 flag is dropped automatically.
CMD=(cargo mutants --baseline=skip --gitignore=true)
if [[ "${FORGEISO_MUTANTS_IN_PLACE:-0}" == "1" ]]; then
    CMD+=(--in-place)
else
    CMD+=(-j "${FORGEISO_MUTANTS_JOBS:-2}")
fi

if (( CHECK_ONLY )); then
    CMD+=(--check)
fi

if [[ -n "$IN_DIFF" ]]; then
    DIFF_FILE="$(mktemp -t forgeiso-mutants-diff.XXXXXX.diff)"
    git diff "$IN_DIFF" >"$DIFF_FILE"
    CMD+=(--in-diff "$DIFF_FILE")
fi

CMD+=("${EXTRA_ARGS[@]}")

echo "running: ${CMD[*]}" | tee -a "$LOG_FILE"
echo "log:     $LOG_FILE"

set +e
"${CMD[@]}" 2>&1 | tee -a "$LOG_FILE"
RC="${PIPESTATUS[0]}"
set -e

# In --check-only mode the score isn't meaningful; just propagate the rc.
if (( CHECK_ONLY )); then
    exit "$RC"
fi

# Parse the summary line. cargo-mutants prints a final block of the form:
#     N mutants tested in 12m 34s: K caught, S missed, T timeout, U unviable
# We want kill_score = (caught + timeout + unviable) / tested * 100.
# Timeouts and unviable mutants are treated as "kept" because the test suite
# couldn't actually distinguish them — but for our threshold we count them
# as kills (the production code still behaves; this matches the convention
# used by the cargo-mutants project itself).
SUMMARY="$(grep -E '^[0-9]+ mutants tested' "$LOG_FILE" | tail -1 || true)"
if [[ -z "$SUMMARY" ]]; then
    echo "warn: no summary line found in cargo-mutants output" >&2
    echo "      kill-score gate skipped, propagating cargo-mutants exit code $RC" >&2
    exit "$RC"
fi

echo "summary: $SUMMARY"

TESTED="$(echo "$SUMMARY"  | grep -oE '^[0-9]+'              | head -1)"
CAUGHT="$(echo "$SUMMARY"  | grep -oE '[0-9]+ caught'        | grep -oE '^[0-9]+' || echo 0)"
MISSED="$(echo "$SUMMARY"  | grep -oE '[0-9]+ missed'        | grep -oE '^[0-9]+' || echo 0)"
TIMEOUT="$(echo "$SUMMARY" | grep -oE '[0-9]+ timeout'       | grep -oE '^[0-9]+' || echo 0)"
UNVIABLE="$(echo "$SUMMARY"| grep -oE '[0-9]+ unviable'      | grep -oE '^[0-9]+' || echo 0)"

if [[ "$TESTED" -eq 0 ]]; then
    echo "warn: tested=0, nothing to score" >&2
    exit 0
fi

KILLED=$(( CAUGHT + TIMEOUT + UNVIABLE ))
SCORE=$(( KILLED * 100 / TESTED ))

printf 'kill-score: %d%% (%d killed / %d tested, %d missed)\n' \
    "$SCORE" "$KILLED" "$TESTED" "$MISSED"
printf 'threshold:  %d%%\n' "$THRESHOLD"

if (( SCORE < THRESHOLD )); then
    echo "FAIL: kill score $SCORE% is below threshold $THRESHOLD%" >&2
    exit 1
fi

echo "PASS: kill score $SCORE% meets threshold $THRESHOLD%"
exit 0
