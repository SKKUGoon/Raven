# src/source/deribit/ – Deribit BTC options source

Three independent microservices for **Deribit** BTC options trading intelligence, each subscribing to a single Deribit WebSocket channel via JSON-RPC 2.0.

## Layout

| Path | Role |
|------|------|
| `client.rs` | Generic WebSocket client: connect, `public/subscribe` to caller-provided channels, read loop, dispatch notifications via callback. |
| `service.rs` | `DeribitService`: generic single-channel service implementing Control + MarketData. Three constructors: `new_ticker_service`, `new_trades_service`, `new_index_service`. |
| `parsing/` | Parse subscription notifications: ticker → `OptionsTicker`, trades → `Trade`, `deribit_price_index` → `PriceIndex`. |

## Microservices

| Binary | Channel | DataType | Port (default) | Description |
|--------|---------|----------|----------------|-------------|
| `deribit_option` | `ticker.BTC-OPTION.100ms` | TICKER | 50008 | Options ticker: OI, IV, mark, best bid/ask. |
| `deribit_trades` | `trades.BTC-OPTION.100ms` | TRADE | 50009 | Options trades: every execution. |
| `deribit_index` | `deribit_price_index.btc_usd` | PRICE_INDEX | 50010 | Underlying BTC index price. |

Each binary runs its own WebSocket connection and gRPC server. They can be deployed, scaled, and restarted independently.

## Data flow

- Each service opens one WebSocket to Deribit, subscribes to its single channel.
- Control `StartCollection(symbol, venue=DERIBIT, data_type)` registers the stream key; no extra Deribit subscription needed (we receive all BTC-OPTION data and filter in `Subscribe`).
- MarketData `Subscribe` returns a stream filtered by symbol. Use `*` or empty for wildcard.

## Proto types

- **OptionsTicker** (TICKER): symbol, timestamp, open_interest, mark_iv, best_bid/ask_price, mark_price, underlying_price, last_price, bid_iv, ask_iv.
- **Trade** (TRADE): symbol, timestamp, price, quantity, side, trade_id.
- **PriceIndex** (PRICE_INDEX): index_name, price, timestamp.

## Config

- `config::DeribitConfig`: `ws_url` (default production), `rest_url`, `channel_capacity`.
- Server ports: `server.port_deribit_ticker`, `server.port_deribit_trades`, `server.port_deribit_index`.

## Conventions

- Venue id: `DERIBIT`. Symbol = Deribit instrument name (e.g. `BTC-29MAR24-50000-C`) or `BTC_USD` for price index.
- Reconnect with backoff on disconnect; same channel resubscribed after connect.
- Parsing: best-effort; log and skip on parse failure.
- If Deribit service ports change, also update `src/bin/persist/dependency_check.rs` so `raven_init` preflight checks remain correct.
