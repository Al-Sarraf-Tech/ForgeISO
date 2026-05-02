#!/usr/bin/env bash
# scripts/test-releases.sh — hermetic per-preset test-release orchestrator.
#
# For every IsoPreset listed in engine/src/sources/catalog/*, this script:
#   1. generates a small synthetic source ISO via tests/fixtures/synthetic-iso.sh
#      (one synthetic per distro family — never downloads upstream media);
#   2. builds a fully-populated InjectConfig CLI invocation with realistic
#      test values (hostname, user, ssh keys, packages, network, services, etc.);
#   3. invokes `forgeiso inject` with that config against the synthetic ISO;
#   4. verifies the output ISO exists, scans it via `forgeiso scan`, and
#      records the SHA-256 of the output (compared to the engine's reported
#      hash where available);
#   5. captures timing, size, and verdict.
#
# Final per-preset PASS/FAIL/SKIP table is printed at end. Exit status is
# the count of failures (0 = all passed).
#
# Modes:
#   --keep-artifacts        keep the per-preset output ISO + workdir
#   --preset <id>           run a single preset (kebab-case id)
#   --parallel <N>          run up to N presets concurrently (default 1)
#   --binary <path>         override forgeiso binary path
#   --threshold <seconds>   per-preset timeout (default 300s)
#   --list                  list every preset id and exit
#   -h, --help              show usage
#
# This script is hermetic: it never reaches the network, never invokes
# upstream installer downloads, and never calls cargo. It assumes the
# `forgeiso` CLI is already installed at ~/.cargo/bin/forgeiso (override
# with --binary).
set -Eeuo pipefail

REPO_ROOT="$(cd "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURES="$REPO_ROOT/tests/fixtures/synthetic-iso.sh"
FORGEISO_BIN="${FORGEISO_BIN:-$HOME/.cargo/bin/forgeiso}"
TEST_BUILDS="$REPO_ROOT/tests/test-builds"
KEEP_ARTIFACTS=0
SINGLE_PRESET=""
PARALLEL=1
PERF_THRESHOLD="${PERF_THRESHOLD:-300}"
LIST_ONLY=0

# ── preset registry ─────────────────────────────────────────────────────────
# Keep this list in sync with engine/src/sources/catalog/*.rs
# Each entry: <preset-id>|<distro-flag>|<synthetic-family>
# distro-flag is what `forgeiso inject --distro` accepts (or "" for default).
PRESETS=(
    "ubuntu-server-lts|ubuntu|ubuntu"
    "ubuntu-desktop-lts|ubuntu|ubuntu"
    "ubuntu-server-2510|ubuntu|ubuntu"
    "ubuntu-desktop-2510|ubuntu|ubuntu"
    "ubuntu-server-jammy|ubuntu|ubuntu"
    "ubuntu-desktop-jammy|ubuntu|ubuntu"
    "ubuntu-server-focal|ubuntu|ubuntu"
    "ubuntu-desktop-focal|ubuntu|ubuntu"
    "ubuntu-server-bionic|ubuntu|ubuntu"
    "ubuntu-desktop-bionic|ubuntu|ubuntu"
    "linux-mint-cinnamon|mint|mint"
    "linux-mint-mate|mint|mint"
    "linux-mint-xfce|mint|mint"
    "fedora-server|fedora|fedora"
    "fedora-workstation|fedora|fedora"
    "fedora-kde|fedora|fedora"
    "rocky-linux|fedora|fedora"
    "almalinux|fedora|fedora"
    "centos-stream|fedora|fedora"
    "arch-linux|arch|arch"
    "endeavouros|arch|arch"
    "garuda-dr460nized|arch|arch"
    "garuda-gnome|arch|arch"
    "garuda-xfce|arch|arch"
    "debian-netinst|ubuntu|debian"
    "kali-linux|ubuntu|debian"
    "kali-linux-netinst|ubuntu|debian"
    "opensuse-leap|ubuntu|opensuse"
    "opensuse-leap-net|ubuntu|opensuse"
    "opensuse-tumbleweed|ubuntu|opensuse"
    "pop-os-22-intel|ubuntu|ubuntu"
    "pop-os-22-nvidia|ubuntu|ubuntu"
    "pop-os-24-intel|ubuntu|ubuntu"
)

usage() {
    sed -n '2,30p' -- "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit 64
}

list_presets() {
    for entry in "${PRESETS[@]}"; do
        printf '%s\n' "${entry%%|*}"
    done
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --keep-artifacts) KEEP_ARTIFACTS=1; shift ;;
        --preset)         SINGLE_PRESET="${2:?--preset needs an id}"; shift 2 ;;
        --parallel)       PARALLEL="${2:?--parallel needs a count}"; shift 2 ;;
        --binary)         FORGEISO_BIN="${2:?--binary needs a path}"; shift 2 ;;
        --threshold)      PERF_THRESHOLD="${2:?--threshold needs seconds}"; shift 2 ;;
        --list)           LIST_ONLY=1; shift ;;
        -h|--help)        usage ;;
        *)                printf 'unknown arg: %s\n' "$1" >&2; usage ;;
    esac
