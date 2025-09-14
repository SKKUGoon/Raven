# Project Raven - Market Data Subscription Server

> _"The ravens are the memory of the realm"_

A high-performance, real-time market data distribution system built in Rust, designed for financial trading applications. Project Raven provides sub-microsecond latency for high-frequency data while maintaining reliability and scalability through advanced error handling, circuit breakers, and comprehensive monitoring.

## 🏗️ Architecture Overview

> **Note**: This codebase follows a modular architecture with separated implementation and test files for improved maintainability and testing coverage.

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   WebSocket     │    │     Citadel      │    │   gRPC Clients  │
│   Data Feeds    │───▶│   (Validation)   │───▶│   (Streaming)   │
│                 │    │  & Processing    │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                │
                                ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  Circuit        │    │  High Frequency  │    │   InfluxDB      │
│  Breakers &     │───▶│  Atomic Storage  │───▶│   (Historical)  │
│  Dead Letter Q  │    │  (Lock-free)     │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

### Core Components

- **Application Module** (`src/app/`): Entry point coordination with CLI parsing, startup, and graceful shutdown logic
- **Citadel** (`src/citadel/`): Data validation and processing engine with sanitization and quality control
- **High Frequency Storage** (`src/types/atomic.rs`): Lock-free atomic operations for orderbook and trade data
- **Snapshot Service** (`src/snapshot_service/`): Atomic snapshot capture with configurable intervals and metrics
- **Subscription Manager** (`src/subscription_manager/`): Topic-based routing with client lifecycle management
- **gRPC Server** (`src/server/`): Bidirectional streaming with connection management and service implementation
- **Circuit Breakers** (`src/circuit_breaker/`): Fault tolerance and graceful degradation with configurable thresholds
- **Dead Letter Queue** (`src/dead_letter_queue/`): Error handling and retry mechanisms with persistence
- **Client Manager** (`src/client_manager/`): Connection lifecycle management with heartbeat monitoring
- **Error Handling** (`src/error/`): Comprehensive error types with context and conversion implementations
- **Types System** (`src/types/`): Core data structures with atomic operations and snapshot support

## 🚀 Quick Start

### Prerequisites

- Rust 1.75+
- Docker & Docker Compose
- Protocol Buffers compiler (`protoc`)

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd raven

# Install dependencies (macOS)
brew install protobuf

# Build the project
make build

# Deploy full stack with monitoring
make deploy
```

### Development Setup

```bash
# Setup development environment
make dev-setup

# Run development cycle (format, lint, test)
make dev

# Run specific benchmarks
make bench-latency
make bench-high-freq
```

## 📊 Deployment & Operations

### Docker Deployment

```bash
# Deploy full stack (InfluxDB, Dashboard, Prometheus, Redis)
make deploy

# Check service health
make health

# View logs
make logs-server
make logs-influx

# Create backup
make backup

# Restore from backup
make restore BACKUP_NAME=backup_20240101_120000
```

### Service URLs

After deployment, access these services:

- **Raven Dashboard**: http://localhost:8050 (Real-time market data visualization)
- **Prometheus**: http://localhost:9091
- **InfluxDB UI**: http://localhost:8086
- **Health Check**: http://localhost:8080/health
- **Metrics**: http://localhost:9090/metrics
- **gRPC Server**: localhost:50051

### Configuration

Configuration files are located in the `config/` directory:

- `default.toml` - Base configuration
- `development.toml` - Development overrides
- `production.toml` - Production settings
- `staging.toml` - Staging environment

## 🌐 Data Integration & Processing

### Data Types and Structures

The system processes various market data types with built-in validation and sanitization:

#### Orderbook Data

```rust
use market_data_subscription_server::types::OrderBookData;

