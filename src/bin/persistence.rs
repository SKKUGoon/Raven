use raven::config::Settings;
use raven::db::influx;
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
        settings.server.host, settings.server.port_persistence
    )
    .parse()?;

    let spot_upstream = format!(
        "http://{}:{}",
        settings.server.host, settings.server.port_spot
    );

    let futures_upstream = format!(
        "http://{}:{}",
        settings.server.host, settings.server.port_futures
    );

    let mut upstreams = HashMap::new();
    upstreams.insert("SPOT".to_string(), spot_upstream.clone());
    upstreams.insert("FUTURES".to_string(), futures_upstream);

    let service_impl = influx::new(spot_upstream, upstreams, settings.influx.clone());
    let raven = RavenService::new("Persistence", service_impl.clone());

    raven.serve(addr).await?;

    Ok(())
}
