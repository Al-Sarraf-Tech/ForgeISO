SHELL := /usr/bin/env bash

.PHONY: dev test build package \
        ci-local ci-parallel ci-base ci-clean \
        matrix matrix-smoke matrix-full matrix-clean \
        lint clean

# ── Development ───────────────────────────────────────────────────────────────
dev:
	@echo "CLI: cargo run -p forgeiso-cli -- doctor"
	@echo "TUI: cargo run -p forgeiso-tui"
	@echo "GUI: cargo run -p forge-slint"

test:
	cargo test --workspace

build:
	cargo build --workspace --release

package:
	@echo "Packaging Linux release tarball"
	scripts/release/package-tarball.sh

lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings

clean:
	cargo clean

# ── CI (local, parallel ephemeral containers) ─────────────────────────────────
#
# ci-base     Build and keep the C0 warm-cache base image (run once or on dep changes)
# ci-parallel Run all 6 stages simultaneously in isolated ephemeral containers
# ci-local    Alias for ci-parallel (backwards compat + pre-push hook parity)
# ci-clean    Remove all CI volumes and the base image

## Build the C0 warm-cache base image.
## Kept alive (not ephemeral) so subsequent CI runs reuse cached Rust toolchain layers.
ci-base:
	docker build \
	  --tag forgeiso-base:latest \
	  --file containers/C0.base.Dockerfile \
	  --progress=plain \
	  .
	@echo "Base image forgeiso-base:latest ready — subsequent CI builds will be faster."

## Run all 6 CI stages in parallel ephemeral containers.
## All must pass. All volumes are destroyed afterwards.
## Override stages with: make ci-parallel CI_STAGES=c1,c3
ci-parallel:
	bash scripts/ci/run-parallel.sh $(if $(CI_STAGES),$(subst $(comma), ,$(CI_STAGES)),)

## Backwards-compatible alias used by the pre-push hook.
ci-local: ci-parallel

## Remove all ephemeral CI volumes and the base image.
ci-clean:
	docker compose -f docker-compose.ci.yml down -v --remove-orphans 2>/dev/null || true
	docker image rm forgeiso-base:latest 2>/dev/null || true
	docker image rm forgeiso-c1 forgeiso-c2 forgeiso-c3 \
	               forgeiso-c4 forgeiso-c5 forgeiso-c6 2>/dev/null || true
	@echo "All CI images and volumes removed."

## Remove all ephemeral matrix volumes and images.
matrix-clean:
	bash scripts/matrix/run-matrix.sh --clean

# ── Distro×Version×Profile matrix (ephemeral, parallel Docker) ────────────────
#
# matrix        Alias for matrix-smoke (fast, no QEMU)
# matrix-smoke  Doctor + build + ISO-9660 check for all cells
# matrix-full   Smoke + QEMU boot test if KVM is available
# MATRIX_CELL   Optionally target one cell: make matrix MATRIX_CELL=ubuntu-2404-minimal

## Fast matrix: doctor + build + ISO-9660 header validation.
matrix-smoke:
	bash scripts/matrix/run-matrix.sh --tier smoke $(if $(MATRIX_CELL),--cell $(MATRIX_CELL),)

## Full matrix: smoke + QEMU boot test (requires KVM on host).
matrix-full:
	bash scripts/matrix/run-matrix.sh --tier full $(if $(MATRIX_CELL),--cell $(MATRIX_CELL),)

## Alias for matrix-smoke.
matrix: matrix-smoke

comma := ,
