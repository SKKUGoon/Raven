# Project Raven - Market Data Subscription Server

> *"The ravens are the memory of the realm"*

A high-performance, real-time market data distribution system built in Rust, designed for financial trading applications. Project Raven provides sub-microsecond latency for high-frequency data while maintaining reliability and scalability.

## 🏗️ Architecture Overview

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   WebSocket     │    │   High Frequency │    │   gRPC Clients  │
│   Data Feeds    │───▶│     Handler      │───▶│   (Streaming)   │
│                 │    │  (Lock-free)     │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                │
                                ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   REST APIs     │    │   Low Frequency  │    │   InfluxDB      │
│   (Candles,     │───▶│     Handler      │───▶│   (Historical)  │
│   Funding)      │    │  (Async Queue)   │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

### Core Components

- **High Frequency Handler**: Lock-free atomic operations for orderbook and trade data
- **Low Frequency Handler**: Async channel-based processing for candles and funding rates  
- **Subscription Manager**: Topic-based routing with client lifecycle management
- **gRPC Server**: Bidirectional streaming with connection management
- **Circuit Breakers**: Fault tolerance and graceful degradation
- **Dead Letter Queue**: Error handling and retry mechanisms

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
# Deploy full stack (InfluxDB, Grafana, Prometheus, Redis)
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

- **Grafana Dashboard**: http://localhost:3000 (raven/ravens_see_all)
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

## 🌐 WebSocket Data Integration

### Creating WebSocket Connections

The system expects WebSocket data in the following formats:

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

### High Frequency Handler Integration

For real-time orderbook and trade data (sub-microsecond latency):

```rust
use market_data_subscription_server::data_handlers::HighFrequencyHandler;

// Create handler
let hf_handler = HighFrequencyHandler::new();

// Ingest orderbook data (lock-free atomic operation)
hf_handler.ingest_orderbook_atomic("BTCUSDT", &orderbook_data)?;

// Ingest trade data (lock-free atomic operation)  
hf_handler.ingest_trade_atomic("BTCUSDT", &trade_data)?;

// Capture atomic snapshots
let orderbook_snapshot = hf_handler.capture_orderbook_snapshot("BTCUSDT")?;
let trade_snapshot = hf_handler.capture_trade_snapshot("BTCUSDT")?;
```

### Low Frequency Handler Integration

For candles, funding rates, and other non-critical data:

```rust
use market_data_subscription_server::data_handlers::LowFrequencyHandler;
use market_data_subscription_server::types::{CandleData, FundingRateData};

// Create and start handler
let lf_handler = LowFrequencyHandler::new();
lf_handler.start().await?;

// Ingest candle data (async channel-based)
let candle_data = CandleData {
    symbol: "BTCUSDT".to_string(),
    timestamp: 1640995200000,
    open: 45000.0,
    high: 45100.0,
    low: 44900.0,
    close: 45050.0,
    volume: 1000.0,
    interval: "1m".to_string(),
    exchange: "binance".to_string(),
};

lf_handler.ingest_candle(&candle_data).await?;

// Ingest funding rate data
let funding_data = FundingRateData {
    symbol: "BTCUSDT".to_string(),
    timestamp: 1640995200000,
    rate: 0.0001, // 0.01%
    next_funding_time: 1640995200000 + 28800000, // 8 hours later
    exchange: "binance".to_string(),
};

lf_handler.ingest_funding_rate(&funding_data).await?;
```

## 🔌 gRPC Client Integration

### Streaming Client Example

