# Project Raven Configuration Guide

*"The wisdom that guides the realm"*

## Overview

Project Raven uses a hierarchical configuration system that supports:
- Environment-based configuration files
- Environment variable overrides
- Hot-reloading capabilities
- Comprehensive validation
- Multiple deployment environments

## Configuration Files

### File Hierarchy (in order of precedence)

1. **Environment Variables** (highest priority)
2. **Environment-specific config** (`config/{environment}.toml`)
3. **Local config** (`config/local.toml`) - for local overrides
4. **Default config** (`config/default.toml`) - base configuration

### Environment Selection

Set the `ENVIRONMENT` variable to select which environment-specific config to load:

```bash
export ENVIRONMENT=production  # Loads config/production.toml
export ENVIRONMENT=development # Loads config/development.toml
export ENVIRONMENT=staging     # Loads config/staging.toml
```

## Configuration Sections

### Server Configuration

```toml
[server]
host = "0.0.0.0"                    # Server bind address
port = 50051                        # gRPC server port
max_connections = 1000              # Maximum concurrent connections
heartbeat_interval_seconds = 30     # Client heartbeat interval
client_timeout_seconds = 60         # Client connection timeout
enable_compression = true           # Enable gRPC compression
max_message_size = 4194304          # Maximum message size (4MB)
```

### Database Configuration

```toml
[database]
influx_url = "http://localhost:8086"        # InfluxDB connection URL
database_name = "market_data"               # Database name
username = "admin"                          # Optional: Database username
password = "secret"                         # Optional: Database password
connection_pool_size = 20                   # Connection pool size
connection_timeout_seconds = 10             # Connection timeout
write_timeout_seconds = 5                   # Write operation timeout
query_timeout_seconds = 30                  # Query timeout
retry_attempts = 3                          # Number of retry attempts
retry_delay_ms = 1000                       # Delay between retries
circuit_breaker_threshold = 5               # Circuit breaker failure threshold
circuit_breaker_timeout_seconds = 60        # Circuit breaker timeout
```

### Data Processing Configuration

```toml
[data_processing]
snapshot_interval_ms = 5                    # Snapshot capture interval
high_frequency_buffer_size = 10000          # High frequency data buffer size
low_frequency_buffer_size = 1000            # Low frequency data buffer size
private_data_buffer_size = 500              # Private data buffer size
data_validation_enabled = true              # Enable data validation
sanitization_enabled = true                 # Enable data sanitization
dead_letter_queue_size = 1000               # Dead letter queue size
```

### Retention Policies

Configure data retention for different data types:

```toml
[retention_policies.high_frequency]
full_resolution_days = 7        # Keep full resolution for 7 days
downsampled_days = 30          # Keep downsampled for 30 days
archive_days = 90              # Keep archived for 90 days
auto_cleanup = true            # Enable automatic cleanup
compression_enabled = true     # Enable compression

[retention_policies.low_frequency]
full_resolution_days = 730     # 2 years
downsampled_days = 1095        # 3 years
archive_days = 1825            # 5 years
auto_cleanup = true
compression_enabled = true

[retention_policies.private_data]
full_resolution_days = 365     # 1 year
downsampled_days = 730         # 2 years
archive_days = 1095            # 3 years
auto_cleanup = false           # Manual cleanup for sensitive data
compression_enabled = true
```

### Batching Configuration

Configure batching behavior for different operations:

```toml
[batching.database_writes]
size = 1000                    # Batch size
timeout_ms = 5                 # Batch timeout
max_memory_mb = 100            # Maximum memory usage
compression_threshold = 500    # Compression threshold

[batching.client_broadcasts]
size = 100
timeout_ms = 1
max_memory_mb = 50
compression_threshold = 50
```

### Monitoring Configuration

```toml
[monitoring]
metrics_enabled = true         # Enable metrics collection
metrics_port = 9090           # Prometheus metrics port
health_check_port = 8080      # Health check endpoint port
tracing_enabled = true        # Enable distributed tracing
log_level = "info"            # Log level (trace, debug, info, warn, error)
performance_monitoring = true  # Enable performance monitoring
```

## Environment Variables

All configuration values can be overridden using environment variables with the `RAVEN_` prefix:

```bash
# Server configuration
export RAVEN_SERVER__HOST=0.0.0.0
export RAVEN_SERVER__PORT=50051
export RAVEN_SERVER__MAX_CONNECTIONS=1000

# Database configuration
export RAVEN_DATABASE__INFLUX_URL=http://localhost:8086
export RAVEN_DATABASE__DATABASE_NAME=market_data
export RAVEN_DATABASE__USERNAME=admin
export RAVEN_DATABASE__PASSWORD=secret_password

# Data processing configuration
export RAVEN_DATA_PROCESSING__SNAPSHOT_INTERVAL_MS=5
export RAVEN_DATA_PROCESSING__HIGH_FREQUENCY_BUFFER_SIZE=10000

# Retention policies (use double underscore for nested sections)
export RAVEN_RETENTION_POLICIES__HIGH_FREQUENCY__FULL_RESOLUTION_DAYS=7
export RAVEN_RETENTION_POLICIES__LOW_FREQUENCY__FULL_RESOLUTION_DAYS=730

# Batching configuration
export RAVEN_BATCHING__DATABASE_WRITES__SIZE=1000
export RAVEN_BATCHING__CLIENT_BROADCASTS__SIZE=100

# Monitoring configuration
export RAVEN_MONITORING__LOG_LEVEL=info
export RAVEN_MONITORING__METRICS_ENABLED=true
```

