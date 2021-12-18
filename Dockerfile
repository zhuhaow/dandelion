FROM rust:1.57-buster as base
RUN cargo install cargo-chef

FROM base as planner
WORKDIR app
COPY ./core ./
RUN cargo chef prepare

FROM base as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release
COPY ./core ./
RUN cargo build --release
# So we know where the binary will be
RUN cargo install --path . --locked

FROM debian:buster-slim
RUN apt update && apt install -y --no-install-recommends \
            ca-certificates \
            libssl1.1 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/specht2 /usr/local/bin/specht2
ENTRYPOINT ["/usr/local/bin/specht2"]
CMD ["/config.ron"]
