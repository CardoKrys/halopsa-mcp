FROM rust:1.88-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin hmcp-server

FROM debian:bookworm-slim AS server
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home --uid 10001 hmcp
COPY --from=builder /app/target/release/hmcp-server /usr/local/bin/
USER hmcp
ENTRYPOINT ["hmcp-server"]

FROM rust:1.88-bookworm AS embedder-builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin hmcp-embedder

FROM debian:bookworm-slim AS embedder
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home --uid 10001 hmcp \
    && mkdir -p /app && chown hmcp:hmcp /app
WORKDIR /app
COPY --from=embedder-builder /app/target/release/hmcp-embedder /usr/local/bin/
USER hmcp
ENTRYPOINT ["hmcp-embedder"]