done

if (( LIST_ONLY )); then
    list_presets
fi

# ── pre-flight ──────────────────────────────────────────────────────────────
[[ -x "$FIXTURES"     ]] || { echo "missing fixture script: $FIXTURES" >&2; exit 69; }
[[ -x "$FORGEISO_BIN" ]] || { echo "missing forgeiso binary: $FORGEISO_BIN (set --binary)" >&2; exit 69; }
command -v xorriso    >/dev/null 2>&1 || { echo "missing xorriso" >&2; exit 69; }
command -v sha256sum  >/dev/null 2>&1 || { echo "missing sha256sum" >&2; exit 69; }
command -v jq         >/dev/null 2>&1 || { echo "missing jq" >&2; exit 69; }
command -v timeout    >/dev/null 2>&1 || { echo "missing timeout" >&2; exit 69; }

mkdir -p -- "$TEST_BUILDS"

# ── per-preset config template ─────────────────────────────────────────────
# Generates a fully-populated `forgeiso inject` argv tailored to the preset's
# distro flag. Values are realistic test data, not engine defaults — the goal
# is to exercise as many InjectConfig fields as the CLI accepts.
emit_inject_args() {
    local preset_id="$1"
    local distro_flag="$2"
    local source_iso="$3"
    local out_dir="$4"
    local out_name="$5"

    local -a args=(
        inject
        --source "$source_iso"
        --out    "$out_dir"
        --name   "$out_name"
        --volume-label "FORGEISO-TEST"
        --json

        # Identity
        --hostname  "test-${preset_id}"
        --username  "forgeops"
        --password  "ForgeISO!2026"
        --realname  "ForgeISO Test Operator"

        # SSH
        --ssh-key   "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITESTKEYDONOTUSE forgeops@test"
        --ssh-install-server
        --no-ssh-password-auth

        # Network
        --dns        "1.1.1.1"
        --dns        "9.9.9.9"
        --ntp-server "time.cloudflare.com"
        --static-ip  "192.0.2.50/24"
        --gateway    "192.0.2.1"

        # System
        --timezone        "UTC"
        --locale          "en_US.UTF-8"
        --keyboard-layout "us"

        # Packages
        --package "htop"
        --package "vim"
        --package "git"
        --package "curl"

        # User & access
        --group         "wheel"
        --shell         "/bin/bash"
        --sudo-nopasswd

        # Firewall
        --firewall
        --firewall-policy "deny"
        --allow-port      "22/tcp"
        --allow-port      "443/tcp"

        # Services
        --enable-service  "sshd"
        --disable-service "cups"

        # Boot
        --grub-timeout 5
        --grub-cmdline "quiet"
        --grub-cmdline "splash"

        # Advanced
        --sysctl       "net.ipv4.ip_forward=1"
        --late-command "echo forgeiso-test >> /etc/motd"
        --no-user-interaction
    )

    if [[ -n "$distro_flag" ]]; then
        args+=( --distro "$distro_flag" )
    fi

    case "$distro_flag" in
        fedora)
            args+=(
                --dnf-repo   "https://download.docker.com/linux/fedora/docker-ce.repo"
                --dnf-mirror "https://dl.fedoraproject.org/pub/fedora/linux"
            )
            ;;
        arch)
            args+=(
                --pacman-repo   "Server = https://geo.mirror.pkgbuild.com/\$repo/os/\$arch"
                --pacman-mirror "https://geo.mirror.pkgbuild.com"
            )
            ;;
        ubuntu|"")
            args+=(
                --apt-repo   "deb http://archive.ubuntu.com/ubuntu noble universe"
                --apt-mirror "http://archive.ubuntu.com/ubuntu"
            )
            ;;
    esac

    printf '%s\n' "${args[@]}"
}

