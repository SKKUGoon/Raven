#!/bin/bash

# Project Raven Health Check Script
# Used by Docker containers to verify service health

set -e

# Configuration
HEALTH_URL="http://localhost:8080/health"
METRICS_URL="http://localhost:9090/metrics"
TIMEOUT=10
MAX_RETRIES=3

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to check HTTP endpoint
check_http_endpoint() {
    local url=$1
    local name=$2
    local timeout=${3:-$TIMEOUT}
    
    if curl -sf --max-time $timeout "$url" >/dev/null 2>&1; then
        echo -e "${GREEN}✅ $name is healthy${NC}"
        return 0
    else
        echo -e "${RED}❌ $name is not responding${NC}"
        return 1
    fi
}

# Function to check gRPC endpoint
check_grpc_endpoint() {
    local host=${1:-localhost}
    local port=${2:-50051}
    
    # Use netcat to check if gRPC port is open
    if nc -z "$host" "$port" 2>/dev/null; then
        echo -e "${GREEN}✅ gRPC server is listening on $host:$port${NC}"
        return 0
    else
        echo -e "${RED}❌ gRPC server is not listening on $host:$port${NC}"
        return 1
    fi
}

# Function to check memory usage
check_memory_usage() {
    local max_memory_mb=${1:-2048}
    
    # Get memory usage in MB
    local memory_kb=$(grep VmRSS /proc/self/status | awk '{print $2}')
    local memory_mb=$((memory_kb / 1024))
    
    if [ $memory_mb -lt $max_memory_mb ]; then
        echo -e "${GREEN}✅ Memory usage: ${memory_mb}MB (limit: ${max_memory_mb}MB)${NC}"
        return 0
    else
        echo -e "${YELLOW}⚠️  Memory usage: ${memory_mb}MB (limit: ${max_memory_mb}MB)${NC}"
        return 1
    fi
}

# Function to check disk space
check_disk_space() {
    local min_free_percent=${1:-10}
    
    # Get disk usage percentage for root filesystem
    local disk_usage=$(df / | tail -1 | awk '{print $5}' | sed 's/%//')
    local free_percent=$((100 - disk_usage))
    
    if [ $free_percent -gt $min_free_percent ]; then
        echo -e "${GREEN}✅ Disk space: ${free_percent}% free${NC}"
        return 0
    else
        echo -e "${YELLOW}⚠️  Disk space: ${free_percent}% free (minimum: ${min_free_percent}%)${NC}"
        return 1
    fi
}

# Main health check function
main_health_check() {
    local exit_code=0
    
    echo "🐦 Project Raven Health Check"
    echo "============================="
    
    # Check HTTP health endpoint
    if ! check_http_endpoint "$HEALTH_URL" "Health endpoint"; then
        exit_code=1
    fi
    
    # Check metrics endpoint
    if ! check_http_endpoint "$METRICS_URL" "Metrics endpoint"; then
        exit_code=1
    fi
    
    # Check gRPC endpoint
    if ! check_grpc_endpoint "localhost" "50051"; then
        exit_code=1
    fi
    
    # Check memory usage
    if ! check_memory_usage 2048; then
        # Memory warning doesn't fail health check, just warns
        :
    fi
    
    # Check disk space
    if ! check_disk_space 10; then
        # Disk space warning doesn't fail health check, just warns
        :
    fi
    
    echo "============================="
    
    if [ $exit_code -eq 0 ]; then
        echo -e "${GREEN}🎉 All health checks passed${NC}"
    else
        echo -e "${RED}💥 Some health checks failed${NC}"
    fi
    
    return $exit_code
}

# Enhanced health check for specific services
service_health_check() {
    local service=$1
    
    case $service in
        "influxdb")
            check_http_endpoint "http://localhost:8086/ping" "InfluxDB"
            ;;
        "redis")
            if redis-cli ping 2>/dev/null | grep -q PONG; then
                echo -e "${GREEN}✅ Redis is responding${NC}"
                return 0
            else
                echo -e "${RED}❌ Redis is not responding${NC}"
                return 1
            fi
            ;;
        "prometheus")
            check_http_endpoint "http://localhost:9090/-/healthy" "Prometheus"
            ;;
        "grafana")
            check_http_endpoint "http://localhost:3000/api/health" "Grafana"
            ;;
        "jaeger")
            check_http_endpoint "http://localhost:14269/" "Jaeger"
            ;;
        *)
            main_health_check
            ;;
    esac
}

# Parse command line arguments
case "${1:-main}" in
    "main"|"")
        main_health_check
        ;;
    "influxdb"|"redis"|"prometheus"|"grafana"|"jaeger")
        service_health_check "$1"
        ;;
    "quick")
        # Quick health check - just HTTP endpoints
        check_http_endpoint "$HEALTH_URL" "Health endpoint" 5
        ;;
    "detailed")
        # Detailed health check with all components
        main_health_check
        echo ""
        echo "📊 Additional System Information:"
        echo "================================="
        echo "Uptime: $(uptime)"
        echo "Load Average: $(cat /proc/loadavg)"
        echo "Memory: $(free -h | grep Mem)"
        echo "Disk: $(df -h / | tail -1)"
        ;;
    *)
        echo "Usage: $0 [main|quick|detailed|influxdb|redis|prometheus|grafana|jaeger]"
        echo ""
        echo "Health check modes:"
        echo "  main      - Standard health check (default)"
        echo "  quick     - Quick HTTP endpoint check only"
        echo "  detailed  - Detailed health check with system info"
        echo ""
        echo "Service-specific checks:"
        echo "  influxdb  - Check InfluxDB health"
        echo "  redis     - Check Redis health"
        echo "  prometheus- Check Prometheus health"
        echo "  grafana   - Check Grafana health"
        echo "  jaeger    - Check Jaeger health"
        exit 1
        ;;
esac