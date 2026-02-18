# src/bin/common/ – Shared binary code

Code shared by multiple binaries under `src/bin/`. Used to avoid duplication of setup, config loading, or server wiring.

## Usage

- **common/mod.rs**: re-exports or helpers used by raw, persist, or statistics binaries (e.g. building a gRPC server with Control + MarketData, or loading config in a standard way).
- Not all binaries must use `common`; only share when two or more bins need the same logic.

## Conventions

- Keep `common` minimal; prefer putting reusable logic in the main library (`crate::service`, `crate::config`, etc.) and use `common` only for bin-specific glue.
- Do not put exchange-specific or feature-specific logic here.
