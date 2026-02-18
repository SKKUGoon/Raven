# proto/ – gRPC and Protocol Buffers

gRPC service and message definitions for Raven. The Rust code uses **generated** types from these files; do not edit generated code.

## Files

| File | Contents |
|------|----------|
| `market_data.proto` | `MarketData` service (`Subscribe`), `MarketDataRequest`, `MarketDataMessage`, and message types: `Trade`, `OrderBookSnapshot`, `Candle`, `FundingRate`, `Liquidation`, `OpenInterest`, `OptionsTicker`, `PriceIndex`. `DataType` enum: ORDERBOOK, TRADE, CANDLE, FUNDING, TICKER, PRICE_INDEX, LIQUIDATION, OPEN_INTEREST. `MarketDataMessage` uses `venue` + `producer` (legacy `exchange` was removed in v3 and reserved). |
| `control.proto` | Control service (e.g. `StartCollection`, `StopCollection`) and request/response types using symbol, venue, and data_type. |

## Build

- Codegen is done at compile time via `tonic`/`tonic-build` in `build.rs` (see repo root). Generated code lives in `crate::proto` and is included with `tonic::include_proto!("raven")`.
- Package name in proto is `raven`; ensure `build.rs` and `include_proto!` use the same name.

## Conventions

- **Breaking changes**: changing field numbers or removing fields is breaking for clients. Prefer adding optional fields or new RPCs.
- **v3 wire break**: `MarketDataMessage.exchange` is removed and reserved (`6`, `"exchange"`). Do not reintroduce or reuse this field number/name.
- **Venue and symbol**: request messages use `string venue` and `string symbol` (venue symbol); keep in sync with `domain::venue` and routing.
- After editing `.proto`, run `cargo build` to regenerate; do not hand-edit generated Rust.
- If proto/service evolution adds new port-bearing services, document and sync those changes with `src/bin/persist/dependency_check.rs` so `raven_init` preflight checks stay complete.
