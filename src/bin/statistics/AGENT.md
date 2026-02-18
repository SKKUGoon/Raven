# src/bin/statistics/ – Feature / bar services

Long-lived services that consume TRADE (and optionally ORDERBOOK) and emit **CANDLE** (bar) streams. Each service runs one task per `(symbol, venue, data_type)`; adding a symbol adds a stream task inside the same process.

Time-based candles are no longer aggregated here — use **Binance Klines** (1m) from `binance_futures_klines` instead. TimescaleDB handles further aggregation (5m, 15m, 1h, etc.).

## Binaries

| Binary | Feature | Input | Config / spec |
|--------|---------|--------|----------------|
| `raven_tibs` | Tick imbalance bars | TRADE | `ServiceSpec`: tibs_small, tibs_large; `tibs.*` config. |
| `raven_trbs` | Tick run bars | TRADE | `ServiceSpec`: trbs_small, trbs_large. |
| `raven_vibs` | Volume imbalance bars | TRADE | `ServiceSpec`: vibs_small, vibs_large. |
| `raven_vpin` | VPIN | TRADE | `port_vpin`; streamed, not persisted by default. |

## Implementation notes

- Entrypoints: `raven_tibs.rs`, `raven_trbs.rs`, `raven_vibs.rs`, `raven_vpin.rs`.
- Bar logic lives in `crate::features` (`tibs`, `trbs`, `vibs`, `vpin`). Bins wire upstream gRPC subscriptions to the feature implementation and expose CANDLE via `MarketData.Subscribe`.
- **ServiceSpec**: TIBS/TRBS/VIBS use small/large specs (different bar params); ports and specs come from `config` and `service::spec`.
- **Wire-first**: these services subscribe to source(s); if the source stream is not started yet, they retry until it is (or config timeout).

When adding a new bar type: implement the feature in `crate::features`, add a bin here that subscribes to the right upstream and exposes CANDLE, and register ports/specs in config and ravenctl.
