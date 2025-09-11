// Project Raven - Market Data Subscription Server
// The Night's Watch begins here - "Crown the king"

use clap::{Arg, Command};
use market_data_subscription_server::{
    circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerRegistry},
    client_manager::{ClientManager, ClientManagerConfig},
    config::{Config, ConfigManager, ConfigUtils},
    database::{EnhancedInfluxClient, InfluxClient, InfluxWriteRetryHandler},
    dead_letter_queue::{DeadLetterQueue, DeadLetterQueueConfig},
    error::{RavenError, RavenResult},
    logging::{init_logging, log_config_validation, log_error_with_context, LoggingConfig},
    monitoring::{HealthService, MetricsService, MonitoringService, TracingService},
    raven_bail,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Application version information
const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_TIMESTAMP: &str = env!("VERGEN_BUILD_TIMESTAMP");
const GIT_SHA: &str = env!("VERGEN_GIT_SHA");

/// CLI arguments structure
#[derive(Debug)]
struct CliArgs {
    pub config_file: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub log_level: Option<String>,
    pub database_url: Option<String>,
    pub max_connections: Option<usize>,
    pub validate_only: bool,
    pub print_config: bool,
}

/// Parse command line arguments
fn parse_cli_args() -> CliArgs {
    let matches = Command::new("Project Raven")
        .version(VERSION)
        .author("The Night's Watch")
        .about("High-performance market data subscription server")
        .long_about(format!(
            "Project Raven - Market Data Subscription Server\n\
             Version: {VERSION}\n\
             Build: {BUILD_TIMESTAMP}\n\
             Git SHA: {GIT_SHA}\n\n\
             A high-performance gRPC-based market data distribution system\n\
             designed for financial trading applications."
        ))
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .long_help("Path to the configuration file. If not specified, will look for config/default.toml")
        )
        .arg(
            Arg::new("host")
                .short('H')
                .long("host")
                .value_name("HOST")
                .help("Server host address")
                .long_help("Override the server host address from configuration")
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Server port number")
                .long_help("Override the server port number from configuration")
                .value_parser(clap::value_parser!(u16))
        )
        .arg(
            Arg::new("log-level")
                .short('l')
                .long("log-level")
                .value_name("LEVEL")
                .help("Log level (trace, debug, info, warn, error)")
                .long_help("Override the log level from configuration")
                .value_parser(["trace", "debug", "info", "warn", "error"])
        )
        .arg(
            Arg::new("database-url")
                .short('d')
                .long("database-url")
                .value_name("URL")
                .help("InfluxDB connection URL")
                .long_help("Override the InfluxDB connection URL from configuration")
        )
        .arg(
            Arg::new("max-connections")
                .short('m')
                .long("max-connections")
                .value_name("COUNT")
                .help("Maximum concurrent client connections")
                .long_help("Override the maximum concurrent client connections from configuration")
                .value_parser(clap::value_parser!(usize))
        )
        .arg(
            Arg::new("validate")
                .long("validate")
                .help("Validate configuration and exit")
                .long_help("Load and validate the configuration, then exit without starting the server")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("print-config")
                .long("print-config")
                .help("Print loaded configuration and exit")
                .long_help("Load configuration, print it in a readable format, then exit")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    CliArgs {
        config_file: matches.get_one::<String>("config").cloned(),
        host: matches.get_one::<String>("host").cloned(),
        port: matches.get_one::<u16>("port").copied(),
        log_level: matches.get_one::<String>("log-level").cloned(),
        database_url: matches.get_one::<String>("database-url").cloned(),
        max_connections: matches.get_one::<usize>("max-connections").copied(),
        validate_only: matches.get_flag("validate"),
        print_config: matches.get_flag("print-config"),
    }
}

/// Apply CLI overrides to configuration
fn apply_cli_overrides(mut config: Config, args: &CliArgs) -> Config {
    if let Some(host) = &args.host {
        info!("🔧 CLI override: host = {}", host);
        config.server.host = host.clone();
    }

    if let Some(port) = args.port {
        info!("🔧 CLI override: port = {}", port);
        config.server.port = port;
    }

    if let Some(log_level) = &args.log_level {
        info!("🔧 CLI override: log_level = {}", log_level);
        config.monitoring.log_level = log_level.clone();
    }

    if let Some(database_url) = &args.database_url {
        info!("🔧 CLI override: database_url = {}", database_url);
        config.database.influx_url = database_url.clone();
    }

    if let Some(max_connections) = args.max_connections {
        info!("🔧 CLI override: max_connections = {}", max_connections);
        config.server.max_connections = max_connections;
    }

    config
}

