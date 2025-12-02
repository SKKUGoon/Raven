use raven::db::influx;
use raven::service::RavenService;
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

    let addr = format!("{}:{}", settings.server.host, settings.server.port_persistence).parse()?;
    let upstream = format!("http://{}:{}", settings.server.host, settings.server.port_spot);

    let service_impl = influx::new(upstream, settings.influx.clone());
    let raven = RavenService::new("Persistence", service_impl.clone());

    raven.serve(addr).await?;

    Ok(())
}
