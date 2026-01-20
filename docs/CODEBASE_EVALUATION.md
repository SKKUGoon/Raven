# Raven Codebase Evaluation

## 1. Objective

**Raven** is a modular, high-performance market-data platform written in **Rust**. Its primary goal is to ingest, process, and persist financial market data (crypto) with low latency and high reliability.

Key capabilities:
-   **Ingestion**: Connects to exchange WebSockets (Binance Spot, Futures) to stream Trade and Orderbook data.
-   **Processing**: Aggregates raw ticks into features like Time Bars (Candles), Tick Imbalance Bars, etc.
-   **Persistence**: Stores high-frequency ticks in InfluxDB and aggregated bars in TimescaleDB (Postgres).
-   **Distribution**: streams data to clients via **gRPC**.

## 2. Codebase Outline

The project follows a microservices architecture where each component is a separate binary, orchestrated by a central CLI (`ravenctl`).

### Directory Structure (`src/`)

-   **`bin/`**: Service entry points.
    -   `ravenctl.rs`: The control plane CLI. Orchestrates other services.
    -   `binance_spot.rs` / `binance_futures.rs`: Source collectors.
    -   `raven_timebar.rs`, `raven_tibs.rs`, etc.: Feature processors.
    -   `tick_persistence.rs` / `bar_persistence.rs`: Database writers.
-   **`proto/`**: gRPC definitions (Inter-service communication contract).
    -   `market_data.proto`: Data models (`Trade`, `Candle`, `OrderBookSnapshot`).
    -   `control.proto`: Control plane commands (`StartCollection`, `StopCollection`).
-   **`lib.rs`**: Shared library code containing:
    -   `config.rs`: Configuration management (Figment, TOML, Env vars).
    -   `features/`: Logic for aggregating bars.
    -   `service/`: Common gRPC service wrapper code (`RavenService`).
    -   `source/`: WebSocket connection logic.
    -   `db/`: Database connection pools/logic.

### Key Technologies
-   **Language**: Rust (Edition 2021)
-   **Communication**: gRPC (`tonic`, `prost`)
-   **Async Runtime**: Tokio
-   **Databases**:
    -   InfluxDB (via `influxdb2` crate)
    -   PostgreSQL/TimescaleDB (via `sqlx`)
-   **Serialization**: Serde

## 3. Coding Style

The codebase exhibits a **Modern Rust** style with a focus on concurrency and type safety.

-   **Async/Await**: Heavy usage of `tokio` for asynchronous tasks and streams.
-   **Error Handling**: Uses `thiserror` for typed errors and `anyhow` (implied or similar) pattern for top-level results.
-   **Configuration**: Strongly typed configuration structs (`Settings`) derived from files and environment variables.
-   **Observability**: Uses `tracing` and `tracing-subscriber` for structured logging.
-   **Strict Typing**: Uses Protobuf generated types (`prost`) ensuring strict contracts between services.
-   **Modular Design**: Services share a common `RavenService` abstraction to handle startup, shutdown, and gRPC server boilerplate.

## 4. How to Use

### Prerequisites
-   Rust Toolchain (`cargo`)
-   PostgreSQL (with TimescaleDB extension) and InfluxDB v2 running.
-   `protoc` compiler (for building gRPC stubs).

### Building
```bash
cargo build --release
```

### Running (Control Plane)
The `ravenctl` binary is the entry point. It manages the lifecycle of other services.

1.  **Start Infrastructure**:
    ```bash
    ./target/release/ravenctl start
    ```
    This starts all background services (collectors, processors, writers).

2.  **Start Data Collection**:
    To start collecting and processing data for a symbol:
    ```bash
    ./target/release/ravenctl start --symbol ETH --base USDC --venue BINANCE_SPOT
    ```

3.  **Visualization**:
    View the service topology:
    ```bash
    ./target/release/ravenctl graph
    ```

### Client Integration (AI Agent Usage)
To consume data from Raven, an agent should use gRPC:
1.  **Read Contracts**: Parse `proto/market_data.proto` to understand the message structure.
2.  **Connect**: Connect to the standard ports (e.g., `50051` for Timebars).
3.  **Subscribe**: Send a `Subscribe` request with the desired `symbol` and `venue`.

**Example (Concept):**
```rust
// Connect to Timebar Service
let mut client = MarketDataClient::connect("http://localhost:50051").await?;
let request =Request::new(MarketDataRequest {
    symbol: "ETHUSDC".into(),
    venue: "BINANCE_SPOT".into(),
    data_type: DataType::Candle.into(),
});
let mut stream = client.subscribe(request).await?.into_inner();
while let Some(msg) = stream.message().await? {
    println!("Received candle: {:?}", msg);
}
```
