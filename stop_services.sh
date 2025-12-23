#!/bin/bash

LOG_DIR="$HOME/.raven/log"

if [ ! -d "$LOG_DIR" ]; then
    echo "No logs directory found."
    exit 0
fi

echo "Stopping Raven services..."

# Check if there are any pid files
if [ ! "$(ls -A $LOG_DIR/*.pid 2>/dev/null)" ]; then
    echo "No running services found (no .pid files)."
    exit 0
fi

for pid_file in "$LOG_DIR"/*.pid; do
    if [ -f "$pid_file" ]; then
        pid=$(cat "$pid_file")
        name=$(basename "$pid_file" .pid)
        
        if ps -p $pid > /dev/null; then
            echo "Killing $name (PID: $pid)..."
            kill $pid
        else
            echo "$name (PID: $pid) is not running. Cleaning up."
        fi
        rm "$pid_file"
    fi
done

echo "All services stopped."

