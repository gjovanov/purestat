# Stage 1: cargo-chef for dependency caching
# The toolchain the test suite runs on. `-bookworm` explicitly: the runtime
# stage is debian:bookworm-slim, and a builder on a newer Debian would link
# against a glibc the runtime does not have (a failure only at start-up).
FROM rust:1.95-slim-bookworm AS chef
# cargo-chef stays pinned (and --locked) so the image does not move under us.
RUN cargo install cargo-chef --version 0.1.71 --locked
WORKDIR /app

# Stage 2: prepare recipe
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: build dependencies and binaries
FROM chef AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
# --locked: the committed Cargo.lock is the resolution the tag was tested with.
RUN cargo build --release --locked --bin purestat-api --bin purestat-tracker

# Stage 4: minimal runtime
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/purestat-api /usr/local/bin/purestat-api
COPY --from=builder /app/target/release/purestat-tracker /usr/local/bin/purestat-tracker
COPY scripts/geoip-download.sh /usr/local/bin/geoip-download.sh

EXPOSE 3000 3001

ENTRYPOINT ["geoip-download.sh"]
CMD ["purestat-api"]
