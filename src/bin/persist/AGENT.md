# src/bin/persist/ – Persistence binaries

Binaries that subscribe to market-data streams (TRADE, ORDERBOOK, or CANDLE) and write to InfluxDB or TimescaleDB.

## Binaries

| Binary | Subscribes to | Writes to | Config |
|--------|----------------|-----------|--------|
| `tick_persistence` | TRADE, ORDERBOOK | InfluxDB v2 | `influx.*` |
| `bar_persistence` | CANDLE (tibs, trbs, vibs, vpin) | TimescaleDB | `timescale.*`; tables in `warehouse.bar__*`. |
| `kline_persistence` | CANDLE (klines, 1m) | TimescaleDB | `timescale.*`; table `warehouse.bar__kline`. |

## Implementation notes

- Entrypoints: `tick_persistence.rs`, `bar_persistence.rs`, `kline_persistence.rs`.
- DB logic lives in `crate::db::influx` (ticks) and `crate::db::timescale` (bars/klines). These bins wire gRPC subscriptions to the appropriate writer.
- **Start order**: ravenctl starts persistence before collectors so that when TRADE/ORDERBOOK (or CANDLE) streams are started, persist processes are already subscribed.
- **Schema**: TimescaleDB tables and DDL reference are in `sql/`. Bar/kline persistence may create schema/tables at startup (best-effort).
- **Klines**: 1-minute candles from Binance Futures; TimescaleDB continuous aggregates handle higher intervals (5m, 15m, 1h, etc.).

When changing write format or schema, update both the writer in `crate::db` and the reference DDL in `sql/` if applicable.
