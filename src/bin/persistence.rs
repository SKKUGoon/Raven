use raven::db::influx::PersistenceService;
use raven::service::RavenService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Persistence runs on 50052 by default, upstream on 50051
    let addr = "0.0.0.0:50052".parse()?;
    let upstream =
        std::env::var("UPSTREAM_URL").unwrap_or_else(|_| "http://localhost:50051".to_string());

    let service_impl = PersistenceService::new(upstream);
    let raven = RavenService::new("Persistence", service_impl.clone());

    raven.serve(addr).await?;

    Ok(())
}
