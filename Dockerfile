FROM rust:1-trixie AS base
RUN cargo install cargo-chef --version 0.1.78 --locked

FROM base AS planner
WORKDIR /app
COPY ./core ./
RUN cargo chef prepare

FROM base AS builder
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked
COPY ./core ./
RUN cargo build --release --locked

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3t64 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dandelion /usr/local/bin/dandelion
ENTRYPOINT ["/usr/local/bin/dandelion"]
CMD ["/config.rn"]
