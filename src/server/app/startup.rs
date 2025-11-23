use crate::{
    common::config::{
        ConfigLoader, ConfigUtils, DatabaseConfig, MonitoringConfig, RuntimeConfig, ServerConfig,
    },
    common::error::RavenResult,
    common::logging::{init_logging, log_config_validation, log_error_with_context, LoggingConfig},
    raven_bail,
    server::client_manager::{ClientManager, ClientManagerConfig},
    server::data_engine::storage::HighFrequencyStorage,
    server::database::{
        circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerRegistry},
        DeadLetterQueue, DeadLetterQueueConfig, EnhancedInfluxClient, InfluxClient, InfluxConfig,
        InfluxWriteRetryHandler,
    },
    server::monitoring::{HealthService, MetricsService, ObservabilityService, TracingService},
    server::subscription_manager::SubscriptionManager,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::server::app::args::CliArgs;

/// Validate system dependencies and requirements
pub async fn validate_dependencies(
    server: &ServerConfig,
    monitoring: &MonitoringConfig,
) -> RavenResult<()> {
    info!("Validating system dependencies...");

    // Check if we can bind to the specified port
    let bind_addr = format!("{}:{}", server.host, server.port);
    match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => {
            drop(listener);
            info!("  * Port {} is available for binding", server.port);
        }
        Err(e) => {
            error!("  * Cannot bind to {}: {}", bind_addr, e);
            crate::raven_bail!(crate::raven_error!(
                configuration,
                format!("  * Port {} is not available: {e}", server.port)
            ));
        }
    }

    // Check if metrics port is available
    let metrics_bind_addr = format!("{}:{}", server.host, monitoring.metrics_port);
    match tokio::net::TcpListener::bind(&metrics_bind_addr).await {
        Ok(listener) => {
            drop(listener);
            info!("  * Metrics port {} is available", monitoring.metrics_port);
        }
        Err(e) => {
            error!(
                "  * Cannot bind to metrics port {}: {e}",
                monitoring.metrics_port,
            );
            crate::raven_bail!(crate::raven_error!(
                configuration,
                format!(
                    "  * Metrics port {} is not available: {e}",
                    monitoring.metrics_port,
                )
            ));
        }
    }

    // Check if health check port is available
    let health_bind_addr = format!("{}:{}", server.host, monitoring.health_check_port);
    match tokio::net::TcpListener::bind(&health_bind_addr).await {
        Ok(listener) => {
            drop(listener);
            info!(
                "  * Health check port {} is available",
                monitoring.health_check_port
            );
        }
        Err(e) => {
            error!(
                "  * Cannot bind to health check port {}: {}",
                monitoring.health_check_port, e
            );
            crate::raven_bail!(crate::raven_error!(
                configuration,
                format!(
                    "  * Health check port {} is not available: {}",
                    monitoring.health_check_port, e
                )
            ));
        }
    }

    info!("All system dependencies validated successfully");
    Ok(())
}

/// Initialize logging with the provided configuration
pub fn initialize_logging(_args: &CliArgs) -> RavenResult<()> {
    let basic_logging = LoggingConfig::default();

    if let Err(e) = init_logging(&basic_logging) {
        eprintln!("Failed to initialize logging: {e}");
        return Err(e);
    }

    Ok(())
}

/// Build a configuration loader based on CLI arguments.
pub fn build_config_loader(args: &CliArgs) -> ConfigLoader {
    let mut loader = ConfigLoader::new();

    if let Some(path) = &args.config_file {
        loader = loader.with_file(PathBuf::from(path));
    }

    loader
}

