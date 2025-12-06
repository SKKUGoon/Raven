# Raven

Raven is a high-performance, modular market data platform written in Rust. It is designed to collect, process, and persist financial market data using a microservices architecture.

## Prerequisites

Before running the Raven binaries, ensure your environment meets the following requirements.

### 1. External Services (Databases)

Raven requires two database services to be running and accessible.

| Service | Role | Requirement | Auth |
| :--- | :--- | :--- | :--- |
| **InfluxDB v2.x** | Tick Persistence | Create an **Organization** and a **Bucket** (e.g., `raven-prod`). | API Token with write access. |
| **PostgreSQL + TimescaleDB** | Bar Persistence | PostgreSQL server with the `timescaledb` extension installed. | Standard Postgres credentials. |

### 2. Database Initialization

You must initialize the database schemas before starting the persistence services.

*   **InfluxDB**: Schema-on-write. No initialization required beyond bucket creation.
*   **TimescaleDB**: Run the initialization script to create the necessary hypertables.
    ```bash
    # Run this against your Postgres database
    psql -d raven -f sql/create_bars_table.sql
    ```

### 3. System Libraries

The binaries are dynamically linked. Ensure the following libraries are installed:
*   **Linux**: `openssl` (libssl-dev / openssl-devel)
*   **macOS**: Standard system libraries (SecureTransport/LibreSSL)

### 4. Network Access

*   **Outbound**: Source services need direct internet access to:
    *   `wss://stream.binance.com:9443`
    *   `wss://fstream.binance.com`
*   **Inbound**: gRPC ports (default `500xx`) must be open between services if running on different machines.

---

## Configuration

Raven uses a hierarchical configuration system.

1.  **File (`prod.toml`)**: Create a `prod.toml` in the working directory (set `RUN_MODE=prod` to use).
    ```toml
    [server]
    host = "0.0.0.0"
    port_binance_spot = 50001
    # ... see src/config.rs for all ports

    [influx]
    url = "http://localhost:8086"
    token = "my-token"
    org = "my-org"
    bucket = "raven-prod"

    [timescale]
    url = "postgres://user:pass@localhost:5432/raven"
    ```

2.  **Environment Variables**: Override any setting using `RAVEN__` prefix (double underscore separator).
    *   `RAVEN__INFLUX__TOKEN=my-secret-token`
    *   `RAVEN__SERVER__PORT_BINANCE_SPOT=50099`

---

## Services & Binaries

Raven consists of several specialized binaries. Below is a guide to each service and how to use it.

### 1. Sources (Data Collectors)

These services connect to external exchanges via WebSocket and stream raw trade data over gRPC.

| Binary | Description | Default Port |
| :--- | :--- | :--- |
| `binance_spot` | Connects to Binance Spot API. | `50001` |
| `binance_futures` | Connects to Binance Futures API. | `50002` |

**Usage:**
```bash
# Start the spot collector
./binance_spot

# Start the futures collector
./binance_futures
```
*Note: These services do not start collecting data until requested by a downstream consumer or via `ravenctl`.*

### 2. Aggregators (Processors)

These services subscribe to **Sources**, process the data (e.g., build candles), and stream the results.

| Binary | Description | Default Port |
| :--- | :--- | :--- |
| `raven_timebar` | Aggregates trades into time-based candles (e.g., 1m, 1h). | `50051` |
| `raven_tibs` | Aggregates trades into Tick Imbalance Bars (TIBs). | `50052` |

**Usage:**
```bash
# Start 1-minute time bars (default)
./raven_timebar

# Start 5-minute time bars
./raven_timebar --seconds 300

# Start TIBs with custom parameters
./raven_tibs --initial-size 1000.0 --alpha-size 0.1
```

### 3. Persistence (Consumers)

These services subscribe to **Sources** or **Aggregators** and write data to the databases.

| Binary | Description | Default Port | Target DB |
| :--- | :--- | :--- | :--- |
| `tick_persistence` | Writes raw trades to InfluxDB. | `50091` | InfluxDB |
| `bar_persistence` | Writes aggregated bars (Time & TIBs) to TimescaleDB. | `50092` | TimescaleDB |

**Usage:**
```bash
./tick_persistence
./bar_persistence
```

### 4. Control Plane (`ravenctl`)

The Command Line Interface (CLI) for managing the Raven cluster. Use this to start/stop collections and check system status.

**Common Commands:**

| Action | Command | Description |
| :--- | :--- | :--- |
| **Check Status** | `./ravenctl status` | Checks health of all configured services. |
| **List Active** | `./ravenctl list` | Lists all active subscriptions on the target host. |
| **Start Spot** | `./ravenctl start --symbol BTCUSDT --service binance_spot` | Starts collecting BTCUSDT trades from Binance Spot. |
| **Start Futures** | `./ravenctl start --symbol BTCUSDT --service binance_futures` | Starts collecting BTCUSDT trades from Binance Futures. |
| **Start 1m Bars** | `./ravenctl start --symbol BTCUSDT --service timebar` | Starts 1m candle generation (auto-starts Spot source). |
| **Start TIBs** | `./ravenctl start --symbol BTCUSDT --service tibs` | Starts TIB generation (auto-starts Spot source). |
| **Stop** | `./ravenctl stop --symbol BTCUSDT` | Stops collection for a symbol. |
| **Stop All** | `./ravenctl stop-all` | Emergency stop for all collections. |

**Example Workflow:**

1.  Start the services (e.g., in separate terminals or via systemd).
    ```bash
    ./binance_spot
    ./raven_timebar
    ```
2.  Use `ravenctl` to start a data feed.
    ```bash
    # This tells raven_timebar to start.
    # raven_timebar will automatically ask binance_spot to start streaming BTCUSDT.
    ./ravenctl start --symbol BTCUSDT --service timebar
    ```
3.  Check the status.
    ```bash
    ./ravenctl list --service timebar
    ```
