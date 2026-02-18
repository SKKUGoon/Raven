# Raven

Raven is a modular market-data platform in Rust: **sources** ingest exchange data, **processors** build features/bars, and **persistence** stores ticks and bars. Everything is controlled via a gRPC **Control plane** (`ravenctl`).

## Operational model (how Raven runs)

Your mental model is the right one; here is how it maps to the current code:

- **1) Instrument + venues**
  - Preferred: **canonical instrument** + quote (base/quote) and let Raven resolve venue symbols:
    - `ravenctl start --symbol ETH --base USDC` (instrument = `ETH/USDC`)
  - Optional: restrict venues using:
    - `--venue BINANCE_SPOT` (single venue)
    - `--venue-include ...` / `--venue-exclude ...` (multi-venue selection)
  - Venue symbol differences (e.g. spot `PEPEUSDT` vs futures `1000PEPEUSDT`) are handled via `routing.symbol_map`.

- **2) Start = spin up services, then wire, then subscribe**
  - `ravenctl start` (no symbol) first runs one-shot `raven_init` (dimension seeding), then starts all long-lived service processes.
  - `ravenctl start --symbol ...` starts infra (if needed) and then starts **collections** in a strict order:
    - downstream first (persistence + feature makers)
    - collectors last (this is when exchange WebSocket subscriptions actually happen)
  - **Exception**: `binance_futures_klines` + `kline_persistence` are autonomous. If you run those binaries directly, they immediately start collecting/persisting klines for the configured symbols without `ravenctl start`.

- **3) Multi-symbol is “within the same process”**
- Services like `raven_tibs` / `raven_trbs` / `raven_vibs` are long-lived processes that run **one task per `(symbol, venue, datatype)`**.
  - Adding another symbol later does **not** require starting another `raven_tibs_*` process; it starts another stream task inside the existing service.

- **4) Data flow until you stop**
  - Collectors produce `TRADE` + `ORDERBOOK`.
  - Feature makers subscribe to `TRADE` and emit `CANDLE` feature streams (time bars, imbalance bars, etc.).
  - Persistence subscribes to upstream streams and writes to DB.
  - The pipeline runs until you call `ravenctl stop ...` (stop collections) or `ravenctl shutdown` (stop processes).

To see the topology and exact calls Raven will make:

- `ravenctl graph` (ASCII) / `ravenctl graph --format dot` (Graphviz)
- `ravenctl plan --symbol ETH --base USDC ...` (no execution; prints the exact ordered `start_collection(...)` calls)


## Prerequisites

### System (For Developers)

- **Rust toolchain**: Rust 2021 edition (install via `rustup`; `cargo` is required).
- **OS**: macOS/Linux recommended. `ravenctl` manages processes using system commands like `ps` + `kill`.
- **Filesystem**: `ravenctl` writes PID/log files under `~/.raven/log` (requires `$HOME` to be set and writeable).
- **Native TLS deps (Linux only)**: because Raven uses `native-tls` (WebSockets + gRPC), you’ll typically need system OpenSSL tooling such as `pkg-config` + `libssl-dev`.

### Databases

- **InfluxDB v2.x**: used by `tick_persistence` as a staging buffer for raw stream data (trades, orderbook, funding rates, liquidations, open interest, options tickers, price index).
- **PostgreSQL + TimescaleDB extension**: used by `bar_persistence` and `kline_persistence` as warehouse + mart storage for derived bars and klines.

Notes:
- **InfluxDB** is schema-on-write, but you must provision:
  - `influx.url`, `influx.org`, `influx.bucket`, and a valid `influx.token` with write permissions.
- **TimescaleDB** expects the **TimescaleDB extension** to be installed/enabled in the target database, because `bar_persistence` and `kline_persistence` call `create_hypertable(...)`.
  - At startup, `bar_persistence` will attempt (best-effort) to create the schema + tables:
    - Schema: `timescale.schema` (default: `mart`)
    - Dimension tables: `mart.dim__coin`, `mart.dim__quote`, `mart.dim__exchange`, `mart.dim__interval`
    - Fact tables: `mart.fact__tick_imbalance`, `mart.fact__volume_imbalance`, `mart.fact__vpin`
  - At startup, `kline_persistence` will attempt (best-effort) to create:
    - Fact table: `mart.fact__kline`
  - All dimensions track lifecycle with `is_deleted` and nullable `deleted_date` (soft delete).
  - Fact tables use NOT NULL foreign keys (`coin_id`, `quote_id`, `exchange_id`, `interval_id`) into the dimension tables.
  - Use `raven_init` to run dimension seeding explicitly; persistence services also run this init step at startup.
  - The connecting DB user must be allowed to `CREATE SCHEMA`, `CREATE TABLE`, and run `create_hypertable`.
  - Runtime schema source of truth is `sql/timescale_schema.sql` (executed by `src/db/timescale/schema.rs`).

### Network

- Outbound WS:
  - `wss://stream.binance.com:9443`
  - `wss://fstream.binance.com`
