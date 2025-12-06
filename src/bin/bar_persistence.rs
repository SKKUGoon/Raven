use raven::config::Settings;
use raven::db::timescale;
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
        settings.server.host, settings.server.port_bar_persistence
    )
    .parse()?;

    // This service connects to aggregators (timebar_minutes, tibs)
    let timebar_minutes_upstream = format!(
        "http://{}:{}",
        settings.server.host, settings.server.port_timebar_minutes
    );

    let tibs_upstream = format!(
        "http://{}:{}",
        settings.server.host, settings.server.port_tibs
    );

    let mut upstreams = HashMap::new();
    upstreams.insert(
        "raven_timebar_minutes".to_string(),
        timebar_minutes_upstream.clone(),
    );
    upstreams.insert("raven_tibs".to_string(), tibs_upstream.clone());

    // Default mappings for standard exchange names to timebar_minutes (or as desired)
    upstreams.insert("BINANCE_SPOT".to_string(), timebar_minutes_upstream.clone());
    upstreams.insert(
        "BINANCE_FUTURES".to_string(),
        timebar_minutes_upstream.clone(),
    );

    let service_impl = timescale::new(
        timebar_minutes_upstream,
        upstreams,
        settings.timescale.clone(),
    )
    .await?;
    let raven = RavenService::new("BarPersistence", service_impl.clone());

    raven.serve(addr).await?;

    Ok(())
}
