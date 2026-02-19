# src/ – Library and entrypoints

This directory is the main Rust crate: **library** (`lib.rs`) plus **binary entrypoints** under `bin/`. The library is used by all binaries.

## Top-level modules (`lib.rs`)

| Module | Role |
|--------|------|
| `proto` | Generated gRPC types (from `proto/*.proto`). |
| `config` | `Settings` and config loading (TOML + env). |
| `domain` | Core types: instrument, asset, venue. |
| `source` | Exchange data sources (Binance spot/futures/klines/options, Deribit options). |
| `features` | Bar/feature logic: tibs, trbs, vibs, vpin. |
| `service` | gRPC server wiring: Control + MarketData, `StreamManager`, `ServiceSpec`. |
| `pipeline` | Pipeline spec and graph rendering (for ravenctl). |
| `routing` | Venue selection and symbol resolution (instrument → venue symbol). |
| `db` | InfluxDB and TimescaleDB writers. |
| `telemetry` | Prometheus metrics for sources and bars. |
| `utils` | Shared helpers: retry, gRPC clients, process, tree, etc. |

## Binaries

All binaries live under `src/bin/`; see `src/bin/AGENT.md`. They depend on this library and `config`.

## Conventions

- **No business logic in `lib.rs`** — only `pub mod` and re-exports if needed.
- **Config**: use `crate::config::Settings` (or injected config); see `config.rs` for structs and env override rules.
- **Errors**: prefer `thiserror`; propagate with `?`; log at boundaries with `tracing`.
- **Async**: Tokio; avoid blocking in async paths.
- **Port-bearing service changes**: when adding/removing/changing a configured service port, also update `src/bin/persist/dependency_check.rs` so `raven_init` preflight checks remain accurate.

Adding a new binary: add `[[bin]]` in `Cargo.toml` and implement under `src/bin/`; reuse `service`, `source`, `features`, or `db` as appropriate.
