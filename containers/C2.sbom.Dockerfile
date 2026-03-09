FROM rust:1.93-bookworm

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-deny for license + advisory policy enforcement
RUN cargo install cargo-deny --version "0.16" --locked 2>/dev/null \
    || cargo install cargo-deny --locked

# Install cargo-audit for advisory database checks
RUN cargo install cargo-audit --locked

# Install syft for SBOM generation (CycloneDX + SPDX)
# Pinned to a tagged release to reduce supply chain risk.
ARG SYFT_VERSION=v1.42.1
RUN curl -sSfL "https://raw.githubusercontent.com/anchore/syft/${SYFT_VERSION}/install.sh" \
    -o /tmp/install-syft.sh \
    && sh /tmp/install-syft.sh -b /usr/local/bin "${SYFT_VERSION}" \
    && rm -f /tmp/install-syft.sh

WORKDIR /workspace
