use raven::config::Settings;
use raven::db::timescale;
use raven::service::RavenService;

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

    // This service connects to aggregators (timebar, tibs)
    let timebar_upstream = format!(
        "http://{}:{}",
        settings.server.host, settings.server.port_timebar_minutes
    );

    let tibs_upstream = format!(
        "http://{}:{}",
        settings.server.host, settings.server.port_tibs
    );

    let service_impl = timescale::new(
        timebar_upstream,
        tibs_upstream,
        settings.timescale.clone(),
    )
    .await?;
    let raven = RavenService::new("BarPersistence", service_impl.clone());

    raven.serve(addr).await?;

    Ok(())
}
