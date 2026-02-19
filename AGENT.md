# Raven – Agent guide (root)

Raven is a **modular market-data platform** in Rust. Use this file and per-directory `AGENT.md` files to navigate and change the codebase safely.

## v3 breaking-change notes

- `MarketDataMessage.exchange` was removed from `proto/market_data.proto` and reserved. Use `venue` and `producer` instead.
- TIBS/TRBS/VIBS size bounds are percentage-only (`size_min_pct`, `size_max_pct`). Legacy absolute bounds (`size_min`, `size_max`) were removed.
- Legacy `timebar_minutes` config/service wiring was removed. Use `binance_futures_klines` (1m CANDLE source) with `kline_persistence` for minute bars.

## Mental model

- **Sources (collectors)** → ingest exchange data (TRADE, ORDERBOOK, CANDLE, FUNDING, LIQUIDATION, OPEN_INTEREST, TICKER, PRICE_INDEX) over WebSockets and REST APIs.
- **Processors (feature makers)** → consume TRADE and emit bar/feature streams (imbalance bars, VPIN, etc.) as CANDLE.
- **Persistence** → subscribe to streams and write to InfluxDB (staging buffer) or TimescaleDB (warehouse + mart for bars/klines).
- **Control plane** → `ravenctl` starts/stops services and collections via gRPC; start order is downstream-first, then collectors.

## Data flow diagram

```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│                              EXCHANGE APIs (External)                                │
├──────────────────────┬───────────────────────────────────┬───────────────────────────┤
│   Binance Spot WS    │       Binance Futures WS/REST     │   Deribit WS (JSON-RPC)   │
└────────┬─────────────┴───────┬──────┬──────┬──────┬──────┴──────┬──────┬──────┬──────┘
         │                     │      │      │      │             │      │      │
         ▼                     ▼      │      │      │             ▼      ▼      ▼
┌─────────────────┐ ┌────────────────┐│      │      │  ┌──────────────┬─────────┬──────────────┐
│  binance_spot   │ │binance_futures ││      │      │  │deribit_option│ deribit │ deribit_index│
│  :50001         │ │  :50002        ││      │      │  │  :50008      │ _trades │   :50010     │
│  WS per-symbol  │ │  WS per-symbol ││      │      │  │  WS single   │ :50009  │  WS single   │
│ produces:       │ │ produces:      ││      │      │  │ produces:    │produces:│ produces:    │
│  TRADE ─────────┼─┤  TRADE ────────┼┼───┐  │      │  │  TICKER      │ TRADE   │ PRICE_INDEX  │
│  ORDERBOOK ─────┼─┤  ORDERBOOK ────┼┴┐  │  │      │  └──────────────┴─────────┴──────────────┘
└─────────────────┘ └────────────────┘ │  │  │      │
                                       │  │  ▼      ▼               ▼
              ┌───────────────────────┐│  │ ┌───────────────┐ ┌────────────────┐ ┌──────────────────┐
              │binance_futures_klines ││  │ │ b_f_funding   │ │b_f_liquidations│ │  b_f_oi          │
              │  :50003               ││  │ │  :50005       │ │  :50004        │ │  :50006          │
              │  WS sharded           ││  │ │  WS single    │ │  WS single     │ │  REST poll       │
              │  1m interval (all)    ││  │ │ !markPrice    │ │ !forceOrder    │ │  per-symbol      │
              │ produces: CANDLE ─────┼┼──┼─┤ @arr@1s       │ │ (all symbols)  │ │                  │
              └───────────────────────┘│  │ │ (all symbols) │ │ produces:      │ │ produces:        │
                    │                  │  │ │ produces:     │ │  LIQUIDATION   │ │  OPEN_INTEREST   │
                    │                  │  │ │  FUNDING      │ └────────────────┘ └──────────────────┘
                    │                  │  │ └───────────────┘
                    │   ┌──────────────┼──┘ ┌───────────────────────┐
                    │   │              │    │   binance_options     │
                    │   │              │    │   :50007              │
                    │   │              │    │   REST poll (all BTC) │
                    │   │              │    │  produces: TICKER     │
                    │   │              │    └───────────────────────┘
                    │   │              │
════════════════════╪═══╪══════════════╪══════════════ gRPC boundary ════════════════════════
                    │   │              │
         ┌──────────┘   │              │
         │    ┌─────────┘              │
         │    │    ┌───────────────────┘
         ▼    ▼    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        FEATURE MAKERS (statistics/)                         │
│                                                                             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐                        │
│  │   tibs   │ │   trbs   │ │   vibs   │ │   vpin   │                        │
│  │ :50054/53│ │ :50055/56│ │ :50057/58│ │  :50059  │                        │
│  └────┬─────┘ └────┬─────┘ └─────┬────┘ └────┬─────┘                        │
│       │            │             │           │                              │
│  consumes: TRADE   │        consumes: TRADE  │                              │
│  produces: CANDLE  │        produces: CANDLE │                              │
└───────┼────────────┼─────────────┼───────────┼──────────────────────────────┘
        │            │             │           │
        └──────┬─────┴──────┬──────┘           │
               │            │                  │
════════════════════════════════════════════════════ gRPC boundary ════════════════════════
               │            │
               ▼            ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│                          PERSISTENCE (persist/)                              │
│                                                                              │
│  ┌───────────────────┐  ┌───────────────────┐  ┌───────────────────┐         │
│  │  tick_persistence │  │  bar_persistence  │  │ kline_persistence │         │
│  │  :50091           │  │  :50092           │  │  :50093           │         │
│  │                   │  │                   │  │                   │         │
│  │ consumes:         │  │ consumes:         │  │ consumes:         │         │
│  │  TRADE ◄──────────┼──┼── from collectors │  │  CANDLE ◄─────────┼─ from   │
│  │  ORDERBOOK ◄──────┼──┼── from collectors │  │   from b_f_klines │  above  │
│  │                   │  │  CANDLE ◄─────────┼──┼── from statistics │         │
│  └────────┬──────────┘  └────────┬──────────┘  └────────┬──────────┘         │
│           │                      │                      │                    │
└───────────┼──────────────────────┼──────────────────────┼────────────────────┘
            │                      │                      │
            ▼                      ▼                      ▼
     ┌─────────────┐       ┌──────────────┐       ┌──────────────┐
     │  InfluxDB   │       │ TimescaleDB  │       │ TimescaleDB  │
     │ (staging    │       │ (warehouse + │       │ (warehouse + │
     │  buffer)    │       │  mart)       │       │  mart)       │
     │  trades     │       │ fact__tick_* │       │ fact__kline  │
     │  orderbook  │       │ fact__vol_*  │       │              │
     │  funding    │       │ fact__vpin   │       │              │
     │  liquidation│       └──────────────┘       └──────────────┘
     │  open_intst │
     │  options_tkr│
     │  price_index│
     └─────────────┘
```

