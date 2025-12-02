use raven::features::bars::BarService;
use raven::service::RavenService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Bars runs on 50053 by default
    let addr = "0.0.0.0:50053".parse()?;
    let upstream =
        std::env::var("UPSTREAM_URL").unwrap_or_else(|_| "http://localhost:50051".to_string());

    let service_impl = BarService::new(upstream);
    let raven = RavenService::new("RavenBars", service_impl.clone());

    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}