- Inbound gRPC: default ports are `500xx` (see config).
- Prometheus metrics: each service also exposes `GET /metrics` on **(service port + 1000)**.

## Configuration (`.toml` + env)

Raven loads configuration in this order:

1. Compiled-in defaults from `test.toml` (always loaded first)
2. A config file selected in this order:
   - `RAVEN_CONFIG_FILE=/absolute/path/to/prod.toml` (highest priority)
   - `./${RUN_MODE}.toml` (optional, e.g. `./prod.toml`)
   - `~/.config/raven/${RUN_MODE}.toml` (optional)
   - `~/.raven/${RUN_MODE}.toml` (optional)
   - `/etc/raven/${RUN_MODE}.toml` (optional)
3. `local.toml` (optional) or `RAVEN_LOCAL_CONFIG_FILE=/path/to/local.toml`
4. Environment variables with prefix `RAVEN__` (double-underscore separator)

Examples:

```bash
export RUN_MODE=prod
export RAVEN_CONFIG_FILE="/etc/raven/prod.toml"
export RAVEN__INFLUX__TOKEN="my-secret-token"
export RAVEN__SERVER__PORT_BINANCE_SPOT=50099
```

### `ravenctl setup` (recommended for system-wide installs)

If you installed `ravenctl` into `/usr/local/bin` and want to run it from anywhere, run:

```bash
ravenctl setup --config /etc/raven/prod.toml --run-mode prod
```

This persists the config location under `~/.raven/` and on future runs `ravenctl` will
auto-set `RAVEN_CONFIG_FILE` (and `RUN_MODE`) for itself and any services it spawns.

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
- `binance_futures_klines` (default `50003`) → auto-subscribes to configured kline symbols on startup

Collectors only connect/subscribe when their streams are started via **Control**, except for the autonomous kline collector described above.

### Processors (feature makers)

- `raven_tibs` (TIBS; configured via `ServiceSpec` entries `tibs_small` / `tibs_large`)
- `raven_trbs` (TRBS; configured via `ServiceSpec` entries `trbs_small` / `trbs_large`)
- `raven_vibs` (VIBS; configured via `ServiceSpec` entries `vibs_small` / `vibs_large`)
- `raven_vpin` (see config)

These services subscribe to sources. With the “wire first” rule, they will keep retrying until the relevant source stream is started.

### Persistence

- `tick_persistence` (default `50091`) → InfluxDB
- `bar_persistence` (default `50092`) → TimescaleDB
- `kline_persistence` (default `50093`) → TimescaleDB; auto-starts persistence for configured kline symbols
- `raven_init` (no port) → runs startup dimension seeding for TimescaleDB (`mart.dim_*`)

## Control plane: `ravenctl`

`ravenctl` is the intended way to run Raven.

### Install from GitHub Releases (Linux)

If you don't want to build from source, you can download a prebuilt release artifact and install the binaries on your server.

1) Choose a version and download the `.tar.gz`:

```bash
VERSION="2.5.3"
TARGET="x86_64-unknown-linux-gnu"

curl -fL -o "raven-v${VERSION}-${TARGET}.tar.gz" \
  "https://github.com/SKKUGoon/Raven/releases/download/v${VERSION}/raven-v${VERSION}-${TARGET}.tar.gz"
```

2) Extract it:

```bash
tar -xzf "raven-v${VERSION}-${TARGET}.tar.gz"
```

3) Install the binaries (system-wide) and verify:

```bash
DIR="raven-v${VERSION}-${TARGET}"

# (Optional) see what you got
ls -la "${DIR}"

# Install all compiled binaries into /usr/local/bin
sudo install -m 0755 "${DIR}/binance_spot"      /usr/local/bin/
sudo install -m 0755 "${DIR}/binance_futures"   /usr/local/bin/
sudo install -m 0755 "${DIR}/tick_persistence"  /usr/local/bin/
sudo install -m 0755 "${DIR}/bar_persistence"   /usr/local/bin/
sudo install -m 0755 "${DIR}/raven_init"        /usr/local/bin/
sudo install -m 0755 "${DIR}/raven_tibs"        /usr/local/bin/
sudo install -m 0755 "${DIR}/raven_trbs"        /usr/local/bin/
sudo install -m 0755 "${DIR}/raven_vibs"        /usr/local/bin/
sudo install -m 0755 "${DIR}/raven_vpin"        /usr/local/bin/
sudo install -m 0755 "${DIR}/ravenctl"          /usr/local/bin/

ravenctl --version
```

If your tarball layout differs, inspect it with:

```bash
tar -tzf "raven-v${VERSION}-${TARGET}.tar.gz" | head
```

### Build

```bash
cargo build --release
```

### Run locally (after build)

Run from the repo without installing:

```bash
./target/release/ravenctl start
```

Run a specific service directly:

```bash
./target/release/binance_futures_klines
./target/release/kline_persistence
./target/release/raven_init
```

Tip: you can also run from source:

