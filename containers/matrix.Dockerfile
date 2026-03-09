FROM rust:1.93-bookworm

ARG DISTRO_LABEL=ubuntu
ARG VERSION_LABEL=24.04
ARG PROFILE_LABEL=minimal

ENV MATRIX_DISTRO=${DISTRO_LABEL} \
    MATRIX_VERSION=${VERSION_LABEL} \
    MATRIX_PROFILE=${PROFILE_LABEL}

RUN apt-get update && apt-get install -y --no-install-recommends \
    grub-common \
    grub-pc-bin \
    grub-efi-amd64-bin \
    mtools \
    xorriso \
    squashfs-tools \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