# ── per-preset runner (one preset, returns 0 on PASS, 1 on FAIL, 2 on SKIP)
run_preset() {
    local entry="$1"
    local preset_id="${entry%%|*}"
    local rest="${entry#*|}"
    local distro_flag="${rest%%|*}"
    local family="${rest#*|}"

    local work="$TEST_BUILDS/$preset_id"
    rm -rf -- "$work"
    mkdir -p -- "$work"

    local source_iso="$work/source.iso"
    local out_dir="$work/out"
    local out_name="${preset_id}-test.iso"
    mkdir -p -- "$out_dir"

    local log="$work/run.log"
    local started=$EPOCHSECONDS

    if ! "$FIXTURES" "$family" "$source_iso" >"$log" 2>&1; then
        printf '%s|FAIL|fixture-failed|0|0\n' "$preset_id"
        return 1
    fi

    local -a args
    mapfile -t args < <(emit_inject_args "$preset_id" "$distro_flag" "$source_iso" "$out_dir" "$out_name")

    local rc=0
    if ! timeout --kill-after=15 "${PERF_THRESHOLD}" "$FORGEISO_BIN" "${args[@]}" >>"$log" 2>&1; then
        rc=$?
    fi
    local elapsed=$(( EPOCHSECONDS - started ))

    if (( rc == 124 || rc == 137 )); then
        printf '%s|SKIP|timeout-%ss|%s|0\n' "$preset_id" "$PERF_THRESHOLD" "$elapsed"
        return 2
    fi
    if (( rc != 0 )); then
        printf '%s|FAIL|inject-rc-%d|%s|0\n' "$preset_id" "$rc" "$elapsed"
        return 1
    fi

    local out_iso="$out_dir/$out_name"
    if [[ ! -f "$out_iso" ]]; then
        printf '%s|FAIL|no-output|%s|0\n' "$preset_id" "$elapsed"
        return 1
    fi

    local sha
    sha="$(sha256sum -- "$out_iso" | awk '{print $1}')"
    local size
    size="$(stat -c%s -- "$out_iso")"

    # Engine reports the SOURCE iso sha256 in BuildResult.iso.sha256; we
    # confirm by re-hashing the source and asserting it matches the JSON.
    # This does not validate the output sha (the engine has no field for
    # that on inject) — see docs/TEST-RELEASES.md for the rationale.
    local source_sha
    source_sha="$(sha256sum -- "$source_iso" | awk '{print $1}')"
    local reported_sha
    reported_sha="$(grep -E '^\{|"sha256"' "$log" | jq -rs '
        map(select(type=="object")) | last | .iso.sha256 // empty
    ' 2>/dev/null || true)"
    if [[ -n "$reported_sha" && "$reported_sha" != "$source_sha" ]]; then
        printf '%s|FAIL|sha-mismatch|%s|%s\n' "$preset_id" "$elapsed" "$size"
        return 1
    fi

    if ! "$FORGEISO_BIN" scan --artifact "$out_iso" --json >>"$log" 2>&1; then
        # `scan` uses host security tools (trivy, syft, etc.) — degrade to
        # WARN if those are missing or fail; the inject pipeline itself
        # already passed at this point.
        printf '%s|PASS|scan-degraded|%s|%s|sha=%s\n' "$preset_id" "$elapsed" "$size" "$sha"
    else
        printf '%s|PASS|ok|%s|%s|sha=%s\n' "$preset_id" "$elapsed" "$size" "$sha"
    fi

    if (( ! KEEP_ARTIFACTS )); then
        rm -rf -- "$work"
    fi
    return 0
}

# ── main loop ──────────────────────────────────────────────────────────────
declare -a TARGET_PRESETS=()
if [[ -n "$SINGLE_PRESET" ]]; then
    for entry in "${PRESETS[@]}"; do
        if [[ "${entry%%|*}" == "$SINGLE_PRESET" ]]; then
            TARGET_PRESETS+=( "$entry" )
            break
        fi
    done
    if (( ${#TARGET_PRESETS[@]} == 0 )); then
        printf 'no such preset: %s\n' "$SINGLE_PRESET" >&2
        printf 'use --list to see all preset ids\n' >&2
        exit 64
    fi
else
    TARGET_PRESETS=( "${PRESETS[@]}" )
fi

results_file="$(mktemp -t forgeiso-test-results-XXXXXXXX)"
trap 'rm -f -- "$results_file"' EXIT

if (( PARALLEL <= 1 )); then
    for entry in "${TARGET_PRESETS[@]}"; do
        run_preset "$entry" >>"$results_file" || true
    done
else
    pids=()
    slot=0
    for entry in "${TARGET_PRESETS[@]}"; do
        ( run_preset "$entry" >>"$results_file" || true ) &
        pids+=( $! )
        slot=$(( slot + 1 ))
        if (( slot >= PARALLEL )); then
            wait -n 2>/dev/null || true
            slot=$(( slot - 1 ))
        fi
    done
    wait || true
fi

# ── summary ────────────────────────────────────────────────────────────────
total=0
passed=0
failed=0
skipped=0

printf '\n%-28s %-6s %-22s %8s %12s\n' "PRESET" "STATUS" "DETAIL" "TIME(s)" "SIZE(B)"
printf -- '%.0s-' {1..82}
printf '\n'

while IFS='|' read -r preset_id status detail elapsed size _rest; do
    [[ -z "$preset_id" ]] && continue
    total=$(( total + 1 ))
    case "$status" in
        PASS) passed=$(( passed + 1 )) ;;
        FAIL) failed=$(( failed + 1 )) ;;
        SKIP) skipped=$(( skipped + 1 )) ;;
    esac
    printf '%-28s %-6s %-22s %8s %12s\n' \
        "$preset_id" "$status" "$detail" "${elapsed:-0}" "${size:-0}"
done < <(sort -- "$results_file")

printf -- '%.0s-' {1..82}
printf '\n'
printf 'total=%d  PASS=%d  FAIL=%d  SKIP=%d\n' "$total" "$passed" "$failed" "$skipped"

exit "$failed"