/// Load and validate configuration with CLI overrides
pub fn load_and_validate_config(
    loader: &ConfigLoader,
    args: &CliArgs,
) -> RavenResult<RuntimeConfig> {
    // Initialize configuration using the selected loader
    let mut config = match RuntimeConfig::load_with_loader(loader) {
        Ok(config) => {
            info!("Configuration loaded successfully");
            config
        }
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            error!("Try running with --validate to check configuration");
            raven_bail!(crate::raven_error!(configuration, e.to_string()));
        }
    };

    // Apply CLI overrides to configuration
    config = crate::server::app::args::apply_cli_overrides(config, args);

    // Handle special CLI modes
    // Print configuration summary
    ConfigUtils::print_config(&config);

    // Check configuration health
    let warnings = ConfigUtils::check_configuration_health(&config);
    log_config_validation("main", warnings.is_empty(), &warnings);

    Ok(config)
}

/// Initialize dead letter queue
pub async fn initialize_dead_letter_queue() -> RavenResult<Arc<DeadLetterQueue>> {
    let dlq_config = DeadLetterQueueConfig {
        max_size: 10000,
        max_age_ms: 24 * 60 * 60 * 1000, // 24 hours
        processing_interval_ms: 30000,   // 30 seconds
        default_max_retries: 3,
        persist_to_disk: true,
        persistence_file: Some("dead_letter_queue.json".to_string()),
    };

    let dead_letter_queue = Arc::new(DeadLetterQueue::new(dlq_config));

    // Load persisted dead letter entries
    if let Err(e) = dead_letter_queue.load_from_disk().await {
        log_error_with_context(&e, "Failed to load dead letter queue from disk");
    }

    // Start dead letter queue processing
    if let Err(e) = dead_letter_queue.start_processing().await {
        log_error_with_context(&e, "Failed to start dead letter queue processing");
        raven_bail!(e);
    }

    info!("Dead letter queue initialized and processing started");
    Ok(dead_letter_queue)
}

/// Initialize circuit breaker registry
pub async fn initialize_circuit_breakers() -> RavenResult<Arc<CircuitBreakerRegistry>> {
    let circuit_breaker_registry = Arc::new(CircuitBreakerRegistry::new());

    // Create circuit breakers for different components
    let database_cb_config = CircuitBreakerConfig {
        failure_threshold: 5,
        success_threshold: 3,
        recovery_timeout: Duration::from_secs(30),
        half_open_max_requests: 3,
        failure_window: Duration::from_secs(60),
        minimum_requests: 10,
    };

    let database_circuit_breaker = Arc::new(CircuitBreaker::new("database", database_cb_config));
    circuit_breaker_registry
        .register(Arc::clone(&database_circuit_breaker))
        .await;

    let grpc_cb_config = CircuitBreakerConfig {
        failure_threshold: 10,
        success_threshold: 5,
        recovery_timeout: Duration::from_secs(15),
        half_open_max_requests: 5,
        failure_window: Duration::from_secs(30),
        minimum_requests: 20,
    };

    let grpc_circuit_breaker = Arc::new(CircuitBreaker::new("grpc", grpc_cb_config));
    circuit_breaker_registry
        .register(Arc::clone(&grpc_circuit_breaker))
        .await;

    info!("Circuit breakers initialized for database and gRPC components");
    Ok(circuit_breaker_registry)
}

