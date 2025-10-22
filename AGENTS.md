# Repository Guidelines
This guide keeps Raven contributions consistent and maintainable.

## Project Structure & Module Organization
- `src/` is the main crate: `server/` handles requests, `citadel/` owns storage, `data_handlers/` manages market feeds, `monitoring/` wraps observability, and `subscription_manager/` routes client state.
- `proto/` stores gRPC contracts; regenerate bindings with `cargo build` after edits.
- `config/` supplies layered TOML defaults, with environment-specific overrides alongside the main configs; release bundles live in `publish/`.
- `python_client/` provides a smoke-test client—keep generated stubs aligned with `proto/`.
- `docs/` and `scripts/` contain architectural notes and helper automation; consult them before adding new tooling.

## Build, Test, and Development Commands
- `make build` produces the release binary; use `cargo build` for incremental local work.
- `make test`, `make test-unit`, and `make test-integration` map to the corresponding `cargo test` suites; run them all before a PR.
- `make fmt` and `make lint` enforce `rustfmt` and `clippy -D warnings` and must pass in CI.
- `make run` launches the server with your active configuration; `./scripts/run-env.sh` provides the same flow with explicit environment selection.

## Coding Style & Naming Conventions
- Follow `rustfmt` defaults (4-space indent, trailing commas). Modules stay snake_case, types are CamelCase, and configuration keys use lower_snake_case to match existing TOML.
- Centralize shared types in `src/lib.rs` or module-level `mod.rs` files to prevent duplication.
- Instrument new behavior with `tracing` macros and the helpers in `monitoring/` rather than ad-hoc logging.

## Testing Guidelines
- Place unit tests in each module’s `mod tests` block and name functions `test_*` for clarity.
- Cross-cutting scenarios belong in `tests/integration_tests.rs`, invoked through `make test-integration`; create additional files when flows span modules.
- Use mocks or the dummy providers in `data_handlers/private_data` to keep tests deterministic and network-free.

## Commit & Pull Request Guidelines
- Adopt the `<verb>: <brief summary>` commit style seen in history (e.g., `fix: handle reconnect backoff`) and reference issues or ADRs in the body when relevant.
- PRs should state intent, include test evidence (commands or logs), and call out proto or config changes; attach screenshots for dashboard-facing updates.
- Before requesting review, run `make fmt lint test` and ensure background processes started for manual testing are stopped.
