# src/bin/persist/ – Persistence binaries

Binaries that subscribe to market-data streams and write to InfluxDB (raw data warehouse) or TimescaleDB (derived data mart).

## Binaries

| Binary | Subscribes to | Writes to | Config |
|--------|----------------|-----------|--------|
| `tick_persistence` | All raw data types (TRADE, ORDERBOOK, FUNDING, LIQUIDATION, OPEN_INTEREST, TICKER, PRICE_INDEX) | InfluxDB v2 | `influx.*` |
| `bar_persistence` | CANDLE (tibs, trbs, vibs, vpin) | TimescaleDB `mart.*` | `timescale.*` |
| `kline_persistence` | CANDLE (klines, 1m) | TimescaleDB `mart.bar__kline` | `timescale.*` |

## InfluxDB measurements (tick_persistence)

| Measurement | Source | Description |
|-------------|--------|-------------|
| `trades` | `binance_spot`, `binance_futures`, `deribit_trades` | Trade executions |
| `orderbook` | `binance_spot`, `binance_futures` | Best bid/ask snapshots with spread, mid, imbalance |
| `funding_rate` | `binance_futures_funding` | Mark price, index price, funding rate, next funding time |
| `liquidation` | `binance_futures_liquidations` | Forced liquidation events |
| `open_interest` | `binance_futures_oi` | Open interest in contracts and notional value |
| `options_ticker` | `binance_options`, `deribit_option` | OI, IV, mark/bid/ask prices, underlying |
| `price_index` | `deribit_index` | Underlying BTC/USD index price |

## Implementation notes

- Entrypoints: `tick_persistence.rs`, `bar_persistence.rs`, `kline_persistence.rs`.
- DB logic lives in `crate::db::influx` (raw data) and `crate::db::timescale` (derived bars/klines).
- **Upstream routing**: `tick_persistence` uses composite keys (`VENUE:DATA_TYPE`) to route to the correct upstream service for each data type.
- **Auto-start**: `tick_persistence` auto-starts wildcard (`*`) streams for all-market services (funding, liquidation, OI, options, deribit). Per-symbol streams (spot/futures trade+orderbook) are started via `Control.StartCollection`.
- **Start order**: ravenctl starts persistence before collectors so that when streams start, persist processes are already subscribed.
- **Klines**: 1-minute candles from Binance Futures; TimescaleDB continuous aggregates handle higher intervals (5m, 15m, 1h, etc.).

When changing write format or schema, update both the writer in `crate::db` and the reference DDL in `sql/` if applicable.
