use crate::{
    circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerRegistry},
    client_manager::{ClientManager, ClientManagerConfig},
    config::{Config, ConfigManager, ConfigUtils},
    database::{
        DeadLetterQueue, DeadLetterQueueConfig, EnhancedInfluxClient, InfluxClient, InfluxConfig,
        InfluxWriteRetryHandler,
    },
    error::{RavenError, RavenResult},
    logging::{init_logging, log_config_validation, log_error_with_context, LoggingConfig},
    monitoring::{CrowService, HealthService, MetricsService, TracingService},
    raven_bail,
    subscription_manager::SubscriptionManager,
    types::HighFrequencyStorage,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::app::cli::CliArgs;

/// Validate system dependencies and requirements
pub async fn validate_dependencies(config: &Config) -> RavenResult<()> {
    info!("🔍 Validating system dependencies...");

    // Check if we can bind to the specified port
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => {
            drop(listener);
            info!("✅ Port {} is available for binding", config.server.port);
        }
        Err(e) => {
            error!("❌ Cannot bind to {}: {}", bind_addr, e);
            return Err(RavenError::configuration(format!(
                "Port {} is not available: {}",
                config.server.port, e
            )));
        }
    }

    // Check if metrics port is available
    let metrics_bind_addr = format!("{}:{}", config.server.host, config.monitoring.metrics_port);
    match tokio::net::TcpListener::bind(&metrics_bind_addr).await {
        Ok(listener) => {
            drop(listener);
            info!(
                "✅ Metrics port {} is available",
                config.monitoring.metrics_port
            );
        }
        Err(e) => {
            error!(
                "❌ Cannot bind to metrics port {}: {}",
                config.monitoring.metrics_port, e
            );
            return Err(RavenError::configuration(format!(
                "Metrics port {} is not available: {}",
                config.monitoring.metrics_port, e
            )));
        }
    }

    // Check if health check port is available
    let health_bind_addr = format!(
        "{}:{}",
        config.server.host, config.monitoring.health_check_port
    );
    match tokio::net::TcpListener::bind(&health_bind_addr).await {
        Ok(listener) => {
            drop(listener);
            info!(
                "✅ Health check port {} is available",
                config.monitoring.health_check_port
            );
        }
        Err(e) => {
            error!(
                "❌ Cannot bind to health check port {}: {}",
                config.monitoring.health_check_port, e
            );
            return Err(RavenError::configuration(format!(
                "Health check port {} is not available: {}",
                config.monitoring.health_check_port, e
            )));
        }
    }

    info!("✅ All system dependencies validated successfully");
    Ok(())
}

/// Initialize logging with the provided configuration
pub fn initialize_logging(args: &CliArgs) -> RavenResult<()> {
    let basic_logging = LoggingConfig {
        level: args.log_level.clone().unwrap_or_else(|| "info".to_string()),
        format: "pretty".to_string(),
        ..Default::default()
    };

    if let Err(e) = init_logging(&basic_logging) {
        eprintln!("❌ Failed to initialize logging: {e}");
        return Err(e);
    }

    Ok(())
}

/// Load and validate configuration with CLI overrides
pub fn load_and_validate_config(args: &CliArgs) -> RavenResult<Config> {
    // Initialize configuration with optional custom config file
    let mut config = match Config::load_with_file(args.config_file.as_deref()) {
        Ok(config) => {
            info!("✅ Configuration loaded successfully");
            config
        }
        Err(e) => {
            error!("❌ Failed to load configuration: {}", e);
            error!("💡 Try running with --validate to check configuration");
            raven_bail!(RavenError::configuration(e.to_string()));
        }
    };

    // Apply CLI overrides to configuration
    config = crate::app::cli::apply_cli_overrides(config, args);

    // Handle special CLI modes
    if args.validate_only {
        info!("🔍 Validating configuration only...");
        if let Err(e) = config.validate() {
            error!("❌ Configuration validation failed: {}", e);
            return Err(RavenError::configuration(e.to_string()));
        }
        info!("✅ Configuration validation passed");
        println!("Configuration is valid!");
        std::process::exit(0);
    }

    if args.print_config {
        info!("📋 Printing configuration...");
        ConfigUtils::print_config(&config);
        std::process::exit(0);
    }

    // Print configuration summary
    ConfigUtils::print_config(&config);

    // Check configuration health
    let warnings = ConfigUtils::check_configuration_health(&config);
    log_config_validation("main", warnings.is_empty(), &warnings);

    Ok(config)
}

