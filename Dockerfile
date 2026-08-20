FROM rust:1-bookworm AS builder

WORKDIR /src

RUN apt-get update \
    && apt-get install --no-install-recommends -y \
        libdbus-1-dev \
        libsqlite3-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build -p unfour-mcp --release --no-default-features --locked

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install --no-install-recommends -y \
        ca-certificates \
        libdbus-1-3 \
        libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/target/release/unfour-mcp /usr/local/bin/unfour-mcp

# Registry/CI containers must never attach to a developer's persistent store.
ENV UNFOUR_MCP_STORAGE_MODE=ephemeral
ENV UNFOUR_DATA_DIR=/tmp/unfour-mcp

ENTRYPOINT ["/usr/local/bin/unfour-mcp"]
