use raven::features::tibs::TibsService;
use raven::service::RavenService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Tibs runs on 50055 by default
    let addr = "0.0.0.0:50055".parse()?;
    let upstream =
        std::env::var("UPSTREAM_URL").unwrap_or_else(|_| "http://localhost:50051".to_string());

    let service_impl = TibsService::new(upstream);
    let raven = RavenService::new("RavenTibs", service_impl.clone());

    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}
