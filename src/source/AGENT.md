# src/source/ – Data sources

Abstraction and implementations for **exchange data ingestion**: WebSocket clients and REST pollers that parse incoming messages and produce the types used in gRPC streams (TRADE, ORDERBOOK, CANDLE, FUNDING, LIQUIDATION, OPEN_INTEREST, TICKER, PRICE_INDEX).

## Layout

| Module | Purpose |
|--------|--------|
| `binance` | Binance Spot, Futures, Futures Klines, Futures Funding (WS), Futures Liquidations (WS), Futures OI (REST), and Options (REST). |
| `deribit` | Deribit BTC options: three microservices — ticker, trades, price index (JSON-RPC over WebSocket). |
| `ws_sharding` | WebSocket sharding/connection helpers used by Binance klines and others. |

## Connection types

| Service | Connection | Coverage |
|---------|-----------|----------|
| `binance_spot` | WS per-symbol | Per `StartCollection` |
| `binance_futures` | WS per-symbol | Per `StartCollection` |
| `binance_futures_klines` | WS sharded (multiple connections) | All USDT perps (auto-discovered) |
| `binance_futures_funding` | WS single (`!markPrice@arr@1s`) | All symbols (all-market stream) |
| `binance_futures_liquidations` | WS single (`!forceOrder`) | All symbols (all-market stream) |
| `binance_futures_oi` | REST poll (5s interval) | From `binance_rest.symbols` config |
| `binance_options` | REST poll (10s interval) | All BTC options (auto-filtered) |
| `deribit_option` | WS single (JSON-RPC) | All BTC options (wildcard channel) |
| `deribit_trades` | WS single (JSON-RPC) | All BTC options (wildcard channel) |
| `deribit_index` | WS single (JSON-RPC) | BTC/USD only |

## Data flow

- Each source has a **worker** or **service** that holds the WS/REST connection and drives the stream.
- **Parsing** modules turn exchange-specific payloads into `proto::Trade`, `proto::OrderBookSnapshot`, `proto::Candle`, `proto::FundingRate`, `proto::Liquidation`, `proto::OpenInterest`, etc.
- The worker is typically driven by a bin in `src/bin/raw/` that also runs the gRPC server; the server's `StreamManager` (or equivalent) creates streams keyed by (symbol, venue, data_type) and feeds them from the worker.

## Conventions

- **Venue symbol**: sources use the exchange's symbol (e.g. `ETHUSDC`). No instrument parsing here; that is in `routing` and ravenctl.
- **Errors**: parse failures should be logged and either skipped or surfaced so the stream can recover; avoid panics in hot paths.
- **Reconnect**: typical pattern is reconnect with backoff (see `utils::retry` or similar) and re-subscribe to the same streams.
- If source-level changes add/remove a port-bearing collector, update `src/bin/persist/dependency_check.rs` so `raven_init` preflight checks stay synchronized.

See `src/source/binance/AGENT.md` for Binance-specific layout and `src/source/deribit/AGENT.md` for Deribit details.
