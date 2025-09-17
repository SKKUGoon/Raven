# Repository Guidelines

## Project Structure & Module Organization
Core Rust services live in `src/`, split by domain (`app/`, `citadel/`, `server/`, `subscription_manager/`) with shared types in `src/types` and cross-cutting utilities in `src/error` and `src/logging.rs`. Workspace-level binaries stay under `src/bin`. End-to-end tests live in `tests/` (`unit_tests.rs`, `integration_tests.rs`). Operational assets sit in `docker/`, `config/`, and `scripts/`; dashboard visualizations
and notebooks live in `dashboard/` and `notebook/`. Protocol buffers reside in `proto/` and are compiled via `build.rs`.

## Build, Test & Development Commands
- `make build` – release build of the Rust server (`cargo build --release`).
- `make dev` – run format, lint, and test cycle for local validation.
- `make test` or `cargo test --all` – execute the full Rust test suite.
- `make fmt` / `make lint` – enforce `cargo fmt --all` and `cargo clippy` with warnings denied.
- `make bench-latency` and `make bench-high-freq` – run Criterion benchmarks for latency-sensitive modules.
- `make deploy` – compose full stack (server, dashboard, InfluxDB, Prometheus).

## Coding Style & Naming Conventions
Use Rust 2021 defaults with four-space indentation and trailing commas where idiomatic. Keep modules and files `snake_case`, public types `PascalCase`, and constants `SCREAMING_SNAKE_CASE`. Always run `cargo fmt --all` before committing and ensure `cargo clippy --all-targets --all-features -D warnings` passes cleanly. Favor small modules that mirror directories under `src/`; colocate domain-specific tests beside
their module when practical.

## Testing Guidelines
Prefer `cargo test --all` locally and include `cargo test --test integration_tests` when touching boundary modules (`server`, `citadel`, `database`). Add focused async tests using `tokio::test` helpers, and follow the `mod_feature_case` naming pattern for clarity. Update or add Criterion benches (`bench-*` in `Makefile`) when altering `src/types` or performance-critical paths. Document any new fixtures under
`tests/` with descriptive filenames.

## Commit & Pull Request Guidelines
Match existing history by prefixing commits with a short type (`add:`, `fix:`, `chore:`, `refactor:`) followed by an imperative summary ≤72 characters. Reference relevant modules in the body and list notable commands run. Pull requests should describe motivation, implementation notes, testing evidence, and any dashboard or gRPC impacts; attach screenshots or logs when user-facing behavior changes. Link issues and
request reviewers early for cross-cutting changes.

## Protocol Buffers & Configuration
Update `.proto` files under `proto/` alongside Rust server changes and run `cargo build` to regenerate code via `build.rs`. Keep environment templates in `config/` in sync; never commit real secrets—use `*.example` updates instead. For local services, copy `config/development.toml` into your workspace and adjust overrides via environment variables.
