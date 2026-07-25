# ------------------------------------------------------------------------------
# NetGene OS — Production Multi-Stage Dockerfile
# ------------------------------------------------------------------------------
FROM rust:1.75-slim as builder

WORKDIR /usr/src/netgene-core
COPY . .

# Build release binary
RUN cargo build --release -p netgene-cli

# ------------------------------------------------------------------------------
# Runtime Image
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/netgene-core/target/release/netgene /usr/local/bin/netgene

# Expose default P2P port
EXPOSE 7777

# Environment variables
ENV RUST_LOG=info
ENV NETGENE_HOME=/app/.netgene

# Default command
CMD ["netgene", "p2p", "listen", "--port", "7777"]
