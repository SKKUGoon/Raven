use raven::service::RavenService;
use raven::source::binance::future;
use raven::config::Settings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;

    let log_level = match settings.logging.level.to_lowercase().as_str() {
        "debug" => tracing::Level::DEBUG,
        "error" => tracing::Level::ERROR,
        "warn" => tracing::Level::WARN,
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .init();

    let addr = format!("{}:{}", settings.server.host, settings.server.port_binance_futures).parse()?;
    let service_impl = future::new();
    let raven = RavenService::new("BinanceFutures", service_impl.clone());

    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}
