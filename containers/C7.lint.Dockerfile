FROM rust:1.93-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    # Slint system library dependencies (backend-winit + renderer-femtovg)
    libxkbcommon-dev \
    libwayland-dev \
    libegl-dev \
    libgl-dev \
    libfontconfig1-dev \
    libdbus-1-dev \
    libx11-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    && rm -rf /var/lib/apt/lists/*
RUN rustup component add rustfmt clippy
WORKDIR /workspace