let orderbook_data = OrderBookData {
    symbol: "BTCUSDT".to_string(),
    timestamp: 1640995200000, // Unix timestamp in milliseconds
    bids: vec![(45000.0, 1.5), (44999.0, 2.0)], // (price, quantity)
    asks: vec![(45001.0, 1.2), (45002.0, 1.8)],
    sequence: 12345,
    exchange: "binance".to_string(),
};
```

#### Trade Data

```rust
use market_data_subscription_server::types::TradeData;

let trade_data = TradeData {
    symbol: "BTCUSDT".to_string(),
    timestamp: 1640995200000,
    price: 45000.5,
    quantity: 0.1,
    side: "buy".to_string(), // "buy" or "sell"
    trade_id: "abc123".to_string(),
    exchange: "binance".to_string(),
};
```

### Citadel Data Processing

The Citadel engine provides comprehensive data validation and processing:

```rust
use market_data_subscription_server::citadel::{Citadel, CitadelConfig};

// Create Citadel with validation rules
let config = CitadelConfig {
    strict_validation: true,
    max_price_deviation: 10.0, // 10%
    enable_sanitization: true,
    enable_dead_letter_queue: true,
    ..Default::default()
};

let citadel = Citadel::new(config, influx_client, subscription_manager);

// Process data with validation and sanitization
citadel.process_orderbook_data("BTCUSDT", orderbook_data).await?;
citadel.process_trade_data("BTCUSDT", trade_data).await?;

// Get processing metrics
let metrics = citadel.get_metrics();
println!("Processed: {}, Failed: {}",
    metrics.get("total_validated").unwrap_or(&0),
    metrics.get("validation_errors").unwrap_or(&0)
);
```

### High Frequency Atomic Storage

For ultra-low latency data access (sub-microsecond):

```rust
use market_data_subscription_server::types::{HighFrequencyStorage, AtomicOrderBook};

// Create atomic storage
let hf_storage = HighFrequencyStorage::new();

// Store data atomically (lock-free)
hf_storage.store_orderbook_atomic("BTCUSDT", &orderbook_data)?;
hf_storage.store_trade_atomic("BTCUSDT", &trade_data)?;

// Capture atomic snapshots
let orderbook_snapshot = hf_storage.capture_orderbook_snapshot("BTCUSDT")?;
let trade_snapshot = hf_storage.capture_trade_snapshot("BTCUSDT")?;
```

### Snapshot Service

Configurable snapshot capture with metrics:

```rust
use market_data_subscription_server::snapshot_service::SnapshotService;

// Create snapshot service with configuration
let snapshot_service = SnapshotService::new(config.clone());

// Capture snapshots with automatic intervals
let snapshot = snapshot_service.capture_snapshot("BTCUSDT").await?;

// Get snapshot metrics
let metrics = snapshot_service.get_metrics().await;
println!("Snapshots captured: {}", metrics.snapshots_captured);
```

## 🔌 gRPC Client Integration

### Protocol Buffer Definitions

The system uses Protocol Buffers for efficient data serialization:

```protobuf
// Market data types
message OrderBookSnapshot {
    string symbol = 1;
    int64 timestamp = 2;
    repeated PriceLevel bids = 3;
    repeated PriceLevel asks = 4;
    int64 sequence = 5;
}

message Trade {
    string symbol = 1;
    int64 timestamp = 2;
    double price = 3;
    double quantity = 4;
    string side = 5;
    string trade_id = 6;
}

message MarketDataMessage {
    oneof data {
        OrderBookSnapshot orderbook = 1;
        Trade trade = 2;
        Candle candle = 3;
        FundingRate funding = 4;
        WalletUpdate wallet = 5;
    }
}
```

### Streaming Client Example

```rust
use tonic::transport::Channel;
use market_data_subscription_server::proto::{
    market_data_service_client::MarketDataServiceClient,
    MarketDataMessage
};

// Connect to server
let mut client = MarketDataServiceClient::connect("http://localhost:50051").await?;

// Start bidirectional streaming
let (tx, rx) = tokio::sync::mpsc::channel(100);

