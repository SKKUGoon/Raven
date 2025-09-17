# Project Raven - Docker Deployment Guide

🐦 **Project Raven** is a high-performance market data subscription server built with Rust, designed to handle real-time financial data streams with sub-millisecond latency.

## Quick Start

### Prerequisites

- Docker 20.10+ and Docker Compose 2.0+
- Make (for using Makefile commands)
- 4GB+ RAM available for containers
- Ports 3000, 6379, 8080, 8086, 9090, 9091, 16686, 50051 available

### Deploy Full Stack

```bash
# From project root directory
make deploy
```

This will:
- Build the Rust application in a Docker container
- Start InfluxDB, Redis, Prometheus, Grafana, and Jaeger
- Deploy the market data server
- Show service URLs when ready

### Check Status

```bash
make status    # Show running services
make health    # Check service health
make urls      # Show service URLs
```

## Available Commands

### Development
```bash
make build          # Build Rust application
make test           # Run all tests
make lint           # Run clippy linter
make fmt            # Format code
make dev            # Run fmt + lint + test
```

### Docker Operations
```bash
make deploy         # Deploy full stack
make start          # Start services
make stop           # Stop services
make restart        # Restart services
make docker-build   # Build Docker image only
```

### Monitoring & Logs
```bash
make logs           # All service logs
make logs-server    # Market data server logs
make logs-influx    # InfluxDB logs
make logs-grafana   # Grafana logs
```

### Data Management
```bash
make backup         # Create data backup
make restore BACKUP_NAME=backup_name  # Restore from backup
make clean          # Clean containers/networks
make clean-all      # Clean everything including data
```

### Utilities
```bash
make urls           # Show service URLs
make shell          # Open server container shell
make db-shell       # Open InfluxDB shell
make help           # Show all commands
```

## Service URLs

After deployment, access these services:

| Service | URL | Credentials |
|---------|-----|-------------|
| **Grafana Dashboard** | http://localhost:3000 | `raven` / `ravens_see_all` |
| **Prometheus** | http://localhost:9091 | - |
| **Jaeger Tracing** | http://localhost:16686 | - |
| **InfluxDB UI** | http://localhost:8086 | `raven` / `ravens_fly_at_dawn` |
| **Health Check** | http://localhost:8080/health | - |
| **Metrics** | http://localhost:9090/metrics | - |
| **gRPC Server** | localhost:50051 | - |

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   gRPC Clients  │    │  Market Data    │    │   WebSocket     │
│                 │◄──►│     Server      │◄──►│     Feeds       │
│  Trading Apps   │    │  (Rust/Tokio)   │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                │
                                ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│    Grafana      │    │    InfluxDB     │    │     Redis       │
│   Dashboard     │◄──►│  Time Series    │    │     Cache       │
│                 │    │    Database     │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         ▲                       ▲
         │              ┌─────────────────┐
         └──────────────┤   Prometheus    │
                        │    Metrics      │
                        └─────────────────┘
```

## Configuration

### Environment Variables

The market data server supports these environment variables:

```bash
# Database Configuration
INFLUXDB_URL=http://influxdb:8086
INFLUXDB_DATABASE=market_data
INFLUXDB_USERNAME=raven
INFLUXDB_PASSWORD=ravens_fly_at_dawn

# Cache Configuration
REDIS_URL=redis://redis:6379

# Server Configuration
SERVER_HOST=0.0.0.0
SERVER_PORT=50051
HEALTH_PORT=8080
METRICS_PORT=9090

