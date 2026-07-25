# Use official Rust image for building
FROM rust:1.80 as builder

WORKDIR /usr/src/netgene
COPY . .

# Build the workspace
RUN cargo build --release

# Use a minimal Ubuntu runtime
FROM ubuntu:22.04

RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/netgene/target/release/netgene-cli /usr/local/bin/netgene-cli

# Default port for P2P Mesh
EXPOSE 8000

CMD ["netgene-cli", "start"]
