# Dev/build image for the MSI Center replacement (Linux, Slint UI).
# Only what compiling a Slint desktop app needs - no JDK, no Ghidra, no Node.
FROM rust:1-bookworm

ENV DEBIAN_FRONTEND=noninteractive
ENV CARGO_HOME=/usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH=/usr/local/cargo/bin:$PATH

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates git pkg-config procps \
        libfontconfig1-dev libfreetype6-dev \
        libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev \
        libgl1-mesa-dev libegl1-mesa-dev \
        python3 \
    && rm -rf /var/lib/apt/lists/*

# Rust toolchain is already in the base image; make sure the components we use exist.
RUN rustc --version && cargo --version \
    && rustup component add rustfmt clippy

ARG UID=1000
ARG GID=1000
RUN groupadd -g ${GID} dev 2>/dev/null || true \
    && useradd -m -u ${UID} -g ${GID} -s /bin/bash dev 2>/dev/null || true \
    && mkdir -p /work && chown ${UID}:${GID} /work \
    && chown -R ${UID}:${GID} ${CARGO_HOME} /usr/local/rustup

RUN echo 'export PATH=/usr/local/cargo/bin:$PATH' > /etc/profile.d/rust.sh

USER ${UID}:${GID}
WORKDIR /work
CMD ["sleep", "infinity"]
