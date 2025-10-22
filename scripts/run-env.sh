#!/bin/bash
# Project Raven Environment Runner
# "Choose your realm wisely"

set -e

ENVIRONMENT=${1:-development}

echo "🏰 Starting Project Raven in $ENVIRONMENT environment..."

# Validate environment and determine config file
case $ENVIRONMENT in
    development)
        CONFIG_FILE="config/development.toml"
        echo "✅ Environment: Development"
        ;;
    production)
        CONFIG_FILE="config/secret.toml"
        echo "✅ Environment: Production (using secret.toml)"
        ;;
    *)
        echo "❌ Invalid environment: $ENVIRONMENT"
        echo "Valid options: development, production"
        exit 1
        ;;
esac

# Check if config file exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo "❌ Configuration file not found: $CONFIG_FILE"
    if [ "$ENVIRONMENT" = "production" ]; then
        echo "💡 Hint: Copy config/example.toml to config/secret.toml and fill in production values"
    else
        echo "💡 Hint: Copy config/example.toml to $CONFIG_FILE and customize for development"
    fi
    exit 1
fi

echo "📜 Using config: $CONFIG_FILE"

# Set environment and run
export ENVIRONMENT=$ENVIRONMENT

# Run the application
echo "🚀 Launching Raven..."
cargo run --bin raven
