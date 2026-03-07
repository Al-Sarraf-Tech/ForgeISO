SHELL := /usr/bin/env bash

.PHONY: dev test build package \
        ci-local ci-parallel ci-base ci-clean \
        lint clean

# ── Development ───────────────────────────────────────────────────────────────
dev:
	@echo "CLI: cargo run -p forgeiso-cli -- doctor"
	@echo "TUI: cargo run -p forgeiso-tui"
	@echo "GUI: cd gui && npm run build && cargo run --manifest-path src-tauri/Cargo.toml"

test:
	cargo test --workspace

build:
	cargo build --workspace --release
	@echo "GUI build: cd gui && npm run build && cargo build --manifest-path src-tauri/Cargo.toml --release"

package:
	@echo "Packaging Linux release tarball"
	scripts/release/package-tarball.sh

lint:
	cargo check --workspace
	cd gui && npm run lint
	cargo check --manifest-path gui/src-tauri/Cargo.toml

clean:
	cargo clean
	cd gui && rm -rf dist src-tauri/target

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

comma := ,