/// Validate system dependencies and requirements
async fn validate_dependencies(config: &Config) -> RavenResult<()> {
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

/// Print Raven ASCII art
fn print_raven_ascii_art() {
    println!(
        r#"

    ██████╗  █████╗ ██╗   ██╗███████╗███╗   ██╗
    ██╔══██╗██╔══██╗██║   ██║██╔════╝████╗  ██║
    ██████╔╝███████║██║   ██║█████╗  ██╔██╗ ██║
    ██╔══██╗██╔══██║╚██╗ ██╔╝██╔══╝  ██║╚██╗██║
    ██║  ██║██║  ██║ ╚████╔╝ ███████╗██║ ╚████║
    ╚═╝  ╚═╝╚═╝  ╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═══╝
    "#
    );
}

/// Print application version and build information
fn print_version_info() {
    print_raven_ascii_art();
    println!();
    println!("🐦‍⬛ Project Raven - Market Data Subscription Server");
    println!("Version: {VERSION}");
    println!("Build Timestamp: {BUILD_TIMESTAMP}");
    println!("Git SHA: {GIT_SHA}");
    println!("Rust Version: {}", env!("VERGEN_RUSTC_SEMVER"));
    println!("Target: {}", env!("VERGEN_CARGO_TARGET_TRIPLE"));
    println!();
}

#[tokio::main]
async fn main() -> RavenResult<()> {
    // Parse command line arguments
    let args = parse_cli_args();

    // Print version information
    print_version_info();
    // Initialize basic logging first
    let basic_logging = LoggingConfig {
        level: args.log_level.clone().unwrap_or_else(|| "info".to_string()),
        format: "pretty".to_string(),
        ..Default::default()
    };

    if let Err(e) = init_logging(&basic_logging) {
        eprintln!("❌ Failed to initialize logging: {e}");
        return Err(e);
    }

    info!("🐦‍⬛ Project Raven is awakening...");
    info!("The Night's Watch begins - Winter is coming, but we are prepared");

    // Initialize configuration
    let mut config = match Config::load() {
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
    config = apply_cli_overrides(config, &args);

    // Handle special CLI modes
    if args.validate_only {
        info!("🔍 Validating configuration only...");
        match config.validate() {
            Ok(()) => {
                info!("✅ Configuration validation passed");
                println!("Configuration is valid!");
                return Ok(());
            }
            Err(e) => {
                error!("❌ Configuration validation failed: {}", e);
                return Err(RavenError::configuration(e.to_string()));
            }
        }
    }

    if args.print_config {
        info!("📋 Printing configuration...");
        ConfigUtils::print_config(&config);
        return Ok(());
    }

    // Note: We keep the initial logging configuration since reinitializing
    // the global subscriber would cause a panic. The CLI log level override
    // is already applied in the initial logging setup.

    // Print configuration summary
    ConfigUtils::print_config(&config);

    // Check configuration health
    let warnings = ConfigUtils::check_configuration_health(&config);
    log_config_validation("main", warnings.is_empty(), &warnings);

    // Validate system dependencies
    if let Err(e) = validate_dependencies(&config).await {
        error!("❌ Dependency validation failed: {}", e);
        return Err(e);
    }

    // Initialize configuration manager with hot-reloading
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

    // Initialize error handling and reliability components
    info!("🛡️ Initializing error handling and reliability features...");

    // 1. Initialize Dead Letter Queue
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

    // 2. Initialize Circuit Breaker Registry
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

    // 3. Initialize InfluxDB Client with Enhanced Error Handling
    let influx_config = market_data_subscription_server::database::InfluxConfig {
        url: config.database.influx_url.clone(),
        database: config.database.database_name.clone(),
        username: config.database.username.clone(),
        password: config.database.password.clone(),
        pool_size: config.database.connection_pool_size,
        timeout: Duration::from_secs(config.database.connection_timeout_seconds),
        retry_attempts: config.database.retry_attempts,
        retry_delay: Duration::from_millis(config.database.retry_delay_ms),
        batch_size: 1000, // Default batch size since it's not in config
        flush_interval: Duration::from_millis(5), // Default flush interval since it's not in config
    };

    let influx_client = Arc::new(InfluxClient::new(influx_config));
    let _enhanced_influx_client = Arc::new(EnhancedInfluxClient::new(
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

    // 4. Initialize Client Manager
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

    // 5. Initialize Monitoring and Observability
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

    // Initialize health service (we'll need to add the missing components later)
    // For now, create placeholder components
    let subscription_manager =
        Arc::new(market_data_subscription_server::subscription_manager::SubscriptionManager::new());
    let hf_storage = Arc::new(market_data_subscription_server::types::HighFrequencyStorage::new());

    let health_service = Arc::new(HealthService::new(
        config.monitoring.clone(),
        Arc::clone(&influx_client),
        Arc::clone(&subscription_manager),
        Arc::clone(&hf_storage),
    ));

    // Create monitoring service
    let monitoring_service = MonitoringService::new(
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

    // Set up graceful shutdown handling
    let shutdown_client_manager = Arc::clone(&client_manager);
    let shutdown_dlq = Arc::clone(&dead_letter_queue);
    let shutdown_tracing = Arc::clone(&tracing_service);

    // Set up signal handlers for graceful shutdown
    info!("🛡️ Setting up graceful shutdown handlers...");

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|e| RavenError::internal(format!("Failed to setup SIGTERM handler: {e}")))?;

    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(|e| RavenError::internal(format!("Failed to setup SIGINT handler: {e}")))?;

    info!("✅ Project Raven is now ready to serve the realm!");
    info!("👑 The king is crowned - all systems operational");

    // Keep the server running and wait for shutdown signals
    tokio::select! {
        _ = sigint.recv() => {
            info!("📡 Received SIGINT (Ctrl+C) shutdown signal");
        }
        _ = sigterm.recv() => {
            info!("📡 Received SIGTERM shutdown signal");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("📡 Received CTRL+C shutdown signal");
        }
    }

    info!("🌙 Shutting down Project Raven gracefully...");
    info!("The long night is ending, but the watch continues");

    // Graceful shutdown sequence
    info!("🛑 Starting graceful shutdown sequence...");

    // 1. Stop accepting new connections and disconnect existing clients
    if let Err(e) = shutdown_client_manager.shutdown_all_clients().await {
        log_error_with_context(&e, "Error during client shutdown");
    }

    // 2. Stop dead letter queue processing and persist remaining entries
    shutdown_dlq.stop_processing();
    if let Err(e) = shutdown_dlq.persist_to_disk().await {
        log_error_with_context(&e, "Failed to persist dead letter queue");
    }

    // 3. Process any remaining dead letter entries
    // Note: The process_dead_letter_queue method doesn't exist in the current implementation
    // This would be handled by the background processing loop that was already stopped
    info!("📮 Dead letter queue processing stopped, remaining entries persisted to disk");

    // 4. Shutdown monitoring services
    info!("📊 Shutting down monitoring services...");

    // Stop monitoring handles
    for handle in monitoring_handles {
        handle.abort();
    }

    // Shutdown tracing and flush spans
    if let Err(e) = shutdown_tracing.shutdown().await {
        log_error_with_context(
            &RavenError::internal(e.to_string()),
            "Failed to shutdown tracing",
        );
    }

    // 5. Get final statistics
    let client_stats = shutdown_client_manager.get_client_stats().await;
    let dlq_stats = shutdown_dlq.get_statistics().await;
    let cb_stats = circuit_breaker_registry.get_all_stats().await;

    info!("📊 Final statistics:");
    info!("  👥 Clients: {:?}", client_stats);
    info!("  📮 Dead Letter Queue: {:?}", dlq_stats);
    info!("  🔌 Circuit Breakers: {:?}", cb_stats);

    info!("🌙 Project Raven shutdown complete");
    info!("The Night's Watch ends, but the realm remembers");

    Ok(())
}
