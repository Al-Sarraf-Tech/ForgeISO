#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

mkdir -p artifacts/e2e

fake_iso=artifacts/e2e/fake.iso
head -c 4096 /dev/zero > "$fake_iso"

cargo run -p forgeiso-cli --offline -- inspect --source "$fake_iso" > artifacts/e2e/inspect.txt || true
cargo run -p forgeiso-cli --offline -- test --iso "$fake_iso" --bios --uefi --json > artifacts/e2e/test.json || true

if command -v qemu-system-x86_64 >/dev/null 2>&1; then
  echo '{"nested_virtualization":"available"}' > artifacts/e2e/virt.json
else
  echo '{"nested_virtualization":"unavailable"}' > artifacts/e2e/virt.json
fi

if command -v qemu-system-x86_64 >/dev/null 2>&1 && { command -v grub2-mkrescue >/dev/null 2>&1 || command -v grub-mkrescue >/dev/null 2>&1; }; then
  smoke_dir="artifacts/e2e/smoke"
  eval "$(scripts/test/make-smoke-iso.sh "$smoke_dir")"

  cargo run -p forgeiso-cli --offline -- build \
    --source "$ISO" \
    --out "$smoke_dir/out" \
    --name ci-e2e \
    --overlay "$OVERLAY" \
    --profile minimal \
    --json > "$smoke_dir/build.json"
  cargo run -p forgeiso-cli --offline -- test \
    --iso "$smoke_dir/out/ci-e2e.iso" \
    --bios \
    --uefi \
    --json > "$smoke_dir/test.json"

  grep -q 'FORGEISO_SMOKE_START' "$smoke_dir/out/test/bios-serial.log"
  grep -q 'FORGEISO_SMOKE_START' "$smoke_dir/out/test/uefi-serial.log"
fi
