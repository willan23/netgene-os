# NetGene OS — headless P2P node image
# Build from the netgene-core directory:  docker build -t netgene netgene-core
# Usage with compose: see deploy/docker-compose.yml

FROM rust:1.94-slim-bookworm AS builder
# clang + libbpf-dev compile the eBPF program (crate netgene-ebpf).
RUN apt-get update && apt-get install -y --no-install-recommends clang libbpf-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/src/netgene-core
COPY . .
RUN cargo build --release -p netgene-cli

FROM debian:bookworm-slim
# The data dir is created here, owned by netgene, so a named volume mounted
# on it inherits that ownership (otherwise Docker creates it as root).
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 netgene \
    && install -d -m 0700 -o netgene -g netgene /home/netgene/.netgene
COPY --from=builder /usr/src/netgene-core/target/release/netgene /usr/local/bin/netgene

USER netgene
WORKDIR /home/netgene
ENV RUST_LOG=info
# Containers have no OS keystore: use the passphrase-encrypted file store
# (NETGENE_SECRET_PASSPHRASE_FILE points at a Docker secret).
ENV NETGENE_SECRET_STORE=file

EXPOSE 7777
# `p2p listen` reads the gene id from NETGENE_GENE_ID (see deploy/docker-compose.yml).
# Other commands: `docker run … netgene-os gene init --name my-node`.
ENTRYPOINT ["netgene"]
CMD ["p2p", "listen", "--port", "7777"]
