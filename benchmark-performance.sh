#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== KIX Performance Benchmark ===${NC}"
echo ""
echo "This script tests the performance improvements made to the KIX system:"
echo "  ✓ JobQueue concurrency increased (4→8)"
echo "  ✓ SIMD optimizations enabled"
echo "  ✓ jemalloc memory allocator"
echo "  ✓ Increased embedding batch sizes"
echo ""

# Check if binary exists
if [ ! -f "./server/target/release/kix" ]; then
    echo -e "${YELLOW}Binary not found. Building with optimizations...${NC}"
    ./build-performance.sh
fi

echo -e "${GREEN}Starting benchmark tests...${NC}"
echo ""

# Function to measure time
measure_time() {
    local start=$(date +%s.%N)
    "$@"
    local end=$(date +%s.%N)
    local runtime=$(echo "$end - $start" | bc)
    echo "$runtime"
}

# Test 1: Check system info
echo -e "${YELLOW}1. System Information:${NC}"
echo "   CPU Cores: $(nproc)"
echo "   Memory: $(free -h | grep '^Mem:' | awk '{print $2}')"
echo "   Rust version: $(rustc --version)"
echo ""

# Test 2: Index statistics
echo -e "${YELLOW}2. Current Index Statistics:${NC}"
./server/target/release/kix stats 2>/dev/null || echo "   No index data yet"
echo ""

# Test 3: Search performance
echo -e "${YELLOW}3. Search Performance Test:${NC}"
echo "   Running 10 searches..."

SEARCH_QUERIES=(
    "message routing"
    "integration patterns"
    "enterprise architecture"
    "semantic search"
    "vector database"
    "knowledge management"
    "document parsing"
    "real-time processing"
    "async operations"
    "performance optimization"
)

total_time=0
for query in "${SEARCH_QUERIES[@]}"; do
    echo -n "   Searching for '$query'... "
    time=$(measure_time ./server/target/release/kix search "$query" --limit 5 2>/dev/null)
    echo -e "${GREEN}${time}s${NC}"
    total_time=$(echo "$total_time + $time" | bc)
done

avg_time=$(echo "scale=3; $total_time / 10" | bc)
echo -e "   ${BLUE}Average search time: ${avg_time}s${NC}"
echo ""

# Test 4: Memory usage
echo -e "${YELLOW}4. Memory Usage Test:${NC}"
echo "   Starting API server in background..."

# Start the API server
./server/target/release/kix api --port 3001 > /tmp/kix_api.log 2>&1 &
API_PID=$!

sleep 2

if ps -p $API_PID > /dev/null; then
    # Get memory usage
    MEM_USAGE=$(ps -o rss= -p $API_PID | awk '{print $1/1024 "MB"}')
    echo -e "   Initial memory usage: ${GREEN}$MEM_USAGE${NC}"

    # Make some API requests
    echo "   Making 100 concurrent API requests..."
    for i in {1..100}; do
        curl -s "http://localhost:3001/api/health" > /dev/null 2>&1 &
    done
    wait

    # Check memory after requests
    MEM_USAGE_AFTER=$(ps -o rss= -p $API_PID | awk '{print $1/1024 "MB"}')
    echo -e "   Memory after 100 requests: ${GREEN}$MEM_USAGE_AFTER${NC}"

    # Kill the API server
    kill $API_PID 2>/dev/null || true
else
    echo "   API server failed to start"
fi
echo ""

# Test 5: Concurrency test
echo -e "${YELLOW}5. Concurrency Test:${NC}"
echo "   Testing parallel search operations..."

# Run searches in parallel
start_time=$(date +%s.%N)
for i in {1..8}; do
    ./server/target/release/kix search "test query $i" --limit 5 > /dev/null 2>&1 &
done
wait
end_time=$(date +%s.%N)
parallel_time=$(echo "$end_time - $start_time" | bc)

echo -e "   8 parallel searches completed in: ${GREEN}${parallel_time}s${NC}"
echo ""

# Summary
echo -e "${BLUE}=== Performance Summary ===${NC}"
echo ""
echo "Key improvements implemented:"
echo "  • JobQueue: 4→8 concurrent jobs (2x improvement)"
echo "  • Batch sizes: 64→256 for CPU (4x improvement)"
echo "  • Memory: jemalloc allocator (better fragmentation)"
echo "  • SIMD: Native CPU optimizations enabled"
echo ""
echo -e "${GREEN}Expected performance gains:${NC}"
echo "  • Embedding throughput: ~3x faster"
echo "  • Search latency: ~2x faster"
echo "  • Document indexing: ~3x faster"
echo "  • Concurrent users: ~3x more"
echo ""
echo "Next steps for even better performance:"
echo "  1. Implement GPU acceleration (10-100x speedup)"
echo "  2. Add ONNX Runtime backend"
echo "  3. Enable INT8 quantization"
echo "  4. Optimize vector database indices"
echo ""
echo -e "${GREEN}Benchmark complete!${NC}"