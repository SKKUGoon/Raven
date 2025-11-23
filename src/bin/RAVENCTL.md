# Raven Control CLI (ravenctl)

`ravenctl` is the command-line interface for managing the Raven market data service. It functions as a gRPC client that communicates with the running Raven server to orchestrate data collection, manage configuration, and inspect system state.

## Architecture

The CLI operates by establishing a gRPC connection to the Raven server's Control Service (default port: `50052`). It does not directly manipulate the data engine or subscription manager; instead, it issues commands that the server executes.

### Communication Flow

1.  **Client (`ravenctl`)**: Parses command-line arguments and constructs gRPC requests (e.g., `StartCollectionRequest`, `StopCollectionRequest`).
2.  **Transport**: Sends the request over HTTP/2 to the configured server address.
3.  **Server (`raven`)**: The `ControlService` running within the main application receives the request, executes the logic (e.g., starting a WebSocket connection to Binance), and returns a response.
4.  **Response**: The CLI displays the result (success/failure, collection IDs, uptime) to the user.

## Commands

### Data Collection Management

These commands require a running Raven server instance.

*   `use`: Start collecting data for a specific symbol on an exchange.
    *   Arguments: `--exchange <EXCHANGE>` (e.g., `binance_futures`), `--symbol <SYMBOL>` (e.g., `BTCUSDT`)
*   `stop`: Stop an active data collection.
    *   Arguments: `--exchange <EXCHANGE>`, `--symbol <SYMBOL>`
*   `list`: Display all currently active data collections, including their IDs, status, and uptime.

### Configuration & Utilities

These commands run locally and do not require a running server.

*   `validate`: Verify the syntax and logical correctness of a configuration file.
    *   Arguments: `--config <PATH>`
*   `show`: Display the currently loaded configuration.
    *   Arguments: `--format <json|pretty>`
*   `health`: Check the configuration for potential issues or warnings.
*   `template`: Generate a default configuration file to use as a starting point.

## Usage Examples

**Start collecting BTC/USDT from Binance Futures:**

```bash
ravenctl use --exchange binance_futures --symbol BTCUSDT
```

**List all active collections:**

```bash
ravenctl list
```

**Validate a configuration file:**

```bash
ravenctl validate --config config/production.toml
```

**Stop a collection:**

```bash
ravenctl stop --exchange binance_futures --symbol BTCUSDT
```

## Connection Settings

By default, `ravenctl` attempts to connect to `http://127.0.0.1:50052`. You can override this for remote management:

```bash
ravenctl --server http://10.0.0.5:50052 list
```

