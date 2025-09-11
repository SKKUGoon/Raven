use market_data_subscription_server::{
    circuit_breaker::CircuitBreakerRegistry,
    client_manager::ClientManager,
    dead_letter_queue::DeadLetterQueue,
    error::{RavenError, RavenResult},
    logging::log_error_with_context,
    monitoring::TracingService,
};
use std::sync::Arc;
use tracing::info;

/// Wait for shutdown signals
pub async fn wait_for_shutdown_signal() -> RavenResult<()> {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|e| RavenError::internal(format!("Failed to setup SIGTERM handler: {e}")))?;

    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(|e| RavenError::internal(format!("Failed to setup SIGINT handler: {e}")))?;

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

    Ok(())
}

/// Perform graceful shutdown sequence
pub async fn perform_graceful_shutdown(
    client_manager: Arc<ClientManager>,
    dead_letter_queue: Arc<DeadLetterQueue>,
    tracing_service: Arc<TracingService>,
    circuit_breaker_registry: Arc<CircuitBreakerRegistry>,
    monitoring_handles: Vec<tokio::task::JoinHandle<()>>,
) -> RavenResult<()> {
    info!("🌙 Shutting down Project Raven gracefully...");
    info!("The long night is ending, but the watch continues");

    // Graceful shutdown sequence
    info!("🛑 Starting graceful shutdown sequence...");

    // 1. Stop accepting new connections and disconnect existing clients
    if let Err(e) = client_manager.shutdown_all_clients().await {
        log_error_with_context(&e, "Error during client shutdown");
    }

    // 2. Stop dead letter queue processing and persist remaining entries
    dead_letter_queue.stop_processing();
    if let Err(e) = dead_letter_queue.persist_to_disk().await {
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
    if let Err(e) = tracing_service.shutdown().await {
        log_error_with_context(
            &RavenError::internal(e.to_string()),
            "Failed to shutdown tracing",
        );
    }

    // 5. Get final statistics
    let client_stats = client_manager.get_client_stats().await;
    let dlq_stats = dead_letter_queue.get_statistics().await;
    let cb_stats = circuit_breaker_registry.get_all_stats().await;

    info!("📊 Final statistics:");
    info!("  👥 Clients: {:?}", client_stats);
    info!("  📮 Dead Letter Queue: {:?}", dlq_stats);
    info!("  🔌 Circuit Breakers: {:?}", cb_stats);

    info!("🌙 Project Raven shutdown complete");
    info!("The Night's Watch ends, but the realm remembers");

    Ok(())
}
