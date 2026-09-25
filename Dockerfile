# syntax=docker/dockerfile:1
FROM rust:1.85-bookworm AS builder

WORKDIR /usr/src/ezrouter

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    sqlite3 \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source tree (including pre-built ui/dist)
COPY . .

# Build release binary
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /usr/src/ezrouter/target/release/ezrouter /usr/local/bin/ezrouter

# Default environment configuration
ENV AG_HOST=0.0.0.0 \
    AG_PORT=20229 \
    AG_DATA_DIR=/data \
    RUST_LOG=ezrouter=info,tower_http=info

VOLUME ["/data"]
EXPOSE 20229

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:20229/health || exit 1

ENTRYPOINT ["ezrouter"]
