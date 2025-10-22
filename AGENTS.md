# Repository Guidelines

## Project Structure & Module Organization
The Rust crate lives in `src/`, split by responsibility: `server/` for request routing, `citadel/` for storage, `data_handlers/` for market feeds, `monitoring/` for observability helpers, and `subscription_manager/` for client state. Shared types belong in `src/lib.rs` or module `mod.rs` files. Generated gRPC contracts sit in `proto/`; rebuild bindings with `cargo build` after edits. Configuration defaults are layered in `config/`, with release bundles under `publish/`. Automated tests live in `tests/`, with unit suites organized under `tests/unit/` and integration flows grouped in `tests/integration/`. The `python_client/` folder supplies a smoke-test client aligned with the protobuf schema.

## Build, Test, and Development Commands
Use `cargo build` for quick local compiles and `make build` for the release binary. Run `make fmt` and `make lint` before commits to enforce `rustfmt` and `clippy -D warnings`. Execute `make test`, or the narrower `make test-unit` and `make test-integration`, to cover all suites. Start the service locally with `ENVIRONMENT=development cargo run --bin raven` or rely on `make run`, adjusting the `ENVIRONMENT` variable for other tiers.

## Coding Style & Naming Conventions
The codebase follows standard Rust formatting (4-space indent, trailing commas) enforced by `rustfmt`. Modules remain snake_case, types use CamelCase, and configuration keys stay lower_snake_case. Prefer embedding shared types in `src/lib.rs` or module `mod.rs` files. Instrument behavior with `tracing` macros and the utilities under `monitoring/` instead of ad-hoc logging.

## Testing Guidelines
Place unit tests in `tests/unit/*.rs`, grouping related scenarios by feature (e.g., `tests/unit/subscription.rs`). Each test function follows the `test_*` naming pattern. House integration flows in `tests/integration/*.rs`; add new files per domain (e.g., `tests/integration/fanout.rs`) as coverage grows. Rely on mocks or the dummy providers under `data_handlers/private_data` to keep suites deterministic and offline. Always run `make test`—or at minimum `make test-unit` and `make test-integration`—before opening a PR.

## Commit & Pull Request Guidelines
Adopt the `<verb>: <brief summary>` commit style already in history (e.g., `fix: handle reconnect backoff`). Pull requests should state intent, reference issues or ADRs when applicable, and note proto or config updates. Include test evidence (command output or logs) and attach screenshots for dashboard-facing changes. Confirm background services started for manual testing are shut down before requesting review.
