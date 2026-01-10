#!/bin/bash
#
# KIX Ollama Setup Script
#
# This script sets up Ollama with the nomic-embed-text model for KIX embeddings.
# It works on both macOS and Linux, automatically using GPU acceleration when available.
#
# Usage:
#   ./scripts/setup-ollama.sh
#
# Requirements:
#   - curl
#   - Ollama (will be installed if not present on macOS)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== KIX Ollama Setup ===${NC}"
echo ""

# Detect OS
OS="$(uname -s)"
ARCH="$(uname -m)"

echo -e "${BLUE}Detected:${NC} $OS ($ARCH)"

# Set Ollama host (can be overridden)
OLLAMA_HOST="${OLLAMA_HOST:-http://localhost:11434}"
OLLAMA_MODEL="${OLLAMA_MODEL:-nomic-embed-text}"

# Function to check if Ollama is running
check_ollama() {
    curl -sf "$OLLAMA_HOST/api/version" > /dev/null 2>&1
}

# Function to install Ollama on macOS
install_ollama_macos() {
    echo -e "${YELLOW}Installing Ollama via Homebrew...${NC}"
    if command -v brew &> /dev/null; then
        brew install ollama
    else
        echo -e "${RED}Homebrew not found. Please install Ollama manually:${NC}"
        echo "  https://ollama.ai/download"
        exit 1
    fi
}

# Function to install Ollama on Linux
install_ollama_linux() {
    echo -e "${YELLOW}Installing Ollama...${NC}"
    curl -fsSL https://ollama.ai/install.sh | sh
}

# Check if Ollama is installed
if ! command -v ollama &> /dev/null; then
    echo -e "${YELLOW}Ollama not found. Installing...${NC}"
    case "$OS" in
        Darwin)
            install_ollama_macos
            ;;
        Linux)
            install_ollama_linux
            ;;
        *)
            echo -e "${RED}Unsupported OS: $OS${NC}"
            echo "Please install Ollama manually: https://ollama.ai/download"
            exit 1
            ;;
    esac
    echo -e "${GREEN}Ollama installed successfully!${NC}"
fi

# Check if Ollama is running, start if not
echo ""
echo -e "${BLUE}Checking Ollama service...${NC}"

if check_ollama; then
    echo -e "${GREEN}Ollama is already running at $OLLAMA_HOST${NC}"
else
    echo -e "${YELLOW}Starting Ollama service...${NC}"

    # Start Ollama in the background
    ollama serve &
    OLLAMA_PID=$!

    # Wait for Ollama to be ready
    echo "Waiting for Ollama to start..."
    max_attempts=30
    attempt=0
    while ! check_ollama; do
        attempt=$((attempt + 1))
        if [ $attempt -ge $max_attempts ]; then
            echo -e "${RED}ERROR: Ollama failed to start after ${max_attempts} attempts${NC}"
            kill $OLLAMA_PID 2>/dev/null || true
            exit 1
        fi
        echo "  Attempt $attempt/$max_attempts..."
        sleep 2
    done

    echo -e "${GREEN}Ollama started successfully (PID: $OLLAMA_PID)${NC}"
fi

# Get Ollama version
VERSION=$(curl -sf "$OLLAMA_HOST/api/version" | grep -o '"version":"[^"]*"' | cut -d'"' -f4)
echo -e "${BLUE}Ollama version:${NC} $VERSION"

# Check GPU availability
echo ""
echo -e "${BLUE}GPU Detection:${NC}"
case "$OS" in
    Darwin)
        if [ "$ARCH" = "arm64" ]; then
            echo -e "  ${GREEN}Apple Silicon detected - Metal GPU acceleration enabled${NC}"
        else
            echo -e "  ${YELLOW}Intel Mac - CPU mode${NC}"
        fi
        ;;
    Linux)
        if command -v nvidia-smi &> /dev/null; then
            echo -e "  ${GREEN}NVIDIA GPU detected - CUDA acceleration enabled${NC}"
            nvidia-smi --query-gpu=name,memory.total --format=csv,noheader 2>/dev/null || true
        else
            echo -e "  ${YELLOW}No NVIDIA GPU detected - CPU mode${NC}"
        fi
        ;;
esac

# Check if model exists, pull if needed
echo ""
echo -e "${BLUE}Checking for $OLLAMA_MODEL model...${NC}"

if curl -sf "$OLLAMA_HOST/api/tags" | grep -q "$OLLAMA_MODEL"; then
    echo -e "${GREEN}Model $OLLAMA_MODEL is already installed${NC}"
else
    echo -e "${YELLOW}Pulling $OLLAMA_MODEL model (this may take a few minutes)...${NC}"
    ollama pull $OLLAMA_MODEL
    echo -e "${GREEN}Model pull complete!${NC}"
fi

# Test embedding generation
echo ""
echo -e "${BLUE}Testing embedding generation...${NC}"

RESULT=$(curl -sf "$OLLAMA_HOST/api/embed" -d "{
    \"model\": \"$OLLAMA_MODEL\",
    \"input\": [\"test embedding\"]
}")

if echo "$RESULT" | grep -q "embeddings"; then
    # Count dimensions (number of floats in the embedding)
    DIM=$(echo "$RESULT" | grep -o '\[[-0-9.e,]*\]' | head -1 | tr ',' '\n' | wc -l | tr -d ' ')
    echo -e "${GREEN}Embedding test successful!${NC}"
    echo -e "  Dimensions: $DIM"
else
    echo -e "${RED}ERROR: Embedding test failed${NC}"
    echo "$RESULT"
    exit 1
fi

# Print summary
echo ""
echo -e "${GREEN}=== Setup Complete ===${NC}"
echo ""
echo "Ollama is configured with $OLLAMA_MODEL ($DIM dimensions)"
echo ""
echo -e "${BLUE}Environment variables for KIX:${NC}"
echo "  export OLLAMA_HOST=$OLLAMA_HOST"
echo "  export OLLAMA_MODEL=$OLLAMA_MODEL"
echo "  export KIX_EMBEDDING_DIM=$DIM"
echo ""
echo -e "${BLUE}To start KIX with Ollama:${NC}"
echo "  ./run.sh"
echo ""
echo -e "${BLUE}To run Ollama as a background service:${NC}"
case "$OS" in
    Darwin)
        echo "  brew services start ollama"
        ;;
    Linux)
        echo "  sudo systemctl enable ollama"
        echo "  sudo systemctl start ollama"
        ;;
esac
