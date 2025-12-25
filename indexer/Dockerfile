FROM rust:1.88 AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY common ./common
COPY storage ./storage
COPY indexer ./indexer
COPY migrations ./migrations
RUN cargo build --release --package deepbook-indexer-indexer --bin deepbook-indexer-indexer

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/deepbook-indexer-indexer /usr/local/bin/indexer
ENTRYPOINT ["/usr/local/bin/indexer"]
