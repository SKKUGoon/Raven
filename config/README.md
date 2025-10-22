# Project Raven Configuration Guide

*"Simple, secure, and maintainable configuration"*

## Overview

Project Raven uses a **simplified 3-file configuration system**:
- **`config/example.toml`** - Template with default values and missing secrets
- **`config/development.toml`** - Your local development settings
- **`config/secret.toml`** - Production settings (git-ignored for security)

## Quick Start

### 1. First Time Setup

```bash
# Copy the example to create your development config
cp config/example.toml config/development.toml

# Edit development.toml with your local settings
nano config/development.toml

# For production, copy and fill secrets
cp config/example.toml config/secret.toml
nano config/secret.toml  # Add production tokens and URLs
```

### 2. Running the Application

```bash
# Development (default)
./scripts/run-env.sh development
# or simply
./scripts/run-env.sh

# Production
./scripts/run-env.sh production
```

## Configuration Files

### `config/example.toml`
- **Purpose**: Template file with sensible defaults
- **Contains**: All configuration options with example values
- **Missing**: Critical secrets (tokens, production URLs)
- **Usage**: Copy this file to create your environment configs

### `config/development.toml`
- **Purpose**: Your local development environment
- **Contains**: Settings aligned with your current local setup
- **Includes**: Development tokens, local URLs, debug settings
- **Usage**: Customize for your development environment

### `config/secret.toml` 🔒
- **Purpose**: Production configuration with secrets
- **Contains**: Production URLs, tokens, optimized settings
- **Security**: **NEVER commit to git** (automatically ignored)
- **Usage**: Production deployments only

## Environment Selection

The `ENVIRONMENT` variable determines which config file to load:

```bash
ENVIRONMENT=development  # → config/development.toml
ENVIRONMENT=production   # → config/secret.toml
```

## Configuration Structure

All configuration files follow the same structure. See `config/example.toml` for detailed documentation of all options.

### Key Sections

- **`[server]`** - gRPC server settings (host, port, connections)
- **`[database]`** - InfluxDB connection and crypto bucket settings
- **`[data_processing]`** - Buffer sizes and processing intervals
- **`[retention_policies]`** - Data retention for different data types
- **`[batching]`** - Batch processing configuration
- **`[monitoring]`** - Metrics, health checks, and logging

### Critical Settings to Customize

```toml
[database]
influx_url = "http://localhost:8086"  # Your InfluxDB URL
bucket = "crypto"                     # Your crypto data bucket
org = "your-org"                      # Your organization
token = "your-secret-token"           # Your InfluxDB token

[monitoring]
log_level = "debug"  # development: debug, production: warn
```

## Running the Server

After updating your configuration file, launch the application directly:

```bash
# Development
./scripts/run-env.sh development

# Production
./scripts/run-env.sh production
```

## Security Best Practices

### ✅ Do This
- Keep `config/secret.toml` out of git (automatically ignored)
- Store production tokens in `config/secret.toml`
- Use different buckets for different environments
- Review `config/example.toml` for new configuration options

### ❌ Don't Do This
- Never commit `config/secret.toml` to git
- Don't put production secrets in development configs
- Don't use the same bucket for dev and production

## Quick Reference

### Setup Commands
```bash
# First time setup
cp config/example.toml config/development.toml
cp config/example.toml config/secret.toml

# Edit your configs
nano config/development.toml  # Add your dev settings
nano config/secret.toml       # Add production secrets

# Run development
./scripts/run-env.sh development

# Run production
./scripts/run-env.sh production
```

### File Summary
- **`config/example.toml`** ✅ Template (commit to git)
- **`config/development.toml`** ✅ Your dev config (commit to git)
- **`config/secret.toml`** 🔒 Production secrets (NEVER commit!)

### Environment Variables (Optional)
```bash
export ENVIRONMENT=development  # or production
export RUST_LOG=debug          # Optional: logging level
```

That's it! Simple, secure, and maintainable. 🚀