## Configuration Management Tool

Use the `raven-config` CLI tool for configuration management:

```bash
# Build the configuration tool
cargo build --bin config_tool

# Validate current configuration
./target/debug/config_tool validate

# Show current configuration
./target/debug/config_tool show

# Show configuration in JSON format
./target/debug/config_tool show --format json

# Check configuration health
./target/debug/config_tool health

# Generate configuration template
./target/debug/config_tool template

# Save template to file
./target/debug/config_tool template --output config/local.toml

# Check environment variables
./target/debug/config_tool env-check

# Show environment-specific recommendations
./target/debug/config_tool recommendations --env production

# Watch configuration file for changes
./target/debug/config_tool watch --config config/default.toml --interval 5
```

## Hot-Reloading

The configuration system supports hot-reloading, which allows configuration changes without restarting the server:

```rust
use market_data_subscription_server::config::ConfigManager;
use std::time::Duration;

// Create configuration manager with hot-reload
let manager = ConfigManager::new(
    "config/default.toml".to_string(),
    Duration::from_secs(5) // Check every 5 seconds
)?;

// Start hot-reload monitoring
manager.start_hot_reload().await?;

// Get current configuration
let config = manager.get_config().await;

// Force reload configuration
manager.force_reload().await?;
```

## Environment-Specific Configurations

### Development Environment

- Smaller buffer sizes for easier debugging
- Verbose logging (debug level)
- Compression disabled for easier inspection
- Shorter retention periods
- Auto-cleanup enabled for all data types

### Production Environment

- Larger buffer sizes for better performance
- Log level set to 'warn' or 'error'
- Compression enabled for all data types
- Longer retention periods for compliance
- Auto-cleanup disabled for sensitive data
- Connection pooling with adequate pool size
- Circuit breakers enabled for resilience

### Staging Environment

- Production-like settings with shorter retention
- Detailed monitoring and metrics enabled
- Moderate buffer sizes
- Failover testing configurations

## Configuration Validation

The system performs comprehensive validation:

- **Port validation**: Ensures ports are valid and available
- **URL validation**: Validates database connection URLs
- **Buffer size validation**: Ensures reasonable buffer sizes
- **Timeout validation**: Validates timeout values
- **Retention policy validation**: Ensures logical retention periods
- **Batch configuration validation**: Validates batching parameters

## Security Considerations

- **Database credentials**: Use environment variables for sensitive data
- **Network binding**: Be careful with `0.0.0.0` binding in production
- **Private data**: Configure appropriate retention and cleanup policies
- **Access control**: Ensure proper firewall and access controls

## Troubleshooting

### Common Issues

1. **Configuration not loading**: Check file paths and permissions
2. **Environment variables not working**: Verify variable names and prefixes
3. **Validation errors**: Check configuration values against constraints
4. **Hot-reload not working**: Verify file permissions and paths

### Debug Commands

```bash
# Check current configuration
./target/debug/config_tool show --format json

# Validate configuration
./target/debug/config_tool validate

# Check for configuration health issues
./target/debug/config_tool health

# Verify environment variables
./target/debug/config_tool env-check
```

## Examples

### Basic Local Development Setup

1. Copy the example environment file:
   ```bash
   cp .env.example .env
   ```

2. Create local configuration:
   ```bash
   ./target/debug/config_tool template --output config/local.toml
   ```

3. Edit `config/local.toml` with your settings

4. Validate configuration:
   ```bash
   ./target/debug/config_tool validate
   ```

### Production Deployment

1. Set environment variables:
   ```bash
   export ENVIRONMENT=production
   export RAVEN_DATABASE__USERNAME=prod_user
   export RAVEN_DATABASE__PASSWORD=secure_password
   export RAVEN_DATABASE__INFLUX_URL=https://influxdb.example.com:8086
   ```

2. Validate production configuration:
   ```bash
   ./target/debug/config_tool validate
   ```

3. Check configuration health:
   ```bash
   ./target/debug/config_tool health
   ```

4. Start the server with hot-reload monitoring:
   ```bash
   ./target/debug/config_tool watch --config config/production.toml
   ```

## Best Practices

1. **Use environment-specific configs** for different deployment environments
2. **Store secrets in environment variables**, not config files
3. **Validate configuration** before deployment
4. **Monitor configuration health** regularly
5. **Use hot-reload** for non-disruptive configuration updates
6. **Test configuration changes** in staging before production
7. **Document custom configurations** for your deployment
8. **Regular backup** of configuration files
9. **Version control** configuration templates
10. **Monitor configuration drift** between environments