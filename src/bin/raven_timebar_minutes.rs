use raven::config::Settings;
use raven::features::timebar_minutes;
use raven::service::RavenService;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;

    let log_level = match settings.logging.level.to_lowercase().as_str() {
        "debug" => tracing::Level::DEBUG,
        "error" => tracing::Level::ERROR,
        "warn" => tracing::Level::WARN,
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::fmt().with_max_level(log_level).init();

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_timebar_minutes
    )
    .parse()?;

    let mut upstreams = HashMap::new();
    // Standard names
    upstreams.insert(
        "BINANCE_SPOT".to_string(),
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_spot
        ),
    );
    upstreams.insert(
        "BINANCE_FUTURES".to_string(),
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_futures
        ),
    );

    let service_impl = timebar_minutes::new(upstreams);
    let raven = RavenService::new("RavenTimeBarMinutes", service_impl.clone());

    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}