// Send subscription requests
tx.send(SubscriptionRequest {
    client_id: "client_001".to_string(),
    symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
    // ... other fields
}).await?;

let mut stream = client.stream_market_data(tokio_stream::wrappers::ReceiverStream::new(rx)).await?.into_inner();

// Process incoming messages
while let Some(message) = stream.message().await? {
    match message.data {
        Some(market_data_message::Data::Orderbook(orderbook)) => {
            println!("📊 Orderbook: {} @ {} (seq: {})",
                orderbook.symbol, orderbook.timestamp, orderbook.sequence);
        }
        Some(market_data_message::Data::Trade(trade)) => {
            println!("💰 Trade: {} {} @ {} (qty: {})",
                trade.symbol, trade.side, trade.price, trade.quantity);
        }
        Some(market_data_message::Data::Candle(candle)) => {
            println!("🕯️ Candle: {} {} OHLCV: {}/{}/{}/{}/{}",
                candle.symbol, candle.interval,
                candle.open, candle.high, candle.low, candle.close, candle.volume);
        }
        _ => {}
    }
}
```

### Connection Management

The server includes sophisticated connection management:

```rust
use market_data_subscription_server::server::ConnectionManager;

// Connection manager handles:
// - Maximum connection limits
// - Heartbeat monitoring
// - Graceful disconnection
// - Connection statistics

let connection_manager = ConnectionManager::new(1000); // Max 1000 connections

// Automatic heartbeat monitoring with configurable intervals
// Automatic cleanup of stale connections
// Circuit breaker integration for fault tolerance
```

## 📈 Performance & Monitoring

### Benchmarks

The system includes comprehensive benchmark suites for all critical performance paths:

```bash
# Run all benchmarks with detailed output
make bench-all

