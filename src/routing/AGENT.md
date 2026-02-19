# src/routing/ – Venue and symbol resolution

Resolves **which venues** to use and **instrument → venue symbol** mapping. Used by ravenctl when starting collections with `--coin ETH --quote USDC` and by config (venue_include/exclude, symbol_map).

## Modules

| Module | Purpose |
|--------|--------|
| `venue_selector` | Apply `routing.venue_include` / `venue_exclude` to decide which venues are active. |
| `symbol_resolver` | Map canonical instrument (e.g. ETH/USDC) to venue symbol per venue; use `routing.symbol_map` for overrides (e.g. 1000PEPEUSDT for futures). |

## Conventions

- **Input**: instrument (base/quote) and optional venue filter. **Output**: list of (venue, venue_symbol) to use for StartCollection or subscription.
- Config: `RoutingConfig` in `config.rs`; symbol_map is keyed by instrument string (e.g. "PEPE/USDT") and maps venue id → venue symbol.
- Keep resolution pure (no I/O); used by ravenctl and possibly by services that need to resolve instrument to venue symbol.
- If routing changes introduce new venue-backed services/ports, keep `src/bin/persist/dependency_check.rs` updated so `raven_init` validates the correct runtime dependencies.
