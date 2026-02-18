# src/domain/ – Core domain types

Shared vocabulary for instruments, assets, and venues. Used by config, routing, and services.

## Modules

| Module | Types | Purpose |
|--------|--------|--------|
| `asset` | Asset-related types | Base/quote or asset identifiers. |
| `instrument` | `Instrument` | Canonical instrument (e.g. base/quote pair). |
| `venue` | `VenueId` | Venue identifier (e.g. BINANCE_SPOT, BINANCE_FUTURES, BINANCE_OPTIONS, DERIBIT). |

## Venues

| VenueId | Exchange | Used by |
|---------|----------|---------|
| `BINANCE_SPOT` | Binance Spot | `binance_spot` |
| `BINANCE_FUTURES` | Binance Futures | `binance_futures`, `binance_futures_klines`, `binance_futures_funding`, `binance_futures_liquidations`, `binance_futures_oi` |
| `BINANCE_OPTIONS` | Binance European Options | `binance_options` |
| `DERIBIT` | Deribit | `deribit_option`, `deribit_trades`, `deribit_index` |

## Conventions

- **Instrument** = canonical pair (ETH/USDC). **Venue symbol** = exchange-specific (ETHUSDC, 1000PEPEUSDT, BTC-29MAR24-50000-C). Conversion is in `crate::routing::symbol_resolver`, not here.
- Keep domain types small and serializable where needed; no I/O or external calls.
- Venue and symbol enums/constants should match what proto and config use (string venue ids).
