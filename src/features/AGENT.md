# src/features/ – Bar / feature computation

Logic for building **bar** (CANDLE) streams from TRADE (and optionally ORDERBOOK) streams. Used by the binaries in `src/bin/statistics/`.

Time-based candles are sourced from **Binance Klines** (1m) via `binance_futures_klines` and stored in `mart.fact__kline`. TimescaleDB continuous aggregates handle upsampling (5m, 15m, 1h, etc.).

## Modules

| Module | Bar type | Input | Notes |
|--------|----------|--------|--------|
| `tibs` | Tick imbalance bars | TRADE | Config: `tibs.initial_size`, `initial_p_buy`, alpha, size bounds. |
| `trbs` | Tick run bars | TRADE | Run-based bar sizing. |
| `vibs` | Volume imbalance bars | TRADE | Volume-based bar sizing. |
| `vpin` | VPIN | TRADE | Volume-synchronized probability of informed trading. |

## Documentation

See `docs/` subdirectory for detailed explanations (English + Korean) of each derived data type, including logic and hyperparameters.

## Conventions

- Each feature module typically: consumes a stream of trades (and maybe orderbook), maintains bar state, emits `proto::Candle` (or equivalent) when a bar is complete.
- **Config**: TIBS (and possibly others) use `config::TibsConfig`; bar parameters come from config or from `ServiceSpec` (small/large) in the statistics bin.
- **Metrics**: bar emission and latency may be instrumented via `crate::telemetry::bars` or similar.
- **Idempotence**: bar logic should be deterministic for the same trade stream; no external I/O inside the core bar computation.
- If a new feature service introduces or changes a configured port, update `src/bin/persist/dependency_check.rs` so `raven_init` preflight checks remain accurate.

When adding a new bar type: add a module here, keep the same "stream in → bars out" pattern, and wire it in `src/bin/statistics/` with the right upstream subscription and config.