# Specific benchmark suites
make bench-grpc      # gRPC streaming performance and throughput
make bench-latency   # End-to-end latency measurements
make bench-memory    # Memory allocation and usage patterns
make bench-high-freq # High frequency data processing performance
```

### Benchmark Categories

#### gRPC Streaming Benchmarks (`benches/grpc_streaming_benchmarks.rs`)

- **10k+ Messages/Second**: Tests streaming performance at high throughput
- **Concurrent Connections**: Multiple client simulation
- **Message Serialization**: Protocol Buffer encoding/decoding performance
- **Backpressure Handling**: Flow control under load

#### Latency Benchmarks (`benches/latency_benchmarks.rs`)

- **End-to-End Latency**: Complete request/response cycles
- **Atomic Operations**: Lock-free data structure performance
- **Snapshot Capture**: Atomic snapshot timing
- **Validation Pipeline**: Data processing latency

#### Memory Usage Benchmarks (`benches/memory_usage_benchmarks.rs`)

- **Allocation Patterns**: Memory usage under different loads
- **Zero-Copy Operations**: Efficient data handling
- **Cache Performance**: CPU cache utilization
- **Memory Leaks**: Long-running stability tests

#### High Frequency Benchmarks (`benches/high_frequency_benchmarks.rs`)

- **Atomic Updates**: Lock-free data structure performance
- **Concurrent Access**: Multi-threaded performance
- **Cache Line Optimization**: NUMA-aware operations
- **Batch Processing**: Bulk operation efficiency

### Performance Targets

- **Atomic Operations**: < 100 nanoseconds per update
- **Data Validation**: < 10 microseconds per message (Citadel)
- **Snapshot Capture**: < 1 microsecond for atomic snapshots
- **gRPC Streaming**: < 100 microseconds end-to-end latency
- **Concurrent Connections**: 1,000+ simultaneous clients (configurable)
- **Throughput**: 100k+ messages per second per core
- **Memory Efficiency**: < 1GB for 1000 symbols with full history

### Monitoring

Access the Raven Dashboard at http://localhost:8050 for:

- Real-time performance metrics
- Connection statistics
- Error rates and circuit breaker status
- Memory and CPU utilization
- Database performance

## 🛠️ Development

### Project Structure

```
src/
├── app/                    # Application entry point and coordination
│   ├── mod.rs              # Main coordination logic
│   ├── cli.rs              # CLI argument parsing with version info
│   ├── startup.rs          # Application startup logic
│   └── shutdown.rs         # Graceful shutdown with signal handling
├── bin/                    # Binary executables
├── circuit_breaker/        # Fault tolerance and resilience
│   ├── mod.rs              # Circuit breaker implementation
│   └── tests.rs            # Circuit breaker tests
├── citadel/                # Data validation and processing engine
│   ├── mod.rs              # Main validation and processing logic
│   └── tests.rs            # Citadel validation tests
├── client_manager/         # Connection lifecycle management
│   ├── mod.rs              # Client management implementation
│   └── tests.rs            # Client management tests
├── config/                 # Configuration management
├── data_handlers/          # Data processing (if implemented)
│   ├── high_frequency/     # Lock-free atomic handlers
│   ├── low_frequency/      # Async channel handlers
│   └── private_data/       # Private data handling
├── database/               # InfluxDB integration
├── dead_letter_queue/      # Error handling and retry mechanisms
│   ├── mod.rs              # Dead letter queue implementation
│   └── tests.rs            # Dead letter queue tests
├── error/                  # Comprehensive error handling
│   ├── mod.rs              # Error types and implementations
│   └── tests.rs            # Error handling tests
├── monitoring/             # Metrics and health checks
├── server/                 # gRPC server implementation
│   ├── mod.rs              # Main server coordination
│   ├── grpc_service.rs     # gRPC service implementation
│   ├── connection.rs       # Connection management
│   └── tests.rs            # Server tests
├── snapshot_service/       # Atomic snapshot service
│   ├── mod.rs              # Main service implementation
│   ├── config.rs           # Configuration structures
│   ├── metrics.rs          # Performance metrics
│   └── tests.rs            # Service tests
├── subscription_manager/   # Topic-based routing
│   ├── mod.rs              # Subscription management implementation
│   └── tests.rs            # Subscription management tests
└── types/                  # Core data structures and atomic operations
    ├── mod.rs              # Main type definitions
    ├── atomic.rs           # Lock-free atomic data structures
    ├── snapshots.rs        # Snapshot types and conversions
    └── tests.rs            # Type system tests

# Additional directories
benches/                    # Performance benchmarks
├── grpc_streaming_benchmarks.rs    # gRPC streaming performance
├── high_frequency_benchmarks.rs    # High frequency data processing
├── latency_benchmarks.rs           # End-to-end latency testing
└── memory_usage_benchmarks.rs      # Memory allocation patterns

config/                     # Configuration files
├── default.toml           # Base configuration
├── development.toml       # Development overrides
├── production.toml        # Production settings
└── staging.toml           # Staging environment

docker/                     # Docker deployment
├── docker-compose.yml     # Full stack deployment
├── Dockerfile             # Application container
├── dashboard/             # Real-time dashboard application
└── prometheus.yml         # Prometheus configuration

proto/                      # Protocol Buffer definitions
├── market_data.proto      # Market data messages
└── subscription.proto     # Subscription management

tests/                      # Integration tests
├── final_system_tests.rs  # End-to-end system tests
└── integration_tests.rs   # Integration test suites
```

### Testing

```bash
# Run all tests
make test

# Run integration tests
make test-integration

# Run unit tests
make test-unit

# Run with coverage
cargo test --all-features
```

### Code Quality

```bash
# Format code
make fmt

# Run linter
make lint

# Check without building
make check
```

## 🔧 Configuration

### Environment Variables

```bash
# Server configuration
RAVEN_HOST=0.0.0.0
RAVEN_PORT=50051
RAVEN_MAX_CONNECTIONS=10000

