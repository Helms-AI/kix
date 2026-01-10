#!/bin/bash

set -e

echo "=== Building KIX Knowledge Indexer ==="
echo ""

# Build Rust binary with SIMD optimizations
# Default: ONNX Runtime backend with BGE-base embeddings (768 dimensions)
# For GPU support, use: --features onnx-coreml (Apple) or --features onnx-cuda (NVIDIA)
echo "Building Rust binary with SIMD optimizations (ONNX backend)..."
RUSTFLAGS="-C target-cpu=native" cargo build --manifest-path server/Cargo.toml --release --package kix-cli

# Build client frontend
echo ""
echo "Building client frontend..."
cd client
npm ci
npm run build
cd ..

echo ""
echo "=== Build complete ==="
echo ""
echo "Local usage:"
echo "  ./run.sh                    # Start all services locally"
echo "  ./server/target/release/kix # Run CLI directly"
echo ""
echo "Docker usage:"
echo "  docker compose build        # Build Docker images"
echo "  docker compose up -d        # Start services"
echo ""
echo "Setup (downloads embedding model):"
echo "  ./server/target/release/kix setup  # Download BGE-base model (~400MB)"
echo ""
echo "Performance:"
echo "  - ONNX Runtime backend with BGE-base embeddings (768 dimensions)"
echo "  - SIMD optimizations enabled (target-cpu=native)"
echo "  - jemalloc memory allocator"
echo "  - Optimized batch sizes (256-512)"
echo ""
echo "GPU acceleration (rebuild with):"
echo "  # Apple Silicon (Metal/CoreML):"
echo "  RUSTFLAGS=\"-C target-cpu=native\" cargo build --manifest-path server/Cargo.toml --release --features onnx-coreml"
echo "  # NVIDIA (CUDA):"
echo "  RUSTFLAGS=\"-C target-cpu=native\" cargo build --manifest-path server/Cargo.toml --release --features onnx-cuda"
