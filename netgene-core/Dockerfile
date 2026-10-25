# NetGene OS — headless P2P node image
# Build from the netgene-core directory:  docker build -t netgene netgene-core

FROM rust:1.94-slim-bookworm AS builder
WORKDIR /usr/src/netgene-core
COPY . .
RUN cargo build --release -p netgene-cli

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 netgene
COPY --from=builder /usr/src/netgene-core/target/release/netgene /usr/local/bin/netgene

USER netgene
WORKDIR /home/netgene
ENV RUST_LOG=info

EXPOSE 7777
CMD ["netgene", "p2p", "listen", "--port", "7777"]
