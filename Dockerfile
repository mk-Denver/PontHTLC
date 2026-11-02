FROM rust:1-bookworm AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        clang \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

ENV CARGO_INCREMENTAL=0
RUN cargo build --release -p ponthtlc

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /src/target/release/ponthtlc /usr/local/bin/ponthtlc

EXPOSE 9800
VOLUME /data
ENTRYPOINT ["ponthtlc"]
CMD ["--data-dir", "/data", "--bind", "0.0.0.0", "--port", "9800", "--ln-port", "9735", "--seller-ln-port", "9736", "--bitcoind-host", "bitcoind", "--bitcoind-port", "18923", "--rpc-user", "ponthtlc", "--rpc-pass", "ponthtlc"]
