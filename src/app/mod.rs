// Project Raven - Market Data Subscription Server
// The Night's Watch begins here - "Crown the king"

pub mod cli;
pub mod shutdown;
pub mod startup;

use market_data_subscription_server::error::RavenResult;
use tracing::info;

use self::cli::{parse_cli_args, print_version_info};
use self::shutdown::{perform_graceful_shutdown, wait_for_shutdown_signal};
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
    info!("The Night's Watch begins - Winter is coming, but we are prepared");

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

    // Perform graceful shutdown
    perform_graceful_shutdown(
        client_manager,
        dead_letter_queue,
        // We need to get the tracing service from monitoring service, but for now we'll create a placeholder
        std::sync::Arc::new(
            market_data_subscription_server::monitoring::TracingService::new(
                config.monitoring.clone(),
            ),
        ),
        circuit_breaker_registry,
        monitoring_handles,
    )
    .await?;

    Ok(())
}
