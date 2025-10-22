// Project Raven - Market Data Subscription Server
// The Crow's Watch begins here - "Crown the king"

pub mod cli;
pub mod shutdown;
pub mod startup;

use crate::citadel::app::{spawn_orderbook_ingestor, spawn_trade_ingestor};
use crate::citadel::streaming::{SnapshotConfig, SnapshotService};
use crate::citadel::{Citadel, CitadelConfig};
use crate::data_handlers::HighFrequencyHandler;
use crate::error::RavenResult;
use crate::exchanges::binance::app::futures::orderbook::initialize_binance_futures_orderbook;
use crate::exchanges::binance::app::futures::trade::initialize_binance_futures_trade;
use crate::subscription_manager::SubscriptionManager;
use crate::types::HighFrequencyStorage;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info};

use self::cli::{parse_cli_args, print_version_info};
use self::shutdown::{perform_graceful_shutdown, wait_for_shutdown_signal, DataCollectors};
use self::startup::{
    build_config_loader, initialize_circuit_breakers, initialize_client_manager,
    initialize_config_manager, initialize_dead_letter_queue, initialize_influx_client,
    initialize_logging, initialize_monitoring_services, load_and_validate_config,
    validate_dependencies,
};

/// Main application entry point and coordination
pub async fn run() -> RavenResult<()> {
    let args = parse_cli_args();

    print_version_info();
    initialize_logging(&args)?;

    let loader = build_config_loader(&args);
    let config = load_and_validate_config(&loader, &args)?;
    validate_dependencies(&config.server, &config.monitoring).await?;
    let _config_manager = initialize_config_manager(loader.clone()).await?;

    info!("Ready: [Kingsguards]");
    let dead_letter_queue = initialize_dead_letter_queue().await?;
    let circuit_breaker_registry = initialize_circuit_breakers().await?;

    let (influx_client, _enhanced_influx_client) =
        initialize_influx_client(&config.database, dead_letter_queue.clone()).await?;
    let client_manager = initialize_client_manager(&config.server).await?;

    info!("Ready: [Citadel]");
    let subscription_manager = Arc::new(SubscriptionManager::new());
    let hf_storage = Arc::new(HighFrequencyStorage::new());
    let high_freq_handler = Arc::new(HighFrequencyHandler::with_storage(Arc::clone(&hf_storage)));
    let citadel = Arc::new(Citadel::new(
        CitadelConfig::default(),
        Arc::clone(&influx_client),
        Arc::clone(&subscription_manager),
    ));

    // Initialize and start the snapshot service for live data distribution
    let snapshot_config = SnapshotConfig {
        snapshot_interval: std::time::Duration::from_millis(100), // 100ms snapshots
        max_batch_size: 1000,
        write_timeout: std::time::Duration::from_millis(100),
        broadcast_enabled: true, // Enable broadcasting to gRPC clients
        persistence_enabled: true,
    };

    let snapshot_service = Arc::new(SnapshotService::new(
        snapshot_config,
        Arc::clone(&hf_storage),
        Arc::clone(&influx_client),
        Arc::clone(&subscription_manager),
    ));

    info!("[Citadel] Starting snapshot service for live data distribution");
    if let Err(e) = snapshot_service.start().await {
        error!("!!! Failed to start snapshot service: {}", e);
    }

    info!("[Kingsguards] On guard");
    let (_monitoring_service, monitoring_handles) = initialize_monitoring_services(
        &config.monitoring,
        Arc::clone(&influx_client),
        Arc::clone(&subscription_manager),
        Arc::clone(&hf_storage),
    )
    .await?;

    info!("[Citadel] Start research");
    let sym = "btcusdt".to_string();
    let (binance_future_clob_collector, binance_clob_receiver) =
        initialize_binance_futures_orderbook(sym.clone()).await?;
    let (binance_future_trade_collector, binance_trade_receiver) =
        initialize_binance_futures_trade(sym).await?;

    // Start market data ingestion pipelines
    let collector_tasks: Vec<JoinHandle<()>> = vec![
        spawn_orderbook_ingestor(
            binance_clob_receiver,
            Arc::clone(&high_freq_handler),
            Arc::clone(&citadel),
        ),
        spawn_trade_ingestor(
            binance_trade_receiver,
            Arc::clone(&high_freq_handler),
            Arc::clone(&citadel),
        ),
    ];

    // Initialize gRPC server with circuit breaker protection
    info!("[Raven] 'Send out the ravens'");
    info!("Server {}:{}", config.server.host, config.server.port);
    info!("Metric check :{}", config.monitoring.metrics_port);
    info!("Health check :{}", config.monitoring.health_check_port);

    // Start the gRPC server
    let server = crate::server::MarketDataServer::new(
        Arc::clone(&subscription_manager),
        Arc::clone(&influx_client),
        Arc::clone(&hf_storage),
        config.server.max_connections,
    );

    // Spawn the gRPC server in a separate task
    let server_handle = {
        let host = "0.0.0.0".to_string(); // Bind to all interfaces
        let port = config.server.port;
        tokio::spawn(async move {
            info!("▶ Starting gRPC server on {}:{}", host, port);
            if let Err(e) = server.start(&host, port).await {
                tracing::error!("✗ gRPC server failed: {}", e);
            } else {
                info!("✓ gRPC server started successfully");
            }
        })
    };

    // Wait for shutdown signals
    wait_for_shutdown_signal().await?;

    // Stop gRPC server
    server_handle.abort();

    // Stop background collector tasks before shutdown
    for task in collector_tasks {
        task.abort();
    }

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
