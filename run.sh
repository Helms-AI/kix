#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "=== Kix Knowledge Indexer ==="

# Function to kill any process using a specific port
kill_port() {
    local port=$1
    local pids=$(lsof -i :$port 2>/dev/null | grep -v "^COMMAND" | awk '{print $2}' | sort -u)
    if [ -n "$pids" ]; then
        echo "Stopping existing processes on port $port..."
        echo "$pids" | xargs kill -9 2>/dev/null || true
        sleep 1
    fi
}

# Clean up any stale processes on our ports
kill_port 3000
kill_port 3001
kill_port 3002

# Check if Rust binary exists and is up to date
BINARY="./server/target/release/kix"
NEEDS_RUST_BUILD=false

if [ ! -f "$BINARY" ]; then
    echo "Rust binary not found, will build..."
    NEEDS_RUST_BUILD=true
else
    # Check if any Rust source files are newer than the binary
    if find server/crates -name "*.rs" -newer "$BINARY" 2>/dev/null | grep -q .; then
        echo "Rust source files changed, will rebuild..."
        NEEDS_RUST_BUILD=true
    fi
fi

# Build Rust if needed (with SIMD optimizations)
# Default: ONNX Runtime backend with BGE-base embeddings (768 dimensions)
if [ "$NEEDS_RUST_BUILD" = true ]; then
    echo ""
    echo "Building Rust binary with SIMD optimizations (ONNX backend)..."
    RUSTFLAGS="-C target-cpu=native" cargo build --manifest-path server/Cargo.toml --release --package kix-cli
fi

# Install client dependencies if needed
if [ ! -d "./client/node_modules" ]; then
    echo ""
    echo "Installing client dependencies..."
    cd client
    npm ci
    cd ..
fi

# Enable logging
export RUST_LOG="${RUST_LOG:-kix=info,warn}"

# Embedding model configuration (ONNX backend)
# Available models: bge-base-en-v1.5 (768d), bge-small-en-v1.5 (384d), bge-large-en-v1.5 (1024d)
export KIX_EMBEDDING_MODEL="${KIX_EMBEDDING_MODEL:-bge-base-en-v1.5}"
export KIX_EMBEDDING_DIM="${KIX_EMBEDDING_DIM:-768}"

# Start MCP HTTP server in background
echo ""
echo "Starting MCP HTTP server on port 3002..."
$BINARY serve-http --port 3002 &
MCP_PID=$!

# Give MCP server a moment to start
sleep 2

# Start API server in background
echo ""
echo "Starting API server on port 3001..."
$BINARY api --port 3001 &
API_PID=$!

# Give API a moment to start
sleep 2

# Start client dev server
echo "Starting client dev server on port 3000..."
echo ""
echo "=== Services Running ==="
echo "  Web UI:   http://localhost:3000"
echo "  API:      http://localhost:3001 (proxied via /api)"
echo "  Indexing: http://localhost:3001/api/indexing/* (with SSE)"
echo "  MCP:      http://localhost:3002/mcp (proxied via /mcp)"
echo ""
echo "Press Ctrl+C to stop all services"
echo ""

# Trap to kill servers when script exits
trap "kill $MCP_PID $API_PID 2>/dev/null" EXIT

# Run client dev server in foreground
cd client
npm run dev
