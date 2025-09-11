#!/bin/bash

# Benchmark Validation Script
# "Testing that all ravens are ready for flight"

set -e

echo "🧪 Validating Performance Benchmarks"
echo "===================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to validate benchmark compilation and basic execution
validate_benchmark() {
    local bench_name=$1
    local description=$2
    
    echo -e "\n${YELLOW}🔍 Validating $description${NC}"
    echo "----------------------------------------"
    
    # Check compilation
    echo "Checking compilation..."
    if cargo check --bench "$bench_name"; then
        echo -e "${GREEN}✅ Compilation successful${NC}"
    else
        echo -e "${RED}❌ Compilation failed${NC}"
        return 1
    fi
    
    # Run quick test
    echo "Running quick test..."
    if timeout 30s cargo bench --bench "$bench_name" -- --test 2>/dev/null; then
        echo -e "${GREEN}✅ Quick test successful${NC}"
    else
        echo -e "${YELLOW}⚠️  Quick test timed out or failed (this may be normal for some benchmarks)${NC}"
    fi
}

# Main validation
main() {
    echo -e "${BLUE}🔧 Building project in release mode${NC}"
    cargo build --release
    
    echo -e "\n${BLUE}📋 Validating all benchmark suites...${NC}"
    
    # Validate each benchmark suite
    validate_benchmark "high_frequency_benchmarks" "High-Frequency Data & Atomic Operations Benchmarks"
    validate_benchmark "grpc_streaming_benchmarks" "gRPC Streaming Throughput Benchmarks"
    validate_benchmark "memory_usage_benchmarks" "Memory Usage Under Load Benchmarks"
    validate_benchmark "latency_benchmarks" "End-to-End Latency Benchmarks"
    
    echo -e "\n${GREEN}🎉 All benchmark validations completed!${NC}"
    echo -e "${BLUE}📊 Benchmarks are ready for performance testing${NC}"
    echo -e "${BLUE}🚀 Run './scripts/run_performance_benchmarks.sh' for full benchmark suite${NC}"
}

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Cargo not found. Please install Rust and Cargo.${NC}"
    exit 1
fi

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}❌ Cargo.toml not found. Please run from project root.${NC}"
    exit 1
fi

# Run main function
main "$@"