use raven::exchange::binance_futures::BinanceFuturesService;
use raven::service::RavenService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let addr = "0.0.0.0:50054".parse()?;
    let service_impl = BinanceFuturesService::new();
    let raven = RavenService::new("BinanceFutures", service_impl.clone());

    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}
