# src/service/ – gRPC server and stream management

Server wiring for **Control** and **MarketData** gRPC services, plus stream lifecycle and service specs.

## Layout

| Module | Purpose |
|--------|--------|
| `mod.rs` | `RavenService`: runs Control + MarketData server and metrics server (port + 1000). |
| `stream_key` | `StreamKey`, `StreamDataType` – key for streams (symbol, venue, data_type). |
| `stream_manager` | `StreamManager`, `StreamWorker` – create and manage gRPC streams; feed from source/feature logic. |
| `spec` | `ServiceSpec`, `ImbalanceBarSpec` – small/large bar specs and ports for TIBS/TRBS/VIBS. |

## Stream data types

`StreamDataType` supports: `TRADE`, `ORDERBOOK`, `CANDLE`, `FUNDING`, `LIQUIDATION`, `OPEN_INTEREST`, `TICKER`, `PRICE_INDEX`. Each collector/service exposes one or more of these via `MarketData.Subscribe`.

## Conventions

- **Control**: `StartCollection` / `StopCollection` (and similar) drive when streams are created and when sources subscribe to the exchange. Implementations are in the bins (raw, statistics) that own the streams.
- **MarketData**: `Subscribe` returns a stream of `MarketDataMessage`; the implementation pushes trade/orderbook/candle/funding/liquidation/ticker/etc. from the appropriate worker or feature.
- **Metrics**: each service binds metrics HTTP on gRPC port + 1000; use `crate::telemetry` for labels and counters.
- Do not put exchange-specific or bar-algorithm logic here; keep this layer to wiring and stream keys/specs.
- Any service-spec port additions/removals must be mirrored in `src/bin/persist/dependency_check.rs` so `raven_init` preflight checks catch conflicts and missing dependencies.
