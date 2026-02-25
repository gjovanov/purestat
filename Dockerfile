# Stage 1: cargo-chef for dependency caching
FROM rust:1.85-slim AS chef
RUN cargo install cargo-chef
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
RUN cargo build --release --bin purestat-api --bin purestat-tracker

# Stage 4: minimal runtime
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/purestat-api /usr/local/bin/purestat-api
COPY --from=builder /app/target/release/purestat-tracker /usr/local/bin/purestat-tracker

EXPOSE 3000 3001

CMD ["purestat-api"]
