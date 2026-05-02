#!/usr/bin/env bash
# scripts/cleanup-artifacts.sh — remove generated artifacts from the ForgeISO tree.
#
# Modes:
#   (default)     remove dist/, mutants.out/, mutants-fullrun.out/,
#                 target/criterion/, target/tarpaulin/, tests/test-builds/* (keep .gitignore),
#                 forgeiso-cache/, forgeiso-regtest/. Preserves the rest of target/
#                 so cargo incremental builds remain hot.
#   --aggressive  default + ~/.cache/forgeiso/ (engine source-ISO cache; tens of GB).
#                 Source ISOs will be re-downloaded on the next build.
#   --dry-run     list what WOULD be removed and how much space would be freed; do not delete.
#                 Combine with --aggressive to preview an aggressive clean.
#   -h | --help   show usage.
#
# Exit codes:
#   0  cleanup completed (also when there is nothing to do, by design — idempotent).
#   2  invalid usage.
#
# Notes:
#   * target/ itself is intentionally left alone. Users want incremental cargo builds.
#     We only prune target/criterion and target/tarpaulin (regenerable reports).
#   * tests/test-builds/ is the documented output dir for Phase 9a's test-releases.sh
#     (does not exist in the tree yet; cleanup is a no-op until it does).
#   * mutants-fullrun.out/ is the output dir for the full-run mutation profile
#     (referenced by docs/MUTATION.md).
#   * Each removed path logs item-count + freed bytes. Totals print at the end.

set -Eeuo pipefail
IFS=$'\n\t'

# ── Resolve repo root from script location, never CWD ────────────────────────
SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -P "${SCRIPT_DIR}/.." && pwd)"

# ── Argument parsing ─────────────────────────────────────────────────────────
MODE="default"
DRY_RUN=0

