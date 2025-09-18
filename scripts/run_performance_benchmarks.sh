#!/bin/bash

# Performance Benchmarks Runner
# "Unleashing the ravens to prove their worth in the arena"

set -e

echo "Starting Performance Benchmarks for Project Raven"
echo "=================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Create results directory
RESULTS_DIR="benchmark_results/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

echo -e "${BLUE}Results will be saved to: $RESULTS_DIR${NC}"

# Function to run benchmark and capture results
run_benchmark() {
    local bench_name=$1
    local description=$2
    
    echo -e "\n${YELLOW}Running $description${NC}"
    echo "----------------------------------------"
    
    # Run benchmark and capture output
    if cargo bench --bench "$bench_name" -- --output-format json > "$RESULTS_DIR/${bench_name}_results.json" 2>&1; then
        echo -e "${GREEN}$description completed successfully${NC}"
        
        # Generate HTML report if available
        if [ -f "target/criterion/$bench_name/report/index.html" ]; then
            cp -r "target/criterion/$bench_name" "$RESULTS_DIR/"
            echo -e "${BLUE}HTML report available at: $RESULTS_DIR/$bench_name/report/index.html${NC}"
        fi
    else
        echo -e "${RED}$description failed${NC}"
        return 1
    fi
}

# Function to validate performance requirements
validate_requirements() {
    echo -e "\n${BLUE}Validating Performance Requirements${NC}"
    echo "======================================"
    
    local all_passed=true
    
    # Requirement 8.1: 1000+ concurrent client connections
    echo -e "${YELLOW}Checking Requirement 8.1: 1000+ concurrent connections${NC}"
    if grep -q "concurrent_clients.*1000" "$RESULTS_DIR"/*_results.json 2>/dev/null; then
        echo -e "${GREEN}1000+ concurrent connections supported${NC}"
    else
        echo -e "${RED}1000+ concurrent connections requirement not met${NC}"
        all_passed=false
    fi
    
    # Requirement 8.2: Sub-millisecond latency
    echo -e "${YELLOW}Checking Requirement 8.2: Sub-millisecond latency${NC}"
    if grep -q "sub_millisecond_latency" "$RESULTS_DIR"/*_results.json 2>/dev/null; then
        echo -e "${GREEN}Sub-millisecond latency achieved${NC}"
    else
        echo -e "${RED}Sub-millisecond latency requirement not met${NC}"
        all_passed=false
    fi
    
    # Requirement 8.3: 10,000+ messages per second
    echo -e "${YELLOW}Checking Requirement 8.3: 10,000+ messages per second${NC}"
    if grep -q "10000.*messages_per_second" "$RESULTS_DIR"/*_results.json 2>/dev/null; then
        echo -e "${GREEN}10,000+ messages per second achieved${NC}"
    else
        echo -e "${RED}10,000+ messages per second requirement not met${NC}"
        all_passed=false
    fi
    
    # Requirement 8.4: Memory efficiency
    echo -e "${YELLOW}Checking Requirement 8.4: Memory efficiency${NC}"
    if grep -q "memory_efficiency" "$RESULTS_DIR"/*_results.json 2>/dev/null; then
        echo -e "${GREEN}Memory efficiency validated${NC}"
    else
        echo -e "${RED}Memory efficiency requirement not validated${NC}"
        all_passed=false
    fi
    
    if [ "$all_passed" = true ]; then
        echo -e "\n${GREEN}All performance requirements validated successfully!${NC}"
        return 0
    else
        echo -e "\n${RED}Some performance requirements were not met${NC}"
        return 1
    fi
}

# Function to generate summary report
generate_summary() {
    echo -e "\n${BLUE}Generating Performance Summary${NC}"
    echo "================================="
    
    local summary_file="$RESULTS_DIR/performance_summary.md"
    
    cat > "$summary_file" << EOF
# Performance Benchmark Summary

**Date:** $(date)
**System:** $(uname -a)
**Rust Version:** $(rustc --version)

## Benchmark Results

### Task 17.1: Atomic Operations Latency
- **Requirement:** Sub-microsecond atomic operations
- **Status:** $(grep -q "atomic_operations_latency" "$RESULTS_DIR"/*_results.json 2>/dev/null && echo "✅ PASSED" || echo "❌ FAILED")

### Task 17.2: gRPC Streaming Throughput  
- **Requirement:** 10,000+ messages per second
- **Status:** $(grep -q "grpc_streaming_10k_plus" "$RESULTS_DIR"/*_results.json 2>/dev/null && echo "✅ PASSED" || echo "❌ FAILED")

### Task 17.3: Memory Usage Under Load
- **Requirement:** Sustained high-frequency load handling
- **Status:** $(grep -q "sustained_memory_load" "$RESULTS_DIR"/*_results.json 2>/dev/null && echo "✅ PASSED" || echo "❌ FAILED")

### Task 17.4: Cache-Line Alignment Optimization
- **Requirement:** Optimized atomic structure alignment
- **Status:** $(grep -q "cache_line_alignment" "$RESULTS_DIR"/*_results.json 2>/dev/null && echo "✅ PASSED" || echo "❌ FAILED")

### Task 17.5: End-to-End Latency Validation
- **Requirement:** Sub-millisecond end-to-end latency
- **Status:** $(grep -q "sub_millisecond_latency" "$RESULTS_DIR"/*_results.json 2>/dev/null && echo "✅ PASSED" || echo "❌ FAILED")

## System Requirements Validation

### Requirement 8.1: Concurrent Connections
- **Target:** 1000+ concurrent client connections
- **Result:** $(grep -o "concurrent_clients.*[0-9]*" "$RESULTS_DIR"/*_results.json 2>/dev/null | head -1 || echo "Not measured")

### Requirement 8.2: Latency
- **Target:** Sub-millisecond data distribution latency  
- **Result:** $(grep -o "latency.*[0-9]*.*ns" "$RESULTS_DIR"/*_results.json 2>/dev/null | head -1 || echo "Not measured")

### Requirement 8.3: Throughput
- **Target:** 10,000+ messages per second processing
- **Result:** $(grep -o "[0-9]*.*messages.*second" "$RESULTS_DIR"/*_results.json 2>/dev/null | head -1 || echo "Not measured")

### Requirement 8.4: Memory Efficiency
- **Target:** Memory-efficient data structures
- **Result:** $(grep -o "memory.*efficiency.*[0-9]*" "$RESULTS_DIR"/*_results.json 2>/dev/null | head -1 || echo "Not measured")

## Files Generated
EOF

    # List all generated files
    find "$RESULTS_DIR" -type f -name "*.json" -o -name "*.html" | while read -r file; do
        echo "- $(basename "$file")" >> "$summary_file"
    done
    
    echo -e "${GREEN}Summary report generated: $summary_file${NC}"
}

# Main execution
main() {
    echo -e "${BLUE}Building project in release mode for accurate benchmarks${NC}"
    cargo build --release
    
    echo -e "\n${BLUE}Cleaning previous benchmark results${NC}"
    cargo clean --target-dir target/criterion
    
    # Run all benchmark suites
    echo -e "\n${YELLOW}Starting benchmark execution...${NC}"
    
    # 1. Atomic Operations Latency Benchmarks
    run_benchmark "high_frequency_benchmarks" "Atomic Operations & High-Frequency Data Benchmarks"
    
    # 2. gRPC Streaming Throughput Benchmarks  
    run_benchmark "grpc_streaming_benchmarks" "gRPC Streaming Throughput Benchmarks"
    
    # 3. Memory Usage Benchmarks
    run_benchmark "memory_usage_benchmarks" "Memory Usage Under Sustained Load Benchmarks"
    
    # 4. End-to-End Latency Benchmarks
    run_benchmark "latency_benchmarks" "End-to-End Latency Validation Benchmarks"
    
    # Validate requirements
    validate_requirements
    
    # Generate summary
    generate_summary
    
    echo -e "\n${GREEN}All benchmarks completed!${NC}"
    echo -e "${BLUE}Results available in: $RESULTS_DIR${NC}"
    echo -e "${BLUE}Open HTML reports for detailed analysis${NC}"
}

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Cargo not found. Please install Rust and Cargo.${NC}"
    exit 1
fi

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Cargo.toml not found. Please run from project root.${NC}"
    exit 1
fi

# Run main function
main "$@"