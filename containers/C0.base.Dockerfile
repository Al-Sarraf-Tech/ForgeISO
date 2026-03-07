# ForgeISO CI Base Image (C0)
#
# Shared warm-cache layer used by all 6 CI containers.
# Build once, reuse across all stages — subsequent stage builds pull from
# the local image cache instead of downloading Rust toolchain + deps again.
#
# Build:
#   docker build -t forgeiso-base:latest -f containers/C0.base.Dockerfile .
#
# This image is intentionally kept alive (not --rm) so it persists as a
# layer cache for C1–C6. Refresh it with: make ci-base

FROM rust:1.93-bookworm

# System packages needed by at least two CI stages
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
    git \
    xorriso \
    mtools \
    squashfs-tools \
    && rm -rf /var/lib/apt/lists/*

# Pre-install Rust toolchain components (fmt + clippy)
RUN rustup component add rustfmt clippy

# Pre-warm the serde + tokio + clap dependency compile — the heaviest crates.
# We copy only Cargo manifests so this layer is invalidated only on dep changes.
WORKDIR /prefetch
COPY Cargo.toml Cargo.lock ./
COPY engine/Cargo.toml engine/Cargo.toml
COPY cli/Cargo.toml    cli/Cargo.toml
COPY tui/Cargo.toml    tui/Cargo.toml

# Create stub lib/main files so `cargo fetch` + `cargo build` can resolve the graph
RUN mkdir -p engine/src cli/src tui/src \
    && echo 'pub fn _stub() {}' > engine/src/lib.rs \
    && echo 'fn main() {}' > cli/src/main.rs \
    && echo 'fn main() {}' > tui/src/main.rs \
    && CARGO_HOME=/ci-base-cargo cargo fetch --locked 2>/dev/null || cargo fetch \
    && rm -rf /prefetch

WORKDIR /workspace

LABEL org.opencontainers.image.title="forgeiso-base" \
      org.opencontainers.image.description="ForgeISO CI warm cache base image"