## Microservices

### Collectors (raw data)

| Binary | Port | Connection | Stream / Endpoint | Symbols | Produces |
|--------|------|------------|-------------------|---------|----------|
| `binance_spot` | 50001 | WS per-symbol | `{sym}@trade`, `{sym}@depth20@100ms` | Per `StartCollection` | TRADE, ORDERBOOK |
| `binance_futures` | 50002 | WS per-symbol | `{sym}@aggTrade`, `{sym}@depth20@100ms` | Per `StartCollection` | TRADE, ORDERBOOK |
| `binance_futures_klines` | 50003 | WS sharded (11 connections × 50 sym) | `{sym}@kline_1m` | All USDT perps (auto-discovered) | CANDLE |
| `binance_futures_liquidations` | 50004 | WS single | `!forceOrder` | All symbols (all-market) | LIQUIDATION |
| `binance_futures_funding` | 50005 | WS single | `!markPrice@arr@1s` | All symbols (all-market) | FUNDING |
| `binance_futures_oi` | 50006 | REST poll (5s) | `GET /fapi/v1/openInterest?symbol={sym}` | From `binance_rest.symbols` config | OPEN_INTEREST |
| `binance_options` | 50007 | REST poll (10s) | `GET /eapi/v1/ticker` | All BTC options (auto-filtered) | TICKER |
| `deribit_option` | 50008 | WS single (JSON-RPC) | `ticker.BTC-OPTION.100ms` | All BTC options (wildcard channel) | TICKER |
| `deribit_trades` | 50009 | WS single (JSON-RPC) | `trades.BTC-OPTION.100ms` | All BTC options (wildcard channel) | TRADE |
| `deribit_index` | 50010 | WS single (JSON-RPC) | `deribit_price_index.btc_usd` | BTC/USD only | PRICE_INDEX |