```rust
use tonic::transport::Channel;
use market_data_subscription_server::proto::{
    market_data_service_client::MarketDataServiceClient,
    SubscribeRequest, SubscriptionRequest, DataType
};

// Connect to server
let mut client = MarketDataServiceClient::connect("http://localhost:50051").await?;

// Create subscription request
let subscribe_req = SubscribeRequest {
    client_id: "client_001".to_string(),
    symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
    data_types: vec![DataType::Orderbook as i32, DataType::Trades as i32],
    filters: std::collections::HashMap::new(),
};

// Start streaming
let outbound = async_stream::stream! {
    yield SubscriptionRequest {
        request: Some(subscription_request::Request::Subscribe(subscribe_req)),
    };
    
    // Send heartbeats
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        yield SubscriptionRequest {
            request: Some(subscription_request::Request::Heartbeat(HeartbeatRequest {
                client_id: "client_001".to_string(),
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64,
            })),
        };
    }
};

let mut inbound = client.stream_market_data(outbound).await?.into_inner();

// Process incoming messages
while let Some(message) = inbound.message().await? {
    match message.data {
        Some(market_data_message::Data::Orderbook(orderbook)) => {
            println!("Orderbook: {} @ {}", orderbook.symbol, orderbook.timestamp);
        }
        Some(market_data_message::Data::Trade(trade)) => {
            println!("Trade: {} {} @ {}", trade.symbol, trade.side, trade.price);
        }
        _ => {}
    }
}
```

### Simple Subscribe/Unsubscribe

```rust
// Subscribe
let response = client.subscribe(SubscribeRequest {
    client_id: "client_001".to_string(),
    symbols: vec!["BTCUSDT".to_string()],
    data_types: vec![DataType::Orderbook as i32],
    filters: std::collections::HashMap::new(),
}).await?;

println!("Subscribed to {} topics", response.into_inner().subscribed_topics.len());

// Unsubscribe
let response = client.unsubscribe(UnsubscribeRequest {
    client_id: "client_001".to_string(),
    symbols: vec!["BTCUSDT".to_string()],
    data_types: vec![DataType::Orderbook as i32],
}).await?;
```

## 📈 Performance & Monitoring

### Benchmarks

```bash
# Run all benchmarks
make bench-all

# Specific benchmark suites
make bench-grpc      # gRPC streaming performance
make bench-latency   # End-to-end latency
make bench-memory    # Memory usage patterns
make bench-high-freq # High frequency data processing
```

### Performance Targets

- **High Frequency Ingestion**: < 1 microsecond per update
- **gRPC Streaming Latency**: < 100 microseconds end-to-end
- **Concurrent Connections**: 10,000+ simultaneous clients
- **Throughput**: 1M+ messages per second
- **Memory Usage**: < 1GB for 1000 symbols

### Monitoring

Access Grafana at http://localhost:3000 for:

- Real-time performance metrics
- Connection statistics
- Error rates and circuit breaker status
- Memory and CPU utilization
- Database performance

## 🛠️ Development

### Project Structure

```
src/
├── bin/                    # Binary executables
├── circuit_breaker/        # Fault tolerance
├── client_manager/         # Connection lifecycle
├── config/                 # Configuration management
├── data_handlers/          # Data processing
│   ├── high_frequency/     # Lock-free atomic handlers
│   └── low_frequency/      # Async channel handlers
├── database/               # InfluxDB integration
├── monitoring/             # Metrics and health checks
├── subscription_manager/   # Topic routing
└── types.rs               # Core data structures
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

## 🚨 Error Handling

The system includes comprehensive error handling:

- **Circuit Breakers**: Prevent cascade failures
- **Dead Letter Queue**: Retry failed operations
- **Graceful Degradation**: Continue operating with reduced functionality
- **Health Checks**: Monitor component status

## 📚 API Reference

### gRPC Services

- `StreamMarketData`: Bidirectional streaming for real-time data
- `Subscribe`: Simple subscription management
- `Unsubscribe`: Remove subscriptions
- `GetHistoricalData`: Query historical data from InfluxDB

### Data Types

- `OrderBookSnapshot`: Real-time orderbook state
- `Trade`: Individual trade execution
- `Candle`: OHLCV candlestick data
- `FundingRate`: Perpetual funding rates
- `WalletUpdate`: Account balance changes

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Run tests and benchmarks
4. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

---

*"Winter is coming, but the ravens are ready."* 🐦‍⬛