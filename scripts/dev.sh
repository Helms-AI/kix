#!/bin/bash
# Development script to run both backend and frontend servers

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

echo -e "${GREEN}Starting EIP Knowledge System development servers...${NC}"

# Start backend in background
echo -e "${GREEN}Starting backend API on port 3001...${NC}"
cargo run --manifest-path server/Cargo.toml -- api --port 3001 &
BACKEND_PID=$!

# Wait for backend to start
sleep 3

# Start frontend
echo -e "${GREEN}Starting frontend on port 3000...${NC}"
cd client && npm run dev &
FRONTEND_PID=$!

# Trap to cleanup on exit
cleanup() {
    echo -e "${RED}Shutting down servers...${NC}"
    kill $BACKEND_PID 2>/dev/null
    kill $FRONTEND_PID 2>/dev/null
    exit 0
}

trap cleanup SIGINT SIGTERM

# Wait for both processes
wait $BACKEND_PID $FRONTEND_PID