/// Initialize InfluxDB client with enhanced error handling
pub async fn initialize_influx_client(
    database: &DatabaseConfig,
    dead_letter_queue: Arc<DeadLetterQueue>,
) -> RavenResult<(Arc<InfluxClient>, Arc<EnhancedInfluxClient>)> {
    let influx_config = InfluxConfig {
        url: database.influx_url.clone(),
        bucket: database.bucket.clone(),
        org: database.org.clone(),
        token: database.token.clone(),
        pool_size: database.connection_pool_size,
        timeout: Duration::from_secs(database.connection_timeout_seconds),
        retry_attempts: database.retry_attempts,
        retry_delay: Duration::from_millis(database.retry_delay_ms),
        batch_size: 1000, // Default batch size since it's not in config
        flush_interval: Duration::from_millis(5), // Default flush interval since it's not in config
    };

    let influx_client = Arc::new(InfluxClient::new(influx_config));
    let enhanced_influx_client = Arc::new(EnhancedInfluxClient::new(
        Arc::clone(&influx_client),
        Arc::clone(&dead_letter_queue),
    ));

    // Register InfluxDB retry handler
    let influx_retry_handler = Box::new(InfluxWriteRetryHandler::new(Arc::clone(&influx_client)));
    dead_letter_queue
        .register_retry_handler(influx_retry_handler)
        .await;

    // Connect to InfluxDB
    if let Err(e) = influx_client.connect().await {
        log_error_with_context(
            &crate::raven_error!(database_connection, e.to_string()),
            "Failed to connect to InfluxDB",
        );
        raven_bail!(crate::raven_error!(database_connection, e.to_string()));
    }

    info!("InfluxDB client initialized with enhanced error handling");
    Ok((influx_client, enhanced_influx_client))
}

/// Initialize client manager
pub async fn initialize_client_manager(server: &ServerConfig) -> RavenResult<Arc<ClientManager>> {
    let client_manager_config = ClientManagerConfig {
        max_clients: server.max_connections,
        heartbeat_timeout: Duration::from_secs(60),
        health_check_interval: Duration::from_secs(30),
        disconnection_grace_period: Duration::from_secs(10),
        max_idle_time: Duration::from_secs(300),
        track_connection_quality: true,
    };

    let client_manager = Arc::new(ClientManager::new(client_manager_config));

    // Start client health monitoring
    if let Err(e) = client_manager.start_health_monitoring().await {
        log_error_with_context(&e, "Failed to start client health monitoring");
        raven_bail!(e);
    }

    // Start disconnection event processing
    if let Err(e) = client_manager.start_disconnection_processing().await {
        log_error_with_context(&e, "Failed to start disconnection event processing");
        raven_bail!(e);
    }

    info!("⚮ Client manager initialized with graceful disconnection handling");
    Ok(client_manager)
}

/// Initialize monitoring and observability services
pub async fn initialize_monitoring_services(
    monitoring: &MonitoringConfig,
    influx_client: Arc<InfluxClient>,
    subscription_manager: Arc<SubscriptionManager>,
    hf_storage: Arc<HighFrequencyStorage>,
) -> RavenResult<(ObservabilityService, Vec<tokio::task::JoinHandle<()>>)> {
    info!("Initializing monitoring and observability services...");

    // Initialize tracing service
    let tracing_service = Arc::new(TracingService::new(monitoring.clone()));
    if let Err(e) = tracing_service.initialize().await {
        log_error_with_context(
            &crate::raven_error!(internal, e.to_string()),
            "Failed to initialize distributed tracing",
        );
        warn!("Continuing without distributed tracing");
    } else {
        info!("Distributed tracing initialized successfully");
    }

    // Initialize metrics service
    let metrics_service = Arc::new(
        MetricsService::new(monitoring.clone())
            .map_err(|e| crate::raven_error!(internal, e.to_string()))?,
    );

    let health_service = Arc::new(HealthService::new(
        monitoring.clone(),
        Arc::clone(&influx_client),
        Arc::clone(&subscription_manager),
        Arc::clone(&hf_storage),
    ));

    // Create monitoring service
    let monitoring_service = ObservabilityService::new(
        Arc::clone(&health_service),
        Arc::clone(&metrics_service),
        Arc::clone(&tracing_service),
    );

    // Start monitoring services
    let monitoring_handles = monitoring_service
        .start()
        .await
        .map_err(|e| crate::raven_error!(internal, e.to_string()))?;

    info!("Monitoring services started:");
    info!("  Health checks on port {}", monitoring.health_check_port);
    info!("  Metrics on port {}", monitoring.metrics_port);
    info!(
        "  Distributed tracing enabled: {}",
        monitoring.tracing_enabled
    );

    Ok((monitoring_service, monitoring_handles))
}
