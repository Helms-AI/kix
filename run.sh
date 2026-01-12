#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "=== Kix Knowledge Indexer ==="
echo ""

# Detect OS
OS="$(uname -s)"
ARCH="$(uname -m)"

echo "Detected: $OS ($ARCH)"

# Determine execution mode based on OS
# - macOS: Use local binaries + Ollama (Metal GPU acceleration)
# - Linux/Windows: Use Docker Compose (optional CUDA support)
case "$OS" in
    Darwin)
        echo "Mode: Local (macOS with Ollama)"
        EXECUTION_MODE="local"
        ;;
    Linux)
        # Check if running in WSL (Windows Subsystem for Linux)
        if grep -qi microsoft /proc/version 2>/dev/null; then
            echo "Mode: Docker (Windows/WSL)"
            EXECUTION_MODE="docker"
        else
            echo "Mode: Docker (Linux)"
            EXECUTION_MODE="docker"
        fi
        ;;
    MINGW*|MSYS*|CYGWIN*)
        echo "Mode: Docker (Windows)"
        EXECUTION_MODE="docker"
        ;;
    *)
        echo "Unknown OS: $OS - defaulting to Docker mode"
        EXECUTION_MODE="docker"
        ;;
esac

echo ""

#
# DOCKER MODE - Use docker-compose for all services
#
run_docker_mode() {
    echo "Starting services via Docker Compose..."
    echo ""

    # Check if Docker is available
    if ! command -v docker &> /dev/null; then
        echo "ERROR: Docker is not installed or not in PATH"
        echo "Please install Docker Desktop: https://www.docker.com/products/docker-desktop"
        exit 1
    fi

    # Check if docker compose is available
    if ! docker compose version &> /dev/null; then
        echo "ERROR: Docker Compose is not available"
        echo "Please install Docker Compose: https://docs.docker.com/compose/install/"
        exit 1
    fi

    # Check if Ollama service needs model setup
    echo "Checking if Ollama model needs to be pulled..."
    if ! docker compose exec -T ollama ollama list 2>/dev/null | grep -q "nomic-embed-text"; then
        echo "Pulling nomic-embed-text model (first run only)..."
        docker compose up -d ollama
        sleep 5  # Wait for Ollama to start
        docker compose run --rm ollama-setup
    fi

    # Start all services
    echo ""
    echo "Starting all services..."
    docker compose up -d

    echo ""
    echo "=== Services Running (Docker) ==="
    echo "  Web UI:   http://localhost:3000"
    echo "  API:      http://localhost:3001"
    echo "  Ollama:   http://localhost:11434"
    echo ""
    echo "View logs: docker compose logs -f"
    echo "Stop all:  docker compose down"
    echo ""
}

#
# LOCAL MODE - Use local binaries + Ollama (macOS)
#
run_local_mode() {
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

    # Check if Ollama is installed
    if ! command -v ollama &> /dev/null; then
        echo "Ollama not found. Installing via Homebrew..."
        if command -v brew &> /dev/null; then
            brew install ollama
        else
            echo "ERROR: Homebrew not found. Please install Ollama manually:"
            echo "  https://ollama.ai/download"
            exit 1
        fi
    fi

    # Check if Ollama is running, start if not
    check_ollama() {
        curl -sf "http://localhost:11434/api/version" > /dev/null 2>&1
    }

    if ! check_ollama; then
        echo "Starting Ollama server..."
        ollama serve &
        OLLAMA_PID=$!

        # Wait for Ollama to be ready
        echo "Waiting for Ollama to start..."
        max_attempts=30
        attempt=0
        while ! check_ollama; do
            attempt=$((attempt + 1))
            if [ $attempt -ge $max_attempts ]; then
                echo "ERROR: Ollama failed to start"
                exit 1
            fi
            sleep 1
        done
        echo "Ollama started (PID: $OLLAMA_PID)"
    else
        echo "Ollama is already running"
        OLLAMA_PID=""
    fi

    # Check if nomic-embed-text model is available, pull if needed
    if ! ollama list 2>/dev/null | grep -q "nomic-embed-text"; then
        echo ""
        echo "Pulling nomic-embed-text model (first run only)..."
        ollama pull nomic-embed-text
    fi

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
    if [ "$NEEDS_RUST_BUILD" = true ]; then
        echo ""
        echo "Building Rust binary with SIMD optimizations (Ollama backend)..."
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

    # Enable logging (include debug for parser and embeddings to trace chunk creation)
    export RUST_LOG="${RUST_LOG:-kix=info,kix_parser=debug,kix_embeddings=debug,warn}"

    # Ollama embedding configuration
    export OLLAMA_HOST="${OLLAMA_HOST:-http://localhost:11434}"
    export OLLAMA_MODEL="${OLLAMA_MODEL:-nomic-embed-text}"
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
    echo "=== Services Running (Local) ==="
    echo "  Web UI:   http://localhost:3000"
    echo "  API:      http://localhost:3001 (proxied via /api)"
    echo "  Indexing: http://localhost:3001/api/indexing/* (with SSE)"
    echo "  MCP:      http://localhost:3002/mcp (proxied via /mcp)"
    echo "  Ollama:   http://localhost:11434"
    echo ""
    if [ "$ARCH" = "arm64" ]; then
        echo "GPU: Apple Silicon Metal acceleration enabled"
    fi
    echo ""
    echo "Press Ctrl+C to stop all services"
    echo ""

    # Trap to kill servers when script exits
    cleanup() {
        echo ""
        echo "Stopping services..."
        kill $MCP_PID $API_PID 2>/dev/null || true
        if [ -n "$OLLAMA_PID" ]; then
            kill $OLLAMA_PID 2>/dev/null || true
        fi
    }
    trap cleanup EXIT

    # Run client dev server in foreground
    cd client
    npm run dev
}

#
# Main execution
#
case "$EXECUTION_MODE" in
    docker)
        run_docker_mode
        ;;
    local)
        run_local_mode
        ;;
esac