### Feature makers (derived data)

| Binary | Port(s) | Input | Output | Persisted to |
|--------|---------|-------|--------|-------------|
| `raven_tibs` | 50054 (small), 50053 (large) | TRADE from collectors | CANDLE (tick imbalance bars) | `mart.fact__tick_imbalance` |
| `raven_trbs` | 50055 (small), 50056 (large) | TRADE from collectors | CANDLE (tick run bars) | `mart.fact__tick_imbalance` |
| `raven_vibs` | 50057 (small), 50058 (large) | TRADE from collectors | CANDLE (volume imbalance bars) | `mart.fact__volume_imbalance` |
| `raven_vpin` | 50059 | TRADE from collectors | CANDLE (VPIN buckets) | `mart.fact__vpin` |

See `src/features/docs/` for detailed English + Korean documentation on each bar type's logic, hyperparameters, and math.

### Persistence

| Binary | Port | Subscribes to | Writes to |
|--------|------|---------------|-----------|
| `tick_persistence` | 50091 | All raw data (see below) | InfluxDB (staging buffer) |
| `bar_persistence` | 50092 | CANDLE from `raven_tibs`, `raven_trbs`, `raven_vibs`, `raven_vpin` | TimescaleDB `mart.*` (warehouse + mart facts) |
| `kline_persistence` | 50093 | CANDLE from `binance_futures_klines` | TimescaleDB `mart.fact__kline` (warehouse + mart fact) |
| `raven_init` | n/a | Startup seed process | TimescaleDB dimensions (`dim__coin`, `dim__quote`, `dim__exchange`, `dim__interval`) |

`tick_persistence` auto-starts wildcard streams for all-market services and writes these InfluxDB measurements:

| Measurement | Source(s) | Tags | Key fields |
|-------------|-----------|------|------------|
| `trades` | `binance_spot`, `binance_futures`, `deribit_trades` | symbol, exchange, side | price, quantity |
| `orderbook` | `binance_spot`, `binance_futures` | symbol, exchange | bid/ask price+qty, spread, mid_price, imbalance |
| `funding_rate` | `binance_futures_funding` | symbol, exchange | rate, mark_price, index_price, next_funding_time |
| `liquidation` | `binance_futures_liquidations` | symbol, exchange, side | price, quantity |
| `open_interest` | `binance_futures_oi` | symbol, exchange | open_interest, open_interest_value |
| `options_ticker` | `binance_options`, `deribit_option` | symbol, exchange | OI, IV, mark, bid/ask prices, underlying |
| `price_index` | `deribit_index` | index_name, exchange | price |

TimescaleDB `mart` uses a star schema:

| Table type | Tables | Notes |
|------------|--------|-------|
| Dimensions | `dim__coin`, `dim__quote`, `dim__exchange`, `dim__interval` | Regular PostgreSQL tables with `is_deleted` + `deleted_date` soft-delete columns |
| Facts (hypertables) | `fact__tick_imbalance`, `fact__volume_imbalance`, `fact__vpin`, `fact__kline` | `coin_id`, `quote_id`, `exchange_id`, `interval_id` are NOT NULL foreign keys |

### Control plane

| Binary | Purpose |
|--------|---------|
| `ravenctl` | CLI: start/stop services and collections, graph topology, status |

### Startup order (downstream-first)

```
ravenctl start
  0. one-shot init (raven_init; seeds mart.dim_* then exits)
  1. persistence   (tick_persistence, bar_persistence, kline_persistence)
  2. statistics    (raven_tibs, raven_trbs, raven_vibs, raven_vpin)
  3. collectors    (binance_spot, binance_futures, binance_futures_klines)
  4. all-market WS (binance_futures_funding, binance_futures_liquidations)
  5. REST pollers  (binance_futures_oi, binance_options)
  6. deribit       (deribit_option, deribit_trades, deribit_index)
```

## Key concepts