# Logging
RUST_LOG=info
RUST_BACKTRACE=1
```

### Resource Limits

Default resource limits per service:

| Service | CPU Limit | Memory Limit | CPU Reserve | Memory Reserve |
|---------|-----------|--------------|-------------|----------------|
| Market Data Server | 2.0 | 2GB | 1.0 | 1GB |
| InfluxDB | 1.0 | 1GB | 0.5 | 512MB |
| Redis | 0.5 | 512MB | 0.25 | 256MB |
| Prometheus | 0.5 | 512MB | 0.25 | 256MB |
| Grafana | 0.5 | 512MB | 0.25 | 256MB |

## Data Persistence

### Volumes

- `influxdb_data`: InfluxDB time-series data
- `influxdb_config`: InfluxDB configuration
- `redis_data`: Redis cache data
- `prometheus_data`: Prometheus metrics data
- `grafana_data`: Grafana dashboards and settings
- `raven_logs`: Application logs
- `raven_data`: Application data

### Backup & Restore

Create backup:
```bash
make backup
# Creates timestamped backup in docker/backups/
```

Restore from backup:
```bash
make restore BACKUP_NAME=raven_backup_20240101_120000
```

## Monitoring

### Grafana Dashboards

Pre-configured dashboards include:
- **Raven Overview**: Key metrics and performance indicators
- **gRPC Metrics**: Connection and request statistics
- **Database Performance**: InfluxDB query and write performance
- **System Resources**: CPU, memory, and network usage

### Prometheus Metrics

Key metrics collected:
- `grpc_active_connections`: Active gRPC connections
- `messages_processed_total`: Total messages processed
- `response_latency_seconds`: Response latency histogram
- `errors_total`: Total error count
- `process_resident_memory_bytes`: Memory usage

### Health Checks

All services include health checks:
- **Startup**: 30-40 second grace period
- **Interval**: 30 second checks
- **Timeout**: 10 second timeout
- **Retries**: 3-5 retries before marking unhealthy

## Troubleshooting

### Common Issues

**Services won't start:**
```bash
make logs           # Check all logs
make health         # Check health status
docker system df    # Check disk space
```

**Port conflicts:**
```bash
# Check what's using ports
netstat -tulpn | grep :3000
netstat -tulpn | grep :50051

# Stop conflicting services or change ports in docker compose.yml
```

**Out of memory:**
```bash
# Check container memory usage
docker stats

# Reduce resource limits in docker compose.yml
# Or increase available system memory
```

**Database connection issues:**
```bash
make logs-influx    # Check InfluxDB logs
make db-shell       # Test InfluxDB connection
```

### Performance Tuning

**For high-frequency trading:**
1. Increase container CPU/memory limits
2. Use SSD storage for volumes
3. Tune kernel network parameters
4. Consider dedicated hardware

**For development:**
1. Reduce resource limits
2. Disable unnecessary services
3. Use local storage volumes

### Logs and Debugging

```bash
# Application logs
make logs-server

# Database logs
make logs-influx

# All service logs
make logs

# Container shell access
make shell

# Database shell access
make db-shell
```

## Production Deployment

### Security Considerations

1. **Change default passwords** in docker compose.yml
2. **Use secrets management** for production credentials
3. **Enable TLS** for gRPC and web interfaces
4. **Configure firewall** rules for exposed ports
5. **Use non-root containers** (already configured)

### Scaling

1. **Horizontal scaling**: Deploy multiple instances behind load balancer
2. **Database scaling**: Use InfluxDB clustering
3. **Cache scaling**: Use Redis clustering
4. **Resource scaling**: Increase CPU/memory limits

### Monitoring in Production

1. **Set up alerting** in Prometheus/Grafana
2. **Configure log aggregation** (ELK stack, etc.)
3. **Monitor disk usage** for data volumes
4. **Set up backup automation**

## Development

### Local Development

```bash
# Setup development environment
make dev-setup

# Development cycle
make dev           # fmt + lint + test

# Build and test
make build test
```

### Testing

```bash
make test              # All tests
make test-unit         # Unit tests only
make test-integration  # Integration tests only
make bench             # Benchmarks
```

## Support

For issues and questions:
1. Check the logs: `make logs`
2. Verify health: `make health`
3. Review configuration in `docker compose.yml`
4. Check resource usage: `docker stats`

## License

Project Raven is licensed under the MIT License.