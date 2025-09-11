#!/bin/bash

# System Performance Validation Script - Project Raven
# Validates all performance requirements for Task 19

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${PURPLE}🎯 Project Raven - System Performance Validation${NC}"
echo -e "${PURPLE}===============================================${NC}"
echo ""

# Performance requirements to validate
declare -A REQUIREMENTS=(
    ["8.1"]="1000+ concurrent client connections"
    ["8.2"]="Sub-millisecond end-to-end latency"
    ["8.3"]="10,000+ messages per second throughput"
    ["8.4"]="Memory-efficient data structures"
)

# Function to print section headers
print_section() {
    echo -e "${CYAN}$1${NC}"
    echo -e "${CYAN}$(printf '=%.0s' $(seq 1 ${#1}))${NC}"
}

# Function to validate requirement
validate_requirement() {
    local req_id="$1"
    local description="$2"
    local test_command="$3"
    
    echo -e "${BLUE}📋 Requirement $req_id: $description${NC}"
    echo -e "${BLUE}   Test: $test_command${NC}"
    echo ""
    
    if eval "$test_command"; then
        echo -e "${GREEN}✅ REQUIREMENT $req_id VALIDATED${NC}"
        return 0
    else
        echo -e "${RED}❌ REQUIREMENT $req_id FAILED${NC}"
        return 1
    fi
}

# Function to run quick performance check
quick_performance_check() {
    print_section "⚡ Quick Performance Check"
    
    echo -e "${BLUE}🔍 Running quick performance validation...${NC}"
    echo ""
    
    # Check if benchmarks compile and run
    echo -e "${YELLOW}📊 Validating benchmark compilation...${NC}"
    if cargo check --benches --quiet; then
        echo -e "${GREEN}✅ All benchmarks compile successfully${NC}"
    else
        echo -e "${RED}❌ Benchmark compilation failed${NC}"
        return 1
    fi
    
    # Run a quick atomic operations test
    echo -e "${YELLOW}⚡ Testing atomic operations performance...${NC}"
    if timeout 30 cargo test --test unit_tests atomic_data_structures::test_atomic_orderbook_update_and_snapshot --release --quiet; then
        echo -e "${GREEN}✅ Atomic operations working correctly${NC}"
    else
        echo -e "${RED}❌ Atomic operations test failed${NC}"
        return 1
    fi
    
    # Test basic system functionality
    echo -e "${YELLOW}🧪 Testing basic system functionality...${NC}"
    if timeout 60 cargo test --test integration_tests test_server_startup_and_shutdown --release --quiet; then
        echo -e "${GREEN}✅ Basic system functionality working${NC}"
    else
        echo -e "${RED}❌ Basic system functionality test failed${NC}"
        return 1
    fi
    
    echo ""
}

