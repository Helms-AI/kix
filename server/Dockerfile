# Build stage for Rust backend
# NOTE: This file disables SSL verification for cargo. Use only in trusted networks.
FROM rust:1.83-bookworm AS rust-builder

WORKDIR /app

# Build arguments for optimization level
# - "native" for local builds (best performance, not portable)
# - "x86-64-v3" for modern x86-64 CPUs with AVX2 (good balance)
# - "x86-64" for maximum compatibility
ARG TARGET_CPU=x86-64-v3

# Install dependencies for PDF parsing and OpenSSL
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    ca-certificates \
    && update-ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Configure cargo to skip SSL verification (for corporate proxy environments)
RUN mkdir -p ~/.cargo && echo '[http]\ncheck-revoke = false\nssl-version = "tlsv1.2"\n\n[net]\ngit-fetch-with-cli = true' > ~/.cargo/config.toml

# Copy Cargo workspace files
COPY server/Cargo.toml server/Cargo.lock ./
COPY server/crates ./crates

# Build release binary with optimizations
# - SIMD instructions via target-cpu
# - LTO for link-time optimization
ENV CARGO_HTTP_CHECK_REVOKE=false
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENV RUSTFLAGS="-C target-cpu=${TARGET_CPU}"
RUN cargo build --release --package kix-cli

# Build stage for React frontend
FROM node:20-slim AS client-builder

WORKDIR /app/client

# Copy package files and install dependencies
COPY client/package.json client/package-lock.json* ./
RUN npm ci

# Copy source and build
COPY client/ ./
RUN npm run build

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies including Chromium for Playwright browser rendering
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    # Chromium dependencies for Playwright
    libnss3 \
    libnspr4 \
    libatk1.0-0 \
    libatk-bridge2.0-0 \
    libcups2 \
    libdrm2 \
    libdbus-1-3 \
    libxkbcommon0 \
    libatspi2.0-0 \
    libxcomposite1 \
    libxdamage1 \
    libxfixes3 \
    libxrandr2 \
    libgbm1 \
    libasound2 \
    libpango-1.0-0 \
    libcairo2 \
    libxshmfence1 \
    libx11-6 \
    libxcb1 \
    libxext6 \
    fonts-liberation \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from rust builder
COPY --from=rust-builder /app/target/release/kix /usr/local/bin/kix

# Copy client static files from client builder
COPY --from=client-builder /app/client/dist /app/client/dist

# Create directories for data, model cache, and Playwright browsers
RUN mkdir -p /app/data/lancedb /app/_content /app/cache/kix/models /app/cache/playwright

# Default environment variables
ENV KIX_DB_PATH=/app/data/lancedb
ENV KIX_CONTENT_PATH=/app/_content
ENV KIX_API_PORT=3001
ENV RUST_LOG=info
# ONNX embedding model configuration
ENV KIX_EMBEDDING_MODEL=bge-base-en-v1.5
ENV KIX_EMBEDDING_DIM=768
# Point cache directory to persistent storage
ENV XDG_CACHE_HOME=/app/cache
# Playwright browser configuration (browsers will be downloaded on first use)
ENV PLAYWRIGHT_BROWSERS_PATH=/app/cache/playwright
# Disable browser sandbox for Docker (running as non-root or in container)
ENV PLAYWRIGHT_CHROMIUM_SANDBOX=false

# Expose API port
EXPOSE 3001

# Default command: run API server
CMD ["kix", "api", "--port", "3001", "--db-path", "/app/data/lancedb"]
