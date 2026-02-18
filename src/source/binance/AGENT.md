# src/source/binance/ – Binance implementations

Binance Spot, Futures, and Futures Klines WebSocket clients and parsing.

## Layout

| Path | Role |
|------|------|
| `client.rs` | Shared HTTP/WS client or connection setup for Binance. |
| `spot/` | Spot worker and parsing (trade, orderbook). |
| `futures/` | Futures worker and parsing (trade, orderbook). |
| `futures/klines/` | Klines service: symbol config, gRPC exposure, candle parsing. |
| `futures/liquidation.rs` | All-market liquidation WebSocket stream: `!forceOrder` → `Liquidation` (all symbols, 1s snapshots). |
| `futures/funding.rs` | Mark price / funding rate WebSocket stream: `!markPrice@arr@1s` → `FundingRate` (all symbols, 1s updates). |
| `futures/open_interest.rs` | Open interest REST poller: `/fapi/v1/openInterest` → `OpenInterest`. |
| `options/` | Binance European Options REST poller: `/eapi/v1/ticker` → `OptionsTicker` (venue BINANCE_OPTIONS). |

## Spot and Futures (trade + orderbook)

- **Worker**: `spot/worker.rs`, `futures/worker.rs` – maintain WS connection, dispatch to parsers, produce `Trade` / `OrderBookSnapshot`.
- **Parsing**: `spot/parsing/`, `futures/parsing/` – convert Binance JSON to `proto` types; handle trade, orderbook, and (futures) candle if applicable.
- **Symbol format**: Binance spot uses symbols like `ethusdc` (lowercase in WS); normalize to venue symbol (e.g. `ETHUSDC`) when emitting.

## Klines

- **Service**: `futures/klines/service.rs` – drives kline streams for configured symbols/intervals.
- **Symbols**: `futures/klines/symbols.rs` – resolve symbols from config (`binance_klines`).
- **gRPC**: `futures/klines/grpc.rs` – expose CANDLE stream to Control/MarketData.
- **Parsing**: `futures/parsing/candle.rs` – Binance kline payload → `proto::Candle`.

## Conventions

- Keep parsing pure where possible (in → out); side effects (logging, metrics) in worker or service layer.
- Binance URLs and stream naming are documented in Binance API docs; keep stream names and subscription logic in one place (worker or service).
