# Raven

## Summary
Raven is a Rust-powered market data service that ingests cryptocurrency feeds, validates them in real time, and streams the results over gRPC. The runtime orchestrated by `app::run()` wires together configuration loading, dependency validation, InfluxDB persistence, and graceful shutdown handling, exposing both public streaming APIs and a control plane for collectors.

## Key Capabilities
- **High-frequency ingestion:** `HighFrequencyHandler` writes directly into lock-free `HighFrequencyStorage` for ultra-low latency updates.
- **Dynamic subscriptions:** `SubscriptionManager` tracks client state, topic routing, heartbeats, and reconnection persistence.
- **Circuit-protected storage:** `Citadel` validates, sanitizes, and persists data to InfluxDB with dead-letter recovery paths.
- **gRPC fan-out:** `MarketDataServer` enforces connection limits while streaming snapshots and live updates to clients.
- **Operational visibility:** Health, metrics, and tracing services expose readiness, active collectors, and performance counters.

## Architecture
### Runtime Flow
1. **Bootstrap:** `app::startup` loads configuration via `ConfigLoader`, applies CLI overrides, validates port availability, and initializes logging.
2. **Foundation services:** Dead letter queues, circuit breakers, and InfluxDB clients are created before the data plane spins up.
3. **Data plane:** `Citadel` plus `HighFrequencyStorage` hold validated data; the snapshot service periodically publishes in-memory state and persists batches.
4. **Serving layer:** `MarketDataServer::start()` exposes the `MarketDataService` gRPC API, while `ControlService` listens on a loopback port for administrative commands.
5. **Shutdown:** Coordinated stop signals abort gRPC tasks, drain collectors, flush queues, and dispose monitoring handles gracefully.

### Component Map
- `src/app` – CLI parsing, startup wiring, shutdown, and version reporting.
- `src/citadel` – Validation engine, atomic storage abstractions, and snapshot broadcasting.
- `src/server` – Connection gating plus gRPC surface backed by subscription routing and historical queries.
- `src/control` – `CollectorManager` that starts/stops exchange collectors through the control gRPC interface.
- `src/exchanges/binance` – WebSocket adapters and parsers for Binance spot and futures feeds.
- `src/config` – Loader, builder, validation, and sectioned TOML schemas with hot-reload support.
- `proto/` – Market data, subscription, and control plane protocol buffers compiled by `build.rs`.

## Configuration
Raven looks for `ENVIRONMENT` (default `development`) and loads `config/<environment>.toml` via `ConfigLoader`. Each config is split into `[server]`, `[database]`, `[data_processing]`, `[retention_policies]`, `[batching]`, and `[monitoring]` sections, validated by `RuntimeConfig::validate()`. Environment variables prefixed with `RAVEN__` can override settings at runtime.

For local work, copy `config/example.toml` to `config/development.toml` and fill in InfluxDB credentials. Production deployments use `config/secret.toml`, which should remain outside version control.

## Running Locally
1. **Install prerequisites:** Rust 1.70+ and access to an InfluxDB 2.x instance.
2. **Fetch dependencies:** `cargo fetch` (optional) to warm the build cache.
3. **Launch the server:**
   ```bash
   ENVIRONMENT=development cargo run --bin raven
   ```
4. **Override configuration file:**
   ```bash
   cargo run --bin raven -- --config /path/to/custom.toml
   ```
5. **Stream specific symbols:**
   ```bash
   cargo run --bin raven -- --symbols BTCUSDT,ETHUSDT
   ```

## Collector Control (`ravenctl`)
The `ravenctl` binary manages live exchange collectors through the control gRPC service on `127.0.0.1:50052`.

```bash
# Start Binance futures data for BTCUSDT
cargo run --bin ravenctl -- use --exchange binance_futures --symbol BTCUSDT

# Stop the collection
cargo run --bin ravenctl -- stop --exchange binance_futures --symbol BTCUSDT

# Inspect active collectors
cargo run --bin ravenctl -- list
```

These commands invoke `CollectorManager`, which spawns or terminates the asynchronous ingestion tasks and cleans associated in-memory state.

## Monitoring
- **Health checks:** HTTP health service listens on `monitoring.health_check_port` with component-level diagnostics.
- **Metrics:** Prometheus metrics expose connection counts, ingestion stats, and circuit breaker signals at `monitoring.metrics_port`.
- **Tracing:** `TracingService` wires `tracing` spans through OpenTelemetry exporters when enabled.

Use `curl http://localhost:9091/health` and `curl http://localhost:9090/metrics` (default ports) to verify the service.

## Testing
- **All tests:** `cargo test --all`
- **Unit tests:** `cargo test --test unit`
- **Integration tests:** `cargo test --test integration`

The unit suite exercises subscription routing, connection management, and Citadel validation logic, while integration tests spin up higher-level flows for config, exchanges, and server boundaries.

## Development Workflow
The repository includes a Makefile with wrappers for common Rust commands (`make fmt`, `make lint`, `make test`). These targets are intended purely for development convenience; the canonical build and execution flow remains the direct Cargo commands shown above.

## Repository Layout
- `src/main.rs` – Application entry point delegating to `app::run()`.
- `src/bin/ravenctl.rs` – Operational CLI for collectors and configuration introspection.
- `src/monitoring` – Health, metrics, and tracing service implementations.
- `tests/` – Unit and integration suites grouped by domain under `tests/unit` and `tests/integration`.
- `python_client/` – Smoke-test gRPC client aligned with the protobuf schema.
