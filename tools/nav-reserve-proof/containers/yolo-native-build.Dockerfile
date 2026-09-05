# Build-only environment matching the qualified collector's Debian 12 ABI.
# Record the resulting immutable image ID and package inventory with each build.
# Go and Rust toolchains are supplied as independently pinned read-only mounts.
FROM python@sha256:ed86c82274b3c69b52fb5820f358f0bd7df0b603332063cb5c6e32bd220c3e6e
RUN apt-get update && apt-get install -y --no-install-recommends \
    gcc libc6-dev libclang-14-dev protobuf-compiler pkg-config \
    && dpkg-query -W > /build-package-versions.txt \
    && rm -rf /var/lib/apt/lists/*
