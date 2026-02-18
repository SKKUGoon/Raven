# src/bin/raw/ – Exchange collectors

Binaries that connect to exchange WebSockets and expose market data via gRPC. They do **not** start exchange subscriptions until `Control.StartCollection` is called (except `binance_futures_klines`).

## Binaries

| Binary | Exchange | Data | Notes |
|--------|----------|------|--------|
| `binance_spot` | Binance Spot | TRADE, ORDERBOOK | Port from `server.port_binance_spot`. |
| `binance_futures` | Binance Futures | TRADE, ORDERBOOK | Port from `server.port_binance_futures`. |
| `binance_futures_klines` | Binance Futures | CANDLE (klines) | Auto-starts streams for `binance_klines.symbols`; port `server.port_binance_futures_klines`. |
| `binance_futures_liquidations` | Binance Futures | LIQUIDATION | `{symbol}@forceOrder` WS stream. Port `server.port_binance_futures_liquidations`. |
| `binance_futures_funding` | Binance Futures | FUNDING | REST poller `/fapi/v1/premiumIndex`. Port `server.port_binance_futures_funding`. |
| `binance_futures_oi` | Binance Futures | OPEN_INTEREST | REST poller `/fapi/v1/openInterest`. Port `server.port_binance_futures_oi`. |
| `binance_options` | Binance Options | TICKER | REST poller `/eapi/v1/ticker` → OptionsTicker (venue=BINANCE_OPTIONS). Port `server.port_binance_options`. |
| `deribit_option` | Deribit | TICKER | BTC options ticker (OI, IV, mark, bid/ask). Port `server.port_deribit_ticker`. |
| `deribit_trades` | Deribit | TRADE | BTC options trades (every execution). Port `server.port_deribit_trades`. |
| `deribit_index` | Deribit | PRICE_INDEX | BTC underlying price index (btc_usd). Port `server.port_deribit_index`. |

## Implementation notes

- Entrypoints: `binance_spot.rs`, `binance_futures.rs`, `binance_futures_klines.rs`.
- Actual WS and parsing live in `crate::source::binance` (spot/futures workers, parsing modules). These bins wire config → `StreamManager` / worker and run the gRPC server.
- **Venue symbol**: requests use the exchange symbol (e.g. `ETHUSDC`, `1000PEPEUSDT`). Routing/symbol_map (used by ravenctl) maps instrument → venue symbol per venue.
- Klines: symbols and intervals come from config `binance_klines`; see `src/source/binance/futures/klines/`.

When adding a new exchange collector: add a bin here, implement or reuse a worker in `crate::source`, and register the service/port in config and in ravenctl’s service registry.