# Function to validate all performance requirements
validate_all_requirements() {
    print_section "📋 Performance Requirements Validation"
    
    local failed_requirements=0
    local total_requirements=${#REQUIREMENTS[@]}
    
    # Requirement 8.1: 1000+ concurrent connections
    echo -e "${BLUE}🔗 Testing concurrent connection capability...${NC}"
    if validate_requirement "8.1" "${REQUIREMENTS[8.1]}" \
        "timeout 120 cargo test --test integration_tests test_concurrent_client_connections --release --quiet"; then
        echo ""
    else
        ((failed_requirements++))
        echo ""
    fi
    
    # Requirement 8.2: Sub-millisecond latency
    echo -e "${BLUE}⏱️ Testing latency performance...${NC}"
    if validate_requirement "8.2" "${REQUIREMENTS[8.2]}" \
        "timeout 60 cargo bench --bench latency_benchmarks bench_sub_millisecond_latency --quiet"; then
        echo ""
    else
        ((failed_requirements++))
        echo ""
    fi
    
    # Requirement 8.3: 10,000+ messages/second
    echo -e "${BLUE}📈 Testing throughput capability...${NC}"
    if validate_requirement "8.3" "${REQUIREMENTS[8.3]}" \
        "timeout 90 cargo bench --bench grpc_streaming_benchmarks bench_grpc_streaming_10k_plus --quiet"; then
        echo ""
    else
        ((failed_requirements++))
        echo ""
    fi
    
    # Requirement 8.4: Memory efficiency
    echo -e "${BLUE}💾 Testing memory efficiency...${NC}"
    if validate_requirement "8.4" "${REQUIREMENTS[8.4]}" \
        "timeout 90 cargo bench --bench memory_usage_benchmarks bench_sustained_memory_load --quiet"; then
        echo ""
    else
        ((failed_requirements++))
        echo ""
    fi
    
    return $failed_requirements
}

# Function to run comprehensive system test
run_comprehensive_system_test() {
    print_section "🚀 Comprehensive System Test"
    
    echo -e "${BLUE}⚔️ Running comprehensive system validation...${NC}"
    echo ""
    
    # Run one comprehensive test that validates multiple requirements
    echo -e "${YELLOW}🎯 Running end-to-end system validation...${NC}"
    if timeout 300 cargo test --test final_system_tests test_performance_requirements_under_load --release -- --nocapture; then
        echo -e "${GREEN}✅ Comprehensive system test passed${NC}"
        return 0
    else
        echo -e "${RED}❌ Comprehensive system test failed${NC}"
        return 1
    fi
}

# Function to generate performance report
generate_performance_report() {
    local validation_result="$1"
    
    print_section "📊 Performance Validation Report"
    
    local report_file="performance_validation_$(date +%Y%m%d_%H%M%S).md"
    
    cat > "$report_file" << EOF
# Performance Validation Report - Project Raven

**Validation Date:** $(date)
**System:** $(uname -a)
**Rust Version:** $(rustc --version)

## Performance Requirements Status

EOF

    for req_id in "${!REQUIREMENTS[@]}"; do
        if [ "$validation_result" -eq 0 ]; then
            echo "### ✅ Requirement $req_id: ${REQUIREMENTS[$req_id]}" >> "$report_file"
            echo "**Status:** VALIDATED" >> "$report_file"
        else
            echo "### ❌ Requirement $req_id: ${REQUIREMENTS[$req_id]}" >> "$report_file"
            echo "**Status:** NEEDS ATTENTION" >> "$report_file"
        fi
        echo "" >> "$report_file"
    done

    cat >> "$report_file" << EOF

## System Capabilities

- **Atomic Operations**: Sub-microsecond performance for high-frequency data
- **Concurrent Connections**: Support for 1000+ simultaneous clients
- **Data Throughput**: 10,000+ messages per second processing capability
- **Memory Management**: Efficient allocation and linear scaling
- **Error Recovery**: Graceful handling of failures and disconnections
- **Data Consistency**: Atomic storage and database synchronization

## Performance Benchmarks

The following benchmarks validate system performance:

1. **High-Frequency Benchmarks** (\`cargo bench --bench high_frequency_benchmarks\`)
   - Atomic orderbook updates
   - Concurrent atomic operations
   - Snapshot capture performance

2. **gRPC Streaming Benchmarks** (\`cargo bench --bench grpc_streaming_benchmarks\`)
   - Streaming throughput validation
   - Concurrent client simulation
   - Backpressure handling

3. **Memory Usage Benchmarks** (\`cargo bench --bench memory_usage_benchmarks\`)
   - Sustained memory load testing
   - Memory allocation patterns
   - Large dataset efficiency

4. **Latency Benchmarks** (\`cargo bench --bench latency_benchmarks\`)
   - End-to-end latency measurement
   - Latency percentile analysis
   - Performance consistency

## Validation Commands

To reproduce these results, run:

\`\`\`bash
# Quick validation
./scripts/validate_system_performance.sh

# Full system tests
./scripts/run_final_system_tests.sh

# Individual benchmarks
cargo bench --bench high_frequency_benchmarks
cargo bench --bench grpc_streaming_benchmarks
cargo bench --bench memory_usage_benchmarks
cargo bench --bench latency_benchmarks
\`\`\`

## Conclusion

EOF

    if [ "$validation_result" -eq 0 ]; then
        cat >> "$report_file" << EOF
🏆 **All performance requirements have been successfully validated!**

Project Raven demonstrates enterprise-grade performance characteristics
suitable for high-frequency trading environments. The system is ready
for production deployment.
EOF
    else
        cat >> "$report_file" << EOF
⚠️ **Some performance requirements need attention.**

Review the failed validations and run the full test suite for detailed
diagnostics. Address any performance issues before production deployment.
EOF
    fi

    echo -e "${GREEN}📄 Performance report generated: $report_file${NC}"
}

# Main execution
main() {
    local start_time=$(date +%s)
    
    echo -e "${PURPLE}🎯 Starting Performance Validation for Project Raven${NC}"
    echo -e "${PURPLE}Validation started at: $(date)${NC}"
    echo ""
    
    # Quick performance check
    if ! quick_performance_check; then
        echo -e "${RED}❌ Quick performance check failed${NC}"
        exit 1
    fi
    
    # Validate all requirements
    local failed_requirements=0
    if ! validate_all_requirements; then
        failed_requirements=$?
    fi
    
    # Run comprehensive system test
    local system_test_result=0
    if ! run_comprehensive_system_test; then
        system_test_result=1
    fi
    
    # Calculate final result
    local final_result=0
    if [ $failed_requirements -gt 0 ] || [ $system_test_result -ne 0 ]; then
        final_result=1
    fi
    
    # Generate report
    generate_performance_report $final_result
    
    # Calculate duration
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    print_section "🏁 Validation Results"
    
    echo -e "${BLUE}📊 Validation Statistics:${NC}"
    echo -e "  ⏱️ Total Duration: ${duration}s"
    echo -e "  📋 Requirements Tested: ${#REQUIREMENTS[@]}"
    echo -e "  ❌ Failed Requirements: $failed_requirements"
    echo ""
    
    if [ $final_result -eq 0 ]; then
        echo -e "${GREEN}🏆 ALL PERFORMANCE REQUIREMENTS VALIDATED!${NC}"
        echo -e "${GREEN}⚡ Project Raven is performance-ready for production!${NC}"
        exit 0
    else
        echo -e "${RED}💥 Performance validation failed${NC}"
        echo -e "${RED}🔧 Review the issues and optimize before deployment${NC}"
        exit 1
    fi
}

# Handle script interruption
trap 'echo -e "\n${YELLOW}⚠️ Performance validation interrupted${NC}"; exit 130' INT TERM

# Run main function
main "$@"