use crate::{
    common::check_port_availability,
    common::config::{
        ConfigLoader, ConfigUtils, DatabaseConfig, MonitoringConfig, RuntimeConfig, ServerConfig,
    },
    common::db::{
        CircuitBreaker, CircuitBreakerConfig, CircuitBreakerRegistry, DeadLetterQueue,
        DeadLetterQueueConfig, EnhancedInfluxClient, InfluxClient, InfluxConfig,
        InfluxWriteRetryHandler,
    },
    common::error::RavenResult,
    raven_bail,
    server::data_engine::storage::HighFrequencyStorage,
    server::data_engine::{DataEngine, DataEngineConfig},
    server::grpc::client_service::manager::ClientManagerConfig,
    server::grpc::client_service::{ClientManager, MarketDataServer},
    server::grpc::controller_service::{CollectorManager, ControlServiceImpl},
    server::prometheus::{HealthService, MetricsService, ObservabilityService, TracingService},
    server::stream_router::StreamRouter,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::server::app::args::CliArgs;

/// Validate system dependencies and requirements
pub async fn validate_dependencies(
    server: &ServerConfig,
    monitoring: &MonitoringConfig,
) -> RavenResult<()> {
    info!("Validating system dependencies...");

    // Check port availability
    check_port_availability(&server.host, server.port, "Server").await?;
    check_port_availability(&server.host, monitoring.metrics_port, "Metrics").await?;
    check_port_availability(&server.host, monitoring.health_check_port, "Health check").await?;

    info!("All system dependencies validated successfully");
    Ok(())
}

/// Initialize logging with default configuration
pub fn initialize_logging(_args: &CliArgs) -> RavenResult<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

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
            warn!("Try running with --validate to check configuration");
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
    if warnings.is_empty() {
        info!("Configuration validation passed");
    } else {
        warn!(warnings = ?warnings, "Configuration validation passed with warnings");
    }

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
        error!(error = %e, "Failed to load dead letter queue from disk");
    }

    // Start dead letter queue processing
    if let Err(e) = dead_letter_queue.start_processing().await {
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
        raven_bail!(e);
    }

    // Start disconnection event processing
    if let Err(e) = client_manager.start_disconnection_processing().await {
        raven_bail!(e);
    }

    info!("⚮ Client manager initialized with graceful disconnection handling");
    Ok(client_manager)
}

/// Initialize monitoring and observability services
pub async fn initialize_monitoring_services(
    monitoring: &MonitoringConfig,
    influx_client: Arc<InfluxClient>,
    stream_router: Arc<StreamRouter>,
    hf_storage: Arc<HighFrequencyStorage>,
) -> RavenResult<(ObservabilityService, Vec<tokio::task::JoinHandle<()>>)> {
    info!("Initializing monitoring and observability services...");

    // Initialize tracing service
    let tracing_service = Arc::new(TracingService::new(monitoring.clone()));
    if let Err(e) = tracing_service.initialize().await {
        error!(error = %e, "Failed to initialize distributed tracing");
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
        Arc::clone(&stream_router),
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

/// Initialize data layer components
pub fn initialize_data_layer(
    enhanced_influx_client: Arc<EnhancedInfluxClient>,
) -> (
    Arc<StreamRouter>,
    Arc<HighFrequencyStorage>,
    Arc<DataEngine>,
) {
    info!("Initializing data layer...");
    let stream_router = Arc::new(StreamRouter::new());
    let hf_storage = Arc::new(HighFrequencyStorage::new());

    let data_engine = Arc::new(DataEngine::new(
        DataEngineConfig::default(),
        Arc::clone(&enhanced_influx_client),
        Arc::clone(&stream_router),
    ));

    info!("[Server] On guard");
    info!("[DataEngine] Ready for dynamic collection");

    (stream_router, hf_storage, data_engine)
}

/// Initialize service layer components
pub fn initialize_services(
    hf_storage: Arc<HighFrequencyStorage>,
    data_engine: Arc<DataEngine>,
    stream_router: Arc<StreamRouter>,
    influx_client: Arc<InfluxClient>,
    client_manager: Arc<ClientManager>,
) -> (Arc<CollectorManager>, ControlServiceImpl, MarketDataServer) {
    info!("Initializing gRPC services...");
    let collector_manager = Arc::new(CollectorManager::new(
        Arc::clone(&hf_storage),
        Arc::clone(&data_engine),
        Arc::clone(&stream_router),
    ));

    let control_service = ControlServiceImpl::new(Arc::clone(&collector_manager));

    let market_data_server = MarketDataServer::new(
        Arc::clone(&stream_router),
        Arc::clone(&influx_client),
        Arc::clone(&hf_storage),
        Arc::clone(&client_manager),
    );

    (collector_manager, control_service, market_data_server)
}
