# src/db/ – Database writers

Persistence layer for **InfluxDB v2** (staging buffer) and **TimescaleDB** (warehouse + mart for derived analytics: bars, klines). Used by the binaries in `src/bin/persist/`.

## Layout

| Module | Role |
|--------|------|
| `influx` | InfluxDB client and write logic; all raw data types (trades, orderbook, funding, liquidation, OI, options ticker, price index). |
| `timescale` | PostgreSQL/TimescaleDB: schema, bar tables, kline table. |

## InfluxDB (staging buffer)

- **Config**: `config::InfluxConfig` (url, org, bucket, token).
- **Worker / persistence**: `influx/worker.rs`, `influx/persistence.rs` – batch and write points; handle retries and errors.
- **Schema**: schema-on-write; measurements documented below.

### Measurements

| Measurement | Source | Tags | Fields |
|-------------|--------|------|--------|
| `trades` | `binance_spot`, `binance_futures`, `deribit_trades` | symbol, exchange, side | price, quantity |
| `orderbook` | `binance_spot`, `binance_futures` | symbol, exchange | bid_price, bid_qty, ask_price, ask_qty, spread, mid_price, imbalance |
| `funding_rate` | `binance_futures_funding` | symbol, exchange | rate, mark_price, index_price, next_funding_time |
| `liquidation` | `binance_futures_liquidations` | symbol, exchange, side | price, quantity |
| `open_interest` | `binance_futures_oi` | symbol, exchange | open_interest, open_interest_value |
| `options_ticker` | `binance_options`, `deribit_option` | symbol, exchange | open_interest, mark_iv, best_bid_price, best_ask_price, mark_price, underlying_price, last_price, bid_iv, ask_iv |
| `price_index` | `deribit_index` | index_name, exchange | price |

### Upstream routing

The `InfluxWorker` selects upstream by composite key `"VENUE:DATA_TYPE"` (e.g. `BINANCE_FUTURES:FUNDING`), falling back to `"VENUE"` for generic trade/orderbook streams.

## TimescaleDB (warehouse + mart)

- **Config**: `config::TimescaleConfig` (connection, schema name, default `mart`).
- **Schema**: `timescale/schema.rs` – creates dimension tables and fact hypertables (`create_hypertable`).
- **Dimensions**: `dim_symbol`, `dim_exchange`, `dim_interval` (regular PostgreSQL tables).
- **Symbol lifecycle**: `dim_symbol` supports soft-deletion with `is_deleted` and `deleted_date`.
- **Fact tables**: `bar__tick_imbalance`, `bar__volume_imbalance`, `bar__vpin`, `bar__kline` with `*_id` foreign keys to dimensions.
- **Writer path**: `timescale/dim_cache.rs` resolves and caches dimension IDs before fact inserts.
- **Init path**: `timescale/init.rs` seeds dimensions at startup and logs added/existing/reactivated/soft-deleted symbols.
- **Reference DDL**: `sql/create_table_bar__*.sql`; keep in sync with code when changing schema.

## Conventions

- **Errors**: use `thiserror`; retry transient failures; log and surface permanent failures.
- **Batching**: prefer batching writes for throughput; respect Influx/Timescale limits.
- **Schema changes**: update both Rust code and `sql/` DDL; consider migrations if needed for production.
- `raven_init` startup dependency preflight lives in `src/bin/persist/dependency_check.rs`; if DB requirements or port-bearing services change, keep that check logic consistent.