# Database configuration
INFLUX_URL=http://localhost:8086
INFLUX_DATABASE=market_data
INFLUX_USERNAME=raven
INFLUX_PASSWORD=ravens_see_all

# Monitoring
METRICS_PORT=9090
HEALTH_CHECK_PORT=8080
LOG_LEVEL=info
```

### Performance Tuning

For high-frequency trading environments:

```toml
[server]
max_connections = 50000
connection_timeout = 30

[high_frequency]
atomic_updates = true
cache_line_padding = true
numa_awareness = true

[monitoring]
metrics_interval = 1000  # 1 second
health_check_interval = 5000  # 5 seconds
```

## 🚨 Error Handling & Reliability

The system includes comprehensive error handling and reliability features:

### Circuit Breakers

```rust
use market_data_subscription_server::circuit_breaker::CircuitBreakerRegistry;

// Automatic circuit breaker protection
let registry = CircuitBreakerRegistry::new();

// Circuit breakers monitor:
// - Database connection failures
// - External API timeouts
// - High error rates
// - Resource exhaustion
```

### Dead Letter Queue

```rust
use market_data_subscription_server::dead_letter_queue::DeadLetterQueue;

// Failed operations are automatically queued for retry
let dlq = DeadLetterQueue::new(config);

// Features:
// - Configurable retry attempts
// - Exponential backoff
// - Persistent storage
// - Manual intervention support
```

### Error Types

```rust
use market_data_subscription_server::error::{RavenError, RavenResult};

// Comprehensive error categorization:
// - DataValidation: Invalid market data
// - DatabaseConnection: InfluxDB issues
// - NetworkTimeout: Connection problems
// - ConfigurationError: Invalid settings
// - InternalError: System failures
```

### Graceful Shutdown

```rust
// The application handles shutdown signals gracefully:
// 1. Stop accepting new connections
// 2. Complete in-flight requests
// 3. Persist dead letter queue entries
// 4. Flush metrics and logs
// 5. Clean up resources
```

## 📚 API Reference

### gRPC Services

Based on the Protocol Buffer definitions in `proto/market_data.proto`:

- `MarketDataService`: Main service for streaming market data
- **Message Types**:
  - `OrderBookSnapshot`: Real-time orderbook state with bids/asks
  - `Trade`: Individual trade execution with price, quantity, side
  - `Candle`: OHLCV candlestick data with intervals
  - `FundingRate`: Perpetual funding rates with next funding time
  - `WalletUpdate`: Account balance changes with asset details

### CLI Interface

```bash
# Start server with custom configuration
./raven --config config/production.toml --host 0.0.0.0 --port 50051

# Available CLI options:
--config FILE              # Configuration file path
--host HOST                # Server host address
--port PORT                # Server port number
--log-level LEVEL          # Log level (trace, debug, info, warn, error)
--database-url URL         # InfluxDB connection URL
--max-connections COUNT    # Maximum concurrent connections
--validate                 # Validate configuration and exit
--print-config             # Print loaded configuration and exit
```

### Configuration

The system uses TOML configuration files with environment-specific overrides:

```toml
[server]
host = "0.0.0.0"
port = 50051
max_connections = 1000
heartbeat_interval_seconds = 30

[database]
influx_url = "http://localhost:8086"
bucket = "market_data"
org = "raven"
connection_pool_size = 20

[data_processing]
snapshot_interval_ms = 5
high_frequency_buffer_size = 10000
data_validation_enabled = true

