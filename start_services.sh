#!/bin/bash

# Configuration
LOG_DIR="$HOME/.raven/log"
mkdir -p "$LOG_DIR"

echo "Starting Raven services..."

# Function to start a service
# Usage: start_service binary_name friendly_name [arguments...]
start_service() {
    local bin=$1
    local name=$2
    shift 2
    local args="$@"
    
    echo "Starting $name..."
    nohup ./target/release/$bin $args > "$LOG_DIR/$name.log" 2>&1 &
    echo $! > "$LOG_DIR/$name.pid"
}

# 1. Start Sources (Collectors)
start_service "binance_spot" "binance_spot"
start_service "binance_futures" "binance_futures"

# 2. Start Aggregators
# Default 1m bars on port 50051 (from config)
start_service "raven_timebar" "timebar_1m" "--seconds 60"

# 1s bars on port 50053
start_service "raven_timebar" "timebar_1s" "--seconds 1 --port 50053"

# Tibs on default port 50052
start_service "raven_tibs" "tibs"

# 3. Start Persistence
start_service "tick_persistence" "tick_persistence"
start_service "bar_persistence" "bar_persistence"

echo "All services started. PIDs stored in $LOG_DIR/"
echo "Check logs in $LOG_DIR/ for output."