- **Terminology**
  - Coin: base asset (`BTC`, `ETH`, `XRP`).
  - Quote: quoting asset (`USDT`, `USD`, `USDC`, `BTC`).
  - Symbol: exchange lookup token composed from coin+quote with venue-specific formatting (e.g. `BTCUSDT`, `btc_usd`).
  - Exchange and venue are equivalent in Raven terminology.
- **Instrument** = base/quote (e.g. ETH/USDC). **Venue symbol** = exchange-specific (e.g. `ETHUSDC`, `1000PEPEUSDT`). Routing and `routing.symbol_map` resolve instrument → venue symbols.
- **Venues**: `BINANCE_SPOT`, `BINANCE_FUTURES`, `BINANCE_OPTIONS`, `DERIBIT`.
- **Collections** are started by `Control.StartCollection` (symbol, venue, data_type). Most services only stream after a collection is started; exceptions: `binance_futures_klines` (auto-starts all USDT perps), `binance_futures_funding` and `binance_futures_liquidations` (all-market WS, always streaming).
- **Ports**: each service has a gRPC port (see `config.rs` / TOML); metrics are on **port + 1000**.
- **Port allocation policy**:
  - Raw data services: `50001` to `50050`
  - Statistics services: `50050` to `50090`
  - Persistence services: `50091` to `50099`
- **Klines**: 1-minute interval from Binance Futures for all USDT perpetuals. TimescaleDB continuous aggregates handle upsampling to 5m, 15m, 1h, etc.

## Layout

| Path | Purpose |
|------|--------|
| `src/` | Library and binary entrypoints; see `src/AGENT.md`. |
| `src/bin/` | All executables: raw collectors, persist, statistics, ravenctl. |
| `proto/` | gRPC definitions (`market_data.proto`, `control.proto`). |
| `sql/` | Timescale schema SQL and optional migration utilities. |
| `*.toml` | Config (see README); `test.toml` is compiled-in default. |

## Build & run

- `cargo build --release` → binaries in `target/release/`.
- `cargo run --release --bin ravenctl -- start` → start all services.
- `cargo run --release --bin ravenctl -- collect --coin ETH --quote USDC` → start pipeline for that instrument.
- See `README.md` for prerequisites (Rust, InfluxDB v2, PostgreSQL + TimescaleDB), config loading order, and `ravenctl` commands.

## Conventions

- **Rust 2021**. Use existing patterns: `thiserror` for errors, `tracing` for logs, `serde` for config.
- **Config**: `Settings` in `src/config.rs`; override via TOML and `RAVEN__*` env vars.
- **gRPC**: types live in `crate::proto` (generated from `proto/`). Do not hand-edit generated code.
- **v3 proto compatibility**: do not reuse reserved protobuf field numbers/names (notably `MarketDataMessage` field `6` / `exchange`).
- **Tests**: unit tests in modules; integration-style tests in `tests/`; some binaries have `#[cfg(test)]` or dedicated test binaries.
- **Port-bearing service lifecycle**: whenever a service with a configured port is added, removed, or has a port changed, update `raven_init` dependency checks in `src/bin/persist/dependency_check.rs` and re-run `cargo run --bin raven_init` (or `cargo check --bin raven_init`) to verify missing dependencies are reported correctly.
- **Argument-heavy construction**: if a function or constructor grows many parameters (or triggers clippy `too_many_arguments` / `type_complexity`), prefer an argument struct with a chainable builder API.
- **Builder style**: for internal builders in this repository, prefer infallible `.build()` that returns the target type directly and enforces required fields internally (panic with clear `missing <field>` messages if violated).

### When adding or deleting DB tables

- Define or remove table DDL and insert SQL together (no separate model profile layer currently).
- Update DDL in `sql/timescale_schema.sql` and keep executor wiring in `src/db/timescale/schema.rs`.
- Wire corresponding insert/read usage in writers (`src/db/timescale/bars.rs`, `src/db/timescale/kline.rs`, etc.).
- Keep dimensions/facts naming and FK columns consistent (`dim__*`, `fact__*`).
- Update docs (`README.md`, `src/db/AGENT.md`) and run `cargo check`.

When editing a subsystem, read the `AGENT.md` in that directory first.
