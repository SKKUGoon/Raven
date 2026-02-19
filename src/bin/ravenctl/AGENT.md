# src/bin/ravenctl/ – Control plane CLI

`ravenctl` is the main operator interface: it starts/stops service **processes** and starts/stops **collections** (which enable actual exchange subscriptions and data flow).

## Entrypoint and layout

- `main.rs` – CLI parsing, dispatch to subcommands.
- `cli.rs` – Subcommand definitions and args.
- `start_stop.rs` – Start/stop collections and pipeline (coin/quote or venue-symbol input, plus venue selection).
- `plan.rs` – Dry-run plan (what `start_collection` calls would be made).
- `cluster.rs` / `ops.rs` – Service process lifecycle: start processes, shutdown, status.
- `shutdown.rs` – Shutdown logic.
- `util.rs` – Shared helpers (e.g. config loading for ravenctl).

## Key behaviors

- **Start order**: when starting a pipeline, ravenctl starts **downstream first** (persistence, then feature makers), then **collectors last** (so exchange WS subscriptions happen after subscribers are ready).
- **Init gate**: `ravenctl start` runs one-shot `raven_init` first; startup now fails early if `raven_init` reports missing dependencies (e.g., Timescale unreachability or configured port conflicts).
- **Instrument vs venue symbol**: `--coin ETH --quote USDC` uses canonical instrument; ravenctl uses `crate::routing` to resolve venue symbols. `--coin ETHUSDC --venue BINANCE_SPOT` uses venue symbol directly.
- **Service registry**: which processes exist and their ports come from config (and possibly `utils::service_registry`). PID/log files go under `~/.raven/log/`.
- **Graph**: `ravenctl graph` (and `--format dot`) uses `crate::pipeline` to render topology.

## Conventions

- ravenctl talks to services via gRPC Control (and possibly MarketData for introspection). Use the same `proto` types; do not duplicate control logic in other bins.
- Config: ravenctl loads config (e.g. via `RAVEN_CONFIG_FILE` or `ravenctl setup`); it passes config path / env to child processes when starting them.
- If a port-bearing service is added/removed/renumbered, keep `utils::service_registry` and `src/bin/persist/dependency_check.rs` aligned so start-time checks remain correct.
- For argument-heavy command input plumbing (e.g. interactive input resolution), prefer builder-style resolvers over long positional parameter lists.

When changing start/stop behavior or adding subcommands, keep the downstream-first order and document any new flags in README.