```bash
cargo run --release --bin ravenctl -- start --symbol ETH --base USDC
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

Venue-symbol input (no base/quote parsing; you are responsible for the venue symbol):

```bash
./target/release/ravenctl start --symbol ETHUSDC --venue BINANCE_SPOT
```

Venue selection:

```bash
./target/release/ravenctl start --symbol ETH --base USDC --venue BINANCE_SPOT
./target/release/ravenctl start --symbol ETH --base USDC --venue-include BINANCE_SPOT --venue-include BINANCE_FUTURES
./target/release/ravenctl start --symbol ETH --base USDC --venue-exclude BINANCE_FUTURES
```

What this does (per venue):

1. Starts downstream collections first:
   - `tick_persistence` (TRADE + ORDERBOOK)
   - `bar_persistence` (CANDLE; subscribes to `raven_tibs` + `raven_trbs` + `raven_vibs` + `raven_vpin`)
   - `raven_tibs` (CANDLE)
   - `raven_trbs` (CANDLE)
   - `raven_vibs` (CANDLE)
   - `raven_vpin` (CANDLE)
2. Starts collectors last:
   - source TRADE + ORDERBOOK streams

Tip: to see the exact execution plan without running anything:

```bash
./target/release/ravenctl plan --symbol ETH --base USDC --venue-include BINANCE_SPOT
```

### Stop a pipeline (instrument + venues)

```bash
./target/release/ravenctl stop --symbol ETH --base USDC
./target/release/ravenctl stop --symbol ETH --base USDC --venue BINANCE_FUTURES
```

### Stop all collections (leave processes running)

```bash
./target/release/ravenctl stop-all
```

### Shutdown services (stop processes)

```bash
./target/release/ravenctl shutdown
```

Or shutdown a single service process:

```bash
./target/release/ravenctl --service binance_spot shutdown
```

### Status / introspection

```bash
./target/release/ravenctl status
./target/release/ravenctl user
./target/release/ravenctl graph
./target/release/ravenctl graph --format dot
```

## Protocol note (if you write your own client)

`MarketDataRequest` includes a **required** `venue` field:

- `symbol`: the **venue symbol** (e.g. `ETHUSDC` or `1000PEPEUSDT`)
- `venue`: e.g. `BINANCE_SPOT`, `BINANCE_FUTURES`
- `data_type`: `TRADE`, `ORDERBOOK`, `CANDLE`, ...

And remember: **for most services you must call `Control.StartCollection` before `MarketData.Subscribe` will succeed**. The `binance_futures_klines` service auto-starts streams for configured symbols, so `Subscribe` can succeed without a prior `StartCollection`.

Control requests use the same concept:

- `ControlRequest.symbol`: venue symbol
- `ControlRequest.venue`: venue id (e.g. `BINANCE_SPOT`)

## Python client example: subscribe to bar (CANDLE) streams

Raven exposes bar/feature streams as `DataType.CANDLE` via the `MarketData.Subscribe` gRPC stream on **feature services** (e.g. `raven_tibs`, `raven_trbs`, `raven_vibs`, `raven_vpin`).

Important notes:

- You typically **start the pipeline** with `ravenctl start ...` first (this will call `Control.StartCollection` for you).
- Then your client connects to a **feature service port** (for example `raven_tibs` on `50054`) and subscribes to `CANDLE`.

### 1) Generate Python gRPC stubs

From the repo root:

```bash
python -m venv .venv
source .venv/bin/activate
pip install grpcio grpcio-tools protobuf

mkdir -p python_client
python -m grpc_tools.protoc \
  -I ./proto \
  --python_out=./python_client \
  --grpc_python_out=./python_client \
  ./proto/market_data.proto ./proto/control.proto
```

### 2) Start a pipeline (example)

```bash
./target/release/ravenctl start --symbol ETH --base USDC --venue BINANCE_SPOT
```

### 3) Subscribe to candles from a feature service

Save as `python_client/subscribe_bars.py`:

```python
import grpc

import market_data_pb2
import market_data_pb2_grpc
import control_pb2
import control_pb2_grpc


def subscribe_candles(service_addr: str, symbol: str, venue: str) -> None:
    channel = grpc.insecure_channel(service_addr)
    md = market_data_pb2_grpc.MarketDataStub(channel)
    req = market_data_pb2.MarketDataRequest(
        symbol=symbol,
        venue=venue,
        data_type=market_data_pb2.CANDLE,
    )

    for msg in md.Subscribe(req):
        if msg.WhichOneof("data") != "candle":
            continue
        c = msg.candle
        print(
            f"{msg.venue} {c.symbol} interval={c.interval} ts={c.timestamp} "
            f"o={c.open} h={c.high} l={c.low} c={c.close} v={c.volume} ticks={c.total_ticks}"
        )


if __name__ == "__main__":
    # TIBS small default port (see config for other services).
    service_addr = "localhost:50054"
    symbol = "ETHUSDC"        # venue symbol
    venue = "BINANCE_SPOT"

    subscribe_candles(service_addr, symbol, venue)
```