usage() {
    cat <<'EOF'
Usage: scripts/cleanup-artifacts.sh [--aggressive] [--dry-run] [-h|--help]

Modes:
  (default)     dist/, mutants.out/, mutants-fullrun.out/, target/criterion/,
                target/tarpaulin/, tests/test-builds/*, forgeiso-cache/, forgeiso-regtest/
  --aggressive  default + ~/.cache/forgeiso/ (drops cached source ISOs)
  --dry-run     list what would be removed; combine with --aggressive

Examples:
  scripts/cleanup-artifacts.sh
  scripts/cleanup-artifacts.sh --dry-run
  scripts/cleanup-artifacts.sh --aggressive --dry-run
  scripts/cleanup-artifacts.sh --aggressive
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --aggressive)
            MODE="aggressive"
            shift
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf 'cleanup-artifacts: unknown argument: %s\n\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

# ── Globals updated by remove_path ───────────────────────────────────────────
TOTAL_ITEMS=0
TOTAL_BYTES=0
TOUCHED=0

# ── Helpers ──────────────────────────────────────────────────────────────────

# Print bytes as a human-readable size (MB/GB), one decimal place.
human_size() {
    local bytes="$1"
    if (( bytes >= 1073741824 )); then
        awk -v b="$bytes" 'BEGIN { printf "%.1f GB", b / 1073741824 }'
    elif (( bytes >= 1048576 )); then
        awk -v b="$bytes" 'BEGIN { printf "%.1f MB", b / 1048576 }'
    elif (( bytes >= 1024 )); then
        awk -v b="$bytes" 'BEGIN { printf "%.1f KB", b / 1024 }'
    else
        printf '%d B' "$bytes"
    fi
}

# Disk usage in bytes for a path. Returns 0 if path missing.
path_bytes() {
    local p="$1"
    if [[ -e "$p" || -L "$p" ]]; then
        du -sb --apparent-size "$p" 2>/dev/null | awk '{print $1}'
    else
        printf '0'
    fi
}

# Count files + dirs that would be removed under a path. Returns 0 if missing.
path_items() {
    local p="$1"
    if [[ -d "$p" ]]; then
        # Include the dir itself in the count.
        find "$p" -mindepth 0 2>/dev/null | wc -l
    elif [[ -e "$p" || -L "$p" ]]; then
        printf '1'
    else
        printf '0'
    fi
}

# remove_path LABEL TARGET
#   - If --dry-run: log "would remove" with size.
#   - Otherwise: rm -rf the target, log size + count freed.
#   - Idempotent: missing target is silently skipped.
remove_path() {
    local label="$1"
    local target="$2"

    if [[ ! -e "$target" && ! -L "$target" ]]; then
        return 0
    fi

    local bytes items
    bytes="$(path_bytes "$target")"
    items="$(path_items "$target")"

    TOTAL_ITEMS=$(( TOTAL_ITEMS + items ))
    TOTAL_BYTES=$(( TOTAL_BYTES + bytes ))
    TOUCHED=$(( TOUCHED + 1 ))

    if (( DRY_RUN )); then
        printf '  [dry-run] %s %s (%d items, %s)\n' \
            "$label" "$target" "$items" "$(human_size "$bytes")"
        return 0
    fi

    rm -rf -- "$target"
    printf '  removed %s %s (%d items, freed %s)\n' \
        "$label" "$target" "$items" "$(human_size "$bytes")"
}

# remove_glob LABEL DIR  — wipe contents of DIR while preserving DIR + .gitignore.
remove_glob() {
    local label="$1"
    local dir="$2"

    if [[ ! -d "$dir" ]]; then
        return 0
    fi

    # Collect entries (files + subdirs) excluding .gitignore.
    local entries=()
    local entry
    while IFS= read -r -d '' entry; do
        entries+=("$entry")
    done < <(find "$dir" -mindepth 1 -maxdepth 1 \
                ! -name '.gitignore' -print0 2>/dev/null)

    if (( ${#entries[@]} == 0 )); then
        return 0
    fi

    local total_bytes=0
    local total_items=0
    local b i
    for entry in "${entries[@]}"; do
        b="$(path_bytes "$entry")"
        i="$(path_items "$entry")"
        total_bytes=$(( total_bytes + b ))
        total_items=$(( total_items + i ))
    done

    TOTAL_ITEMS=$(( TOTAL_ITEMS + total_items ))
    TOTAL_BYTES=$(( TOTAL_BYTES + total_bytes ))
    TOUCHED=$(( TOUCHED + 1 ))

    if (( DRY_RUN )); then
        printf '  [dry-run] %s contents of %s (%d items, %s)\n' \
            "$label" "$dir" "$total_items" "$(human_size "$total_bytes")"
        return 0
    fi

    for entry in "${entries[@]}"; do
        rm -rf -- "$entry"
    done
    printf '  cleared %s contents of %s (%d items, freed %s)\n' \
        "$label" "$dir" "$total_items" "$(human_size "$total_bytes")"
}

# ── Banner ───────────────────────────────────────────────────────────────────
if (( DRY_RUN )); then
    printf 'cleanup-artifacts: DRY-RUN mode (%s) — repo: %s\n' "$MODE" "$REPO_ROOT"
else
    printf 'cleanup-artifacts: %s mode — repo: %s\n' "$MODE" "$REPO_ROOT"
fi

# ── Default removals (always run) ────────────────────────────────────────────
remove_path "dist"               "${REPO_ROOT}/dist"
remove_path "mutants.out"        "${REPO_ROOT}/mutants.out"
remove_path "mutants-fullrun.out" "${REPO_ROOT}/mutants-fullrun.out"
remove_path "target/criterion"   "${REPO_ROOT}/target/criterion"
remove_path "target/tarpaulin"   "${REPO_ROOT}/target/tarpaulin"
remove_path "forgeiso-cache"     "${REPO_ROOT}/forgeiso-cache"
remove_path "forgeiso-regtest"   "${REPO_ROOT}/forgeiso-regtest"
remove_glob "tests/test-builds"  "${REPO_ROOT}/tests/test-builds"

# ── Aggressive-only removals ─────────────────────────────────────────────────
if [[ "$MODE" == "aggressive" ]]; then
    remove_path "engine cache"   "${HOME}/.cache/forgeiso"
fi

# ── Summary ──────────────────────────────────────────────────────────────────
if (( TOUCHED == 0 )); then
    printf 'cleanup-artifacts: nothing to do (already clean)\n'
    exit 0
fi

if (( DRY_RUN )); then
    printf 'cleanup-artifacts: would remove %d items totaling %s\n' \
        "$TOTAL_ITEMS" "$(human_size "$TOTAL_BYTES")"
else
    printf 'cleanup-artifacts: removed %d items, freed %s\n' \
        "$TOTAL_ITEMS" "$(human_size "$TOTAL_BYTES")"
fi
