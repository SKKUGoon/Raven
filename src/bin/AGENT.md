# src/bin/ – Executables

All Raven executables live here. Each binary is a separate process; `ravenctl` starts and coordinates them via gRPC Control.

## Layout

| Subdir | Binaries | Purpose |
|--------|----------|--------|
| `raw/` | `binance_spot`, `binance_futures`, `binance_futures_klines`, `binance_futures_funding`, `binance_futures_liquidations`, `binance_futures_oi`, `binance_options`, `deribit_option`, `deribit_trades`, `deribit_index` | Exchange data collectors (WebSocket/REST → gRPC streams). |
| `persist/` | `tick_persistence`, `bar_persistence`, `kline_persistence`, `raven_init` | Subscribe to streams and write to InfluxDB or TimescaleDB; `raven_init` runs one-shot dependency preflight + dimension seeding. |
| `statistics/` | `raven_tibs`, `raven_trbs`, `raven_vibs`, `raven_vpin` | Feature/bar makers; consume TRADE and emit CANDLE. |
| `ravenctl/` | `ravenctl` | CLI and control plane: start/stop services and collections. |
| `common/` | (shared code) | Helpers used by multiple binaries (e.g. service setup). |

## Collectors (raw/)

| Binary | Connection | Coverage | Produces |
|--------|-----------|----------|----------|
| `binance_spot` | WS per-symbol | Per `StartCollection` | TRADE, ORDERBOOK |
| `binance_futures` | WS per-symbol | Per `StartCollection` | TRADE, ORDERBOOK |
| `binance_futures_klines` | WS sharded (11 conn × 50 sym) | All USDT perps (auto) | CANDLE (1m) |
| `binance_futures_funding` | WS single (`!markPrice@arr@1s`) | All symbols (all-market) | FUNDING |
| `binance_futures_liquidations` | WS single (`!forceOrder`) | All symbols (all-market) | LIQUIDATION |
| `binance_futures_oi` | REST poll (5s) | From `binance_rest.symbols` config | OPEN_INTEREST |
| `binance_options` | REST poll (10s) | All BTC options (auto-filtered) | TICKER |
| `deribit_option` | WS single (JSON-RPC) | All BTC options (wildcard) | TICKER |
| `deribit_trades` | WS single (JSON-RPC) | All BTC options (wildcard) | TRADE |
| `deribit_index` | WS single (JSON-RPC) | BTC/USD index | PRICE_INDEX |

## Data flow

```
Exchanges (Binance Spot/Futures, Deribit)
  │
  ▼
Collectors (raw/)
  │ TRADE, ORDERBOOK, CANDLE, FUNDING, LIQUIDATION, OPEN_INTEREST, TICKER, PRICE_INDEX
  │
  ├──────────────────────────────────► Tick Persistence ──► InfluxDB
  │                                    (TRADE, ORDERBOOK)
  │
  ├──► Feature Makers (statistics/)
  │      TRADE ──► CANDLE
  │        │
  │        └────────────────────────► Bar Persistence ──► TimescaleDB
  │                                    (CANDLE: TIBS, TRBS, VIBS, VPIN)
  │
  └──► Kline Persistence ──────────► TimescaleDB
       (CANDLE from klines, 1m)       (bar__kline)
```

- **Collectors** (raw): connect to exchange WS/REST, parse, expose `MarketData.Subscribe`. Per-symbol collectors require `Control.StartCollection`; all-market streams (`binance_futures_funding`, `binance_futures_liquidations`, `binance_futures_klines`) stream automatically for all symbols.
- **Feature makers** (statistics): subscribe to collector TRADE, run bar logic from `crate::features`, expose CANDLE via `MarketData.Subscribe`.
- **Persistence**: subscribe to TRADE/ORDERBOOK or CANDLE and write to DB; started downstream-first by ravenctl.

## Conventions

- Each binary typically: loads config, builds a `RavenService` (or equivalent) with Control + MarketData impl, binds to port from config, runs `serve_with_market_data`.
- Metrics: each service exposes Prometheus on **gRPC port + 1000** (see `crate::service`).
- Reuse library types from `crate::domain`, `crate::config`, `crate::service`, `crate::features`, `crate::db`; avoid duplicating logic in bin code.
- If a port-bearing service is added/removed/renumbered, update `src/bin/persist/dependency_check.rs` so `raven_init` catches missing dependencies and port conflicts correctly.

See per-subdir `AGENT.md` for raw, persist, statistics, and ravenctl.
