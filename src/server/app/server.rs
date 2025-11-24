use crate::common::config::RuntimeConfig;
use crate::common::db::{CircuitBreakerRegistry, DeadLetterQueue};
use crate::common::error::RavenResult;
use crate::proto::control_service_server::ControlServiceServer;
use crate::server::app::args::{parse_cli_args, print_version_info, CliArgs};
use crate::server::app::shutdown::wait_for_shutdown_signal;
use crate::server::app::startup;
use crate::server::grpc::client_service::manager::ClientManager;
use crate::server::grpc::client_service::MarketDataServer;
use crate::server::grpc::controller_service::CollectorManager;
use crate::server::grpc::controller_service::ControlServiceImpl;
use crate::server::prometheus::ObservabilityService;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tonic::transport::Server as TonicServer;
use tracing::info;

pub struct Server {
    config: RuntimeConfig,
    client_manager: Arc<ClientManager>,
    dead_letter_queue: Arc<DeadLetterQueue>,
    circuit_breaker_registry: Arc<CircuitBreakerRegistry>,
    observability_service: ObservabilityService,
    monitoring_handles: Vec<JoinHandle<()>>,
    collector_manager: Arc<CollectorManager>,
    market_data_server: MarketDataServer,
    control_service: ControlServiceImpl,
}

impl Server {
    pub fn builder() -> ServerBuilder {
        ServerBuilder::default()
    }

    pub async fn run(self) -> RavenResult<()> {
        info!("[Raven] Starting servers");
        info!(
            "Server {}:{}",
            self.config.server.host, self.config.server.port
        );
        info!("Metric check :{}", self.config.monitoring.metrics_port);
        info!("Health check :{}", self.config.monitoring.health_check_port);

        // Start the gRPC servers
        let market_data_server_handle =
            Self::spawn_market_data_server(self.market_data_server.clone(), &self.config);
        let control_server_handle =
            Self::spawn_control_server(self.control_service.clone(), &self.config);

        // Wait for shutdown signals
        wait_for_shutdown_signal().await?;

        // Stop gRPC servers
        market_data_server_handle.abort();
        control_server_handle.abort();

        // Stop all active collections
        self.collector_manager.stop_all_collections().await;

        // Perform graceful shutdown
        self.shutdown().await
    }

    fn spawn_market_data_server(
        server: MarketDataServer,
        config: &RuntimeConfig,
    ) -> JoinHandle<()> {
        let host = "0.0.0.0".to_string();
        let port = config.server.port;

        tokio::spawn(async move {
            info!("▶ Starting Market Data gRPC server on {}:{}", host, port);
            if let Err(e) = server.start(&host, port).await {
                tracing::error!("✗ Market Data gRPC server failed: {}", e);
            } else {
                info!("✓ Market Data gRPC server started successfully");
            }
        })
    }

    fn spawn_control_server(service: ControlServiceImpl, config: &RuntimeConfig) -> JoinHandle<()> {
        let host = "127.0.0.1".to_string();
        let port = config.server.port + 1;

        tokio::spawn(async move {
            let addr = format!("{host}:{port}").parse().unwrap();
            info!("▶ Starting Control gRPC server on {}", addr);
            let service = ControlServiceServer::new(service);
            if let Err(e) = TonicServer::builder()
                .add_service(service)
                .serve(addr)
                .await
            {
                tracing::error!("✗ Control gRPC server failed: {}", e);
            } else {
                info!("✓ Control gRPC server started successfully");
            }
        })
    }

    /// Perform graceful shutdown sequence
    async fn shutdown(&self) -> RavenResult<()> {
        info!("Shutting down Raven gracefully...");

        // Stop accepting new connections and disconnect existing clients
        if let Err(e) = self.client_manager.shutdown_all_clients().await {
            tracing::error!(error = %e, "Error during client shutdown");
        }

        // Stop dead letter queue processing and persist remaining entries
        self.dead_letter_queue.stop_processing();
        if let Err(e) = self.dead_letter_queue.persist_to_disk().await {
            tracing::error!(error = %e, "Failed to persist dead letter queue");
        }

        // Shutdown monitoring services
        info!("Shutting down monitoring services...");
        for handle in &self.monitoring_handles {
            handle.abort();
        }

        // Shutdown tracing and flush spans
        if let Err(e) = self.observability_service.tracing().shutdown().await {
            tracing::error!(error = %e, "Failed to shutdown tracing");
        }

        // Get final statistics
        let client_stats = self.client_manager.get_client_stats().await;
        let dlq_stats = self.dead_letter_queue.get_statistics().await;
        let cb_stats = self.circuit_breaker_registry.get_all_stats().await;

        info!("Final statistics:");
        info!("  * Clients: {:?}", client_stats);
        info!("  * Dead Letter Queue: {:?}", dlq_stats);
        info!("  * Circuit Breakers: {:?}", cb_stats);

        info!("Raven shutdown complete");

        Ok(())
    }
}

#[derive(Default)]
pub struct ServerBuilder {
    args: Option<CliArgs>,
}

impl ServerBuilder {
    pub fn with_cli_args(mut self, args: CliArgs) -> Self {
        self.args = Some(args);
        self
    }

    pub async fn build(self) -> RavenResult<Server> {
        // 1. Parse args if not provided
        let args = self.args.unwrap_or_else(parse_cli_args);

        print_version_info();
        startup::initialize_logging(&args)?;

        // 2. Load Config
        let loader = startup::build_config_loader(&args);
        let config = startup::load_and_validate_config(&loader, &args)?;
        startup::validate_dependencies(&config.server, &config.monitoring).await?;

        info!("Ready: [Server]");

        // 3. Init Infrastructure
        let dead_letter_queue = startup::initialize_dead_letter_queue().await?;
        let circuit_breaker_registry = startup::initialize_circuit_breakers().await?;

        // 4. Init Data Layer
        let (influx_client, enhanced_influx_client) =
            startup::initialize_influx_client(&config.database, dead_letter_queue.clone()).await?;

        let client_manager = startup::initialize_client_manager(&config.server).await?;

        info!("Ready: [DataEngine]");

        // 4. Init Data Layer
        let (stream_router, hf_storage, data_engine) =
            startup::initialize_data_layer(Arc::clone(&enhanced_influx_client));

        // 5. Init Monitoring
        let (observability_service, monitoring_handles) = startup::initialize_monitoring_services(
            &config.monitoring,
            Arc::clone(&influx_client),
            Arc::clone(&stream_router),
            Arc::clone(&hf_storage),
        )
        .await?;

        // 6. Init Services
        let (collector_manager, control_service, market_data_server) = startup::initialize_services(
            Arc::clone(&hf_storage),
            Arc::clone(&data_engine),
            Arc::clone(&stream_router),
            Arc::clone(&influx_client),
            Arc::clone(&client_manager),
        );

        Ok(Server {
            config,
            client_manager,
            dead_letter_queue,
            circuit_breaker_registry,
            observability_service,
            monitoring_handles,
            collector_manager,
            market_data_server,
            control_service,
        })
    }
}
