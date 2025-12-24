# Raven

Raven is a modular market-data platform in Rust: **sources** ingest exchange data, **processors** build features/bars, and **persistence** stores ticks and bars. Everything is controlled via a gRPC **Control plane** (`ravenctl`).


## Prerequisites

### System

- **Rust toolchain**: Rust 2021 edition (install via `rustup`; `cargo` is required).
- **OS**: macOS/Linux recommended. `ravenctl` manages processes using system commands like `ps` + `kill`.
- **Filesystem**: `ravenctl` writes PID/log files under `~/.raven/log` (requires `$HOME` to be set and writeable).
- **Native TLS deps (Linux only)**: because Raven uses `native-tls` (WebSockets + gRPC), you’ll typically need system OpenSSL tooling such as `pkg-config` + `libssl-dev`.

### Databases

- **InfluxDB v2.x**: used by `tick_persistence` (writes trades + orderbook snapshots).
- **PostgreSQL + TimescaleDB extension**: used by `bar_persistence` (writes `bar__time` and `bar__tick_imbalance`).

Notes:
- **InfluxDB** is schema-on-write, but you must provision:
  - `influx.url`, `influx.org`, `influx.bucket`, and a valid `influx.token` with write permissions.
- **TimescaleDB** expects the **TimescaleDB extension** to be installed/enabled in the target database, because `bar_persistence` calls `create_hypertable(...)`.
  - At startup, `bar_persistence` will attempt (best-effort) to create the schema + tables:
    - Schema: `timescale.schema` (default: `warehouse`)
    - Tables: `warehouse.bar__time`, `warehouse.bar__tick_imbalance`
  - The connecting DB user must be allowed to `CREATE SCHEMA`, `CREATE TABLE`, and run `create_hypertable`.
  - Reference DDL is also checked into `sql/` (`create_table_bar__time.sql`, `create_table_bar__tick_imbalance.sql`).

### Network

- Outbound WS:
  - `wss://stream.binance.com:9443`
  - `wss://fstream.binance.com`
- Inbound gRPC: default ports are `500xx` (see config).
- Prometheus metrics: each service also exposes `GET /metrics` on **(service port + 1000)**.

## Configuration (`.toml` + env)

Raven loads configuration in this order:

1. `test.toml` (always loaded first as defaults)
2. `${RUN_MODE}.toml` (optional, e.g. `prod.toml`)
3. `local.toml` (optional, for uncommitted local overrides)
4. Environment variables with prefix `RAVEN__` (double-underscore separator)

Examples:

```bash
export RUN_MODE=prod
export RAVEN__INFLUX__TOKEN="my-secret-token"
export RAVEN__SERVER__PORT_BINANCE_SPOT=50099
```

### Routing section: venues + symbol mapping

Raven supports:

- **Venue selection**: `routing.venue_include` / `routing.venue_exclude`
- **Venue-specific symbol mapping**: `routing.symbol_map`

Example (spot uses `PEPEUSDT`, futures uses `1000PEPEUSDT`):

```toml
[routing]
venue_include = []
venue_exclude = []

[routing.symbol_map]
"PEPE/USDT" = { BINANCE_FUTURES = "1000PEPEUSDT" }
```

## Services & binaries

### Sources (collectors)

- `binance_spot` (default `50001`)
- `binance_futures` (default `50002`)

Collectors only connect/subscribe when their streams are started via **Control**.

### Processors (feature makers)

- `raven_timebar` (default `50051`)
- `raven_tibs` (default `50052`)

These services subscribe to sources. With the “wire first” rule, they will keep retrying until the relevant source stream is started.

### Persistence

- `tick_persistence` (default `50091`) → InfluxDB
- `bar_persistence` (default `50092`) → TimescaleDB

## Control plane: `ravenctl`

`ravenctl` is the intended way to run Raven.

### Build

```bash
cargo build --release
```

### Start infrastructure (all services)

```bash
./target/release/ravenctl start
```

This starts all service processes based on the service registry and writes PID/log files under:

- `~/.raven/log/*.pid`
- `~/.raven/log/*.log`

### Start a pipeline (instrument + venues)

Canonical instrument input:

```bash
./target/release/ravenctl start --symbol ETH --base USDC
```

Venue selection:

```bash
./target/release/ravenctl start --symbol ETH --base USDC --venue-include BINANCE_SPOT --venue-include BINANCE_FUTURES
./target/release/ravenctl start --symbol ETH --base USDC --venue-exclude BINANCE_FUTURES
```

What this does (per venue):

1. Starts downstream collections first:
   - `tick_persistence` (TRADE + ORDERBOOK)
   - `bar_persistence` (CANDLE; subscribes to both `raven_timebar` + `raven_tibs`)
   - `raven_timebar` (CANDLE)
   - `raven_tibs` (CANDLE)
2. Starts collectors last:
   - source TRADE + ORDERBOOK streams

### Stop a pipeline (instrument + venues)

```bash
./target/release/ravenctl stop --symbol ETH --base USDC
./target/release/ravenctl stop --symbol ETH --base USDC --venue BINANCE_FUTURES
```

### Stop everything

```bash
./target/release/ravenctl shutdown
```

Or stop a single service process:

```bash
./target/release/ravenctl --service binance_spot shutdown
```

### Status / introspection

```bash
./target/release/ravenctl status
./target/release/ravenctl user
```

## Protocol note (if you write your own client)

`MarketDataRequest` includes a **required** `venue` field:

- `symbol`: the **venue symbol** (e.g. `ETHUSDC` or `1000PEPEUSDT`)
- `venue`: e.g. `BINANCE_SPOT`, `BINANCE_FUTURES`
- `data_type`: `TRADE`, `ORDERBOOK`, `CANDLE`, ...

And remember: **you must call `Control.StartCollection` before `MarketData.Subscribe` will succeed**.

Control requests use the same concept:

- `ControlRequest.symbol`: venue symbol
- `ControlRequest.venue`: venue id (e.g. `BINANCE_SPOT`)

## Do we still need `start_services.sh` and `stop_services.sh`?

Not strictly.

- `start_services.sh` is just a thin wrapper over `ravenctl start` (infra start).
- `stop_services.sh` duplicates `ravenctl shutdown` behavior (kill services via PID files).

Recommendation: keep them only if you like the convenience; otherwise you can delete them and use `ravenctl` directly.