/// Initialize configuration manager with hot-reloading
pub async fn initialize_config_manager() -> RavenResult<ConfigManager> {
    let config_manager = ConfigManager::new(
        "config/default.toml".to_string(),
        Duration::from_secs(5), // Check every 5 seconds
    )
    .map_err(|e| RavenError::configuration(e.to_string()))?;

    // Start hot-reload monitoring
    if let Err(e) = config_manager.start_hot_reload().await {
        log_error_with_context(
            &RavenError::configuration(e.to_string()),
            "Failed to start configuration hot-reload",
        );
        warn!("Continuing without hot-reload capability");
    } else {
        info!("🔄 Configuration hot-reloading enabled");
    }

    Ok(config_manager)
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

    info!("📮 Dead letter queue initialized and processing started");
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

    info!("🔌 Circuit breakers initialized for database and gRPC components");
    Ok(circuit_breaker_registry)
}

/// Initialize InfluxDB client with enhanced error handling
pub async fn initialize_influx_client(
    config: &Config,
    dead_letter_queue: Arc<DeadLetterQueue>,
) -> RavenResult<(Arc<InfluxClient>, Arc<EnhancedInfluxClient>)> {
    let influx_config = InfluxConfig {
        url: config.database.influx_url.clone(),
        bucket: config.database.bucket.clone(),
        org: config.database.org.clone(),
        token: config.database.token.clone(),
        pool_size: config.database.connection_pool_size,
        timeout: Duration::from_secs(config.database.connection_timeout_seconds),
        retry_attempts: config.database.retry_attempts,
        retry_delay: Duration::from_millis(config.database.retry_delay_ms),
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
            &RavenError::database_connection(e.to_string()),
            "Failed to connect to InfluxDB",
        );
        raven_bail!(RavenError::database_connection(e.to_string()));
    }

    info!("🏦 InfluxDB client initialized with enhanced error handling");
    Ok((influx_client, enhanced_influx_client))
}

/// Initialize client manager
pub async fn initialize_client_manager(config: &Config) -> RavenResult<Arc<ClientManager>> {
    let client_manager_config = ClientManagerConfig {
        max_clients: config.server.max_connections,
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

    info!("👥 Client manager initialized with graceful disconnection handling");
    Ok(client_manager)
}

/// Initialize monitoring and observability services
pub async fn initialize_monitoring_services(
    config: &Config,
    influx_client: Arc<InfluxClient>,
    subscription_manager: Arc<SubscriptionManager>,
    hf_storage: Arc<HighFrequencyStorage>,
) -> RavenResult<(CrowService, Vec<tokio::task::JoinHandle<()>>)> {
    info!("📊 Initializing monitoring and observability services...");

    // Initialize tracing service
    let tracing_service = Arc::new(TracingService::new(config.monitoring.clone()));
    if let Err(e) = tracing_service.initialize().await {
        log_error_with_context(
            &RavenError::internal(e.to_string()),
            "Failed to initialize distributed tracing",
        );
        warn!("Continuing without distributed tracing");
    } else {
        info!("🔍 Distributed tracing initialized successfully");
    }

    // Initialize metrics service
    let metrics_service = Arc::new(
        MetricsService::new(config.monitoring.clone())
            .map_err(|e| RavenError::internal(e.to_string()))?,
    );

    let health_service = Arc::new(HealthService::new(
        config.monitoring.clone(),
        Arc::clone(&influx_client),
        Arc::clone(&subscription_manager),
        Arc::clone(&hf_storage),
    ));

    // Create monitoring service
    let monitoring_service = CrowService::new(
        Arc::clone(&health_service),
        Arc::clone(&metrics_service),
        Arc::clone(&tracing_service),
    );

    // Start monitoring services
    let monitoring_handles = monitoring_service
        .start()
        .await
        .map_err(|e| RavenError::internal(e.to_string()))?;

    info!("📊 Monitoring services started:");
    info!(
        "  🏥 Health checks on port {}",
        config.monitoring.health_check_port
    );
    info!("  📈 Metrics on port {}", config.monitoring.metrics_port);
    info!(
        "  🔍 Distributed tracing enabled: {}",
        config.monitoring.tracing_enabled
    );

    Ok((monitoring_service, monitoring_handles))
}