[monitoring]
metrics_enabled = true
metrics_port = 9090
health_check_port = 8080
log_level = "info"
```

## 📁 Module Organization & Architecture

This codebase follows a modular architecture with clear separation of concerns:

### Core Principles

- **Separation of Implementation and Tests**: Each module has dedicated test files
- **Atomic Operations**: Lock-free data structures for high-frequency operations
- **Error Resilience**: Comprehensive error handling with circuit breakers and dead letter queues
- **Configuration-Driven**: Environment-specific configuration with CLI overrides
- **Observability**: Built-in metrics, tracing, and health checks

### Key Architectural Components

#### Application Layer (`src/app/`)

- **CLI Interface**: Comprehensive command-line argument parsing with version info
- **Startup Orchestration**: Coordinated initialization of all system components
- **Graceful Shutdown**: Signal handling with proper resource cleanup

#### Data Processing (`src/citadel/`, `src/types/`)

- **Citadel Engine**: Data validation, sanitization, and quality control
- **Atomic Storage**: Lock-free data structures for sub-microsecond latency
- **Snapshot Service**: Configurable atomic snapshot capture with metrics

#### Network Layer (`src/server/`)

- **gRPC Service**: Bidirectional streaming with Protocol Buffer serialization
- **Connection Management**: Client lifecycle with heartbeat monitoring
- **Circuit Breaker Integration**: Fault tolerance for external dependencies

#### Reliability Layer (`src/circuit_breaker/`, `src/dead_letter_queue/`)

- **Circuit Breakers**: Prevent cascade failures with configurable thresholds
- **Dead Letter Queue**: Retry mechanisms with exponential backoff and persistence
- **Error Categorization**: Comprehensive error types with context

#### Observability (`src/monitoring/`)

- **Metrics Collection**: Prometheus-compatible metrics
- **Health Checks**: Component status monitoring
- **Distributed Tracing**: Request flow tracking

This architecture ensures high performance, reliability, and maintainability while supporting financial-grade requirements for latency and fault tolerance.

## 🚧 Current Development Status

### Recently Implemented Features

- ✅ **Modular Architecture**: Separated implementation and test files across all modules
- ✅ **Citadel Data Engine**: Comprehensive data validation and processing with sanitization
- ✅ **Atomic Storage System**: Lock-free data structures for high-frequency operations
- ✅ **Snapshot Service**: Configurable atomic snapshot capture with performance metrics
- ✅ **Enhanced Error Handling**: Comprehensive error types with context and conversion
- ✅ **Circuit Breaker System**: Fault tolerance with configurable thresholds
- ✅ **Dead Letter Queue**: Error recovery with retry mechanisms and persistence
- ✅ **Client Manager**: Connection lifecycle management with heartbeat monitoring
- ✅ **CLI Interface**: Rich command-line interface with version info and configuration overrides
- ✅ **Graceful Shutdown**: Signal handling with proper resource cleanup
- ✅ **Performance Benchmarks**: Comprehensive benchmark suite for all critical paths

### In Development

- 🔄 **Data Handler Integration**: Connecting high-frequency and low-frequency data handlers
- 🔄 **gRPC Service Implementation**: Complete bidirectional streaming service
- 🔄 **Subscription Manager**: Topic-based routing with client lifecycle integration
- 🔄 **Database Integration**: Enhanced InfluxDB client with circuit breaker protection
- 🔄 **Monitoring Services**: Complete observability stack with metrics and tracing

### Performance Targets (Current Implementation)

- **Atomic Operations**: < 100 nanoseconds per update
- **Data Validation**: < 10 microseconds per message
- **Snapshot Capture**: < 1 microsecond for atomic snapshots
- **Memory Efficiency**: Zero-copy operations where possible
- **Error Recovery**: < 1ms for circuit breaker decisions

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Run the full development cycle: `make dev` (format, lint, test)
4. Run benchmarks: `make bench-all`
5. Submit a pull request with comprehensive tests

### Development Workflow

```bash
# Setup development environment
make dev-setup

# Run development cycle
make dev  # Equivalent to: make fmt lint test

# Run specific benchmark suites
make bench-latency    # Latency benchmarks
make bench-memory     # Memory usage patterns
make bench-grpc       # gRPC streaming performance
make bench-high-freq  # High frequency data processing

# Deploy and test full stack
make deploy
make health
```

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

---

_"Winter is coming, but the ravens are ready."_ 🐦‍⬛
