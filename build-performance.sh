#!/bin/bash

set -e

echo "=== Building KIX with Performance Optimizations ==="
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Applying performance optimizations:${NC}"
echo "  - SIMD instructions (target-cpu=native)"
echo "  - Link-time optimization (lto=fat)"
echo "  - Code generation units = 1"
echo ""

# Build with all performance optimizations
echo -e "${GREEN}Building Rust binary with maximum performance...${NC}"
RUSTFLAGS="-C target-cpu=native -C lto=fat -C codegen-units=1" \
    cargo build --manifest-path server/Cargo.toml --release

echo ""
echo -e "${GREEN}=== Performance build complete ===${NC}"
echo ""
echo "Run with: ./server/target/release/kix"
echo ""
echo "Quick performance test commands:"
echo "  # Start API server"
echo "  ./server/target/release/kix api --port 3001"
echo ""
echo "  # Check index statistics"
echo "  ./server/target/release/kix stats"
echo ""
echo "Note: The JobQueue bottleneck has been fixed (4→8 concurrent jobs)"
echo "      This alone should provide ~2x throughput improvement!"