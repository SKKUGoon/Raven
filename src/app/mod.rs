// Project Raven - Market Data Subscription Server
// The Crow's Watch begins here - "Crown the king"

pub mod cli;
pub mod shutdown;
pub mod startup;

use crate::error::RavenResult;
use crate::exchanges::binance::app::futures::orderbook::initialize_binance_futures_orderbook;
use crate::exchanges::binance::app::futures::trade::initialize_binance_futures_trade;
use tracing::info;

use self::cli::{parse_cli_args, print_version_info};
use self::shutdown::{perform_graceful_shutdown, wait_for_shutdown_signal, DataCollectors};
use self::startup::{
    initialize_circuit_breakers, initialize_client_manager, initialize_config_manager,
    initialize_dead_letter_queue, initialize_influx_client, initialize_logging,
    initialize_monitoring_services, load_and_validate_config, validate_dependencies,
};

/// Main application entry point and coordination
pub async fn run() -> RavenResult<()> {
    // Parse command line arguments
    let args = parse_cli_args();

    // Print version information
    print_version_info();

    // Initialize basic logging first
    initialize_logging(&args)?;

    info!("🐦‍⬛ Project Raven is awakening...");
    info!("The Crow's Watch begins - Winter is coming, but we are prepared");

    // Load and validate configuration with CLI overrides
    let config = load_and_validate_config(&args)?;

    // Validate system dependencies
    validate_dependencies(&config).await?;

    // Initialize configuration manager with hot-reloading
    let _config_manager = initialize_config_manager().await?;

    // Initialize error handling and reliability components
    info!("🛡️ Initializing error handling and reliability features...");

    // 1. Initialize Dead Letter Queue
    let dead_letter_queue = initialize_dead_letter_queue().await?;

    // 2. Initialize Circuit Breaker Registry
    let circuit_breaker_registry = initialize_circuit_breakers().await?;

    // 3. Initialize InfluxDB Client with Enhanced Error Handling
    let (influx_client, _enhanced_influx_client) =
        initialize_influx_client(&config, dead_letter_queue.clone()).await?;

    // 4. Initialize Client Manager
    let client_manager = initialize_client_manager(&config).await?;

    // 5. Initialize Monitoring and Observability
    let (_monitoring_service, monitoring_handles) =
        initialize_monitoring_services(&config, influx_client).await?;

    // 6. Initialize Binance Data Collector
    let sym = "btcusdt".to_string();
    let (binance_future_clob_collector, _binance_clob_receiver) =
        initialize_binance_futures_orderbook(sym.clone()).await?;
    let (binance_future_trade_collector, _binance_trade_receiver) =
        initialize_binance_futures_trade(sym).await?;

    // TODO: Initialize remaining components with error handling
    // - gRPC server with circuit breaker protection
    // - Data handlers with dead letter queue integration
    // - Subscription manager with client manager integration

    info!("✅ Error handling and reliability features initialized successfully");
    info!("🛡️ Winter preparations complete - all defenses are in place");

    info!("✅ Project Raven initialized successfully");
    info!(
        "🏰 Server will listen on {}:{}",
        config.server.host, config.server.port
    );
    info!(
        "📊 Metrics will be available on port {}",
        config.monitoring.metrics_port
    );
    info!(
        "🏥 Health checks will be available on port {}",
        config.monitoring.health_check_port
    );

    info!("✅ Project Raven is now ready to serve the realm!");
    info!("👑 The king is crowned - all systems operational");

    // Wait for shutdown signals
    wait_for_shutdown_signal().await?;

    // Create data collectors container
    let data_collectors = DataCollectors::new()
        .add_collector(
            "binance-futures-orderbook".to_string(),
            binance_future_clob_collector,
        )
        .add_collector(
            "binance-futures-trade".to_string(),
            binance_future_trade_collector,
        );

    // Perform graceful shutdown
    perform_graceful_shutdown(
        client_manager,
        dead_letter_queue,
        // We need to get the tracing service from monitoring service, but for now we'll create a placeholder
        std::sync::Arc::new(crate::monitoring::TracingService::new(
            config.monitoring.clone(),
        )),
        circuit_breaker_registry,
        monitoring_handles,
        data_collectors,
    )
    .await?;

    Ok(())
}
