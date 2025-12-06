use crate::proto::control_server::{Control, ControlServer};
use crate::proto::market_data_server::{MarketData, MarketDataServer};
use std::net::SocketAddr;
use tonic::transport::Server;

// Metrics
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server as HttpServer};
use prometheus::{Encoder, TextEncoder};

pub mod stream_manager;
pub use stream_manager::{StreamManager, StreamWorker};

pub struct RavenService<C> {
    control: C,
    name: String,
}

impl<C> RavenService<C>
where
    C: Control,
{
    pub fn new(name: &str, control: C) -> Self {
        Self {
            control,
            name: name.to_string(),
        }
    }

    pub async fn serve_with_market_data<M>(
        self,
        addr: SocketAddr,
        market_data: M,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        M: MarketData,
    {
        tracing::info!("Starting {} on {}", self.name, addr);
        
        // Start Prometheus metrics server on port + 1000
        let metrics_port = addr.port() + 1000;
        let metrics_addr = SocketAddr::from(([0, 0, 0, 0], metrics_port));
        self.spawn_metrics_server(metrics_addr);

        Server::builder()
            .add_service(ControlServer::new(self.control))
            .add_service(MarketDataServer::new(market_data))
            .serve(addr)
            .await?;
        Ok(())
    }

    pub async fn serve(self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Starting {} on {}", self.name, addr);
        
        // Start Prometheus metrics server on port + 1000
        let metrics_port = addr.port() + 1000;
        let metrics_addr = SocketAddr::from(([0, 0, 0, 0], metrics_port));
        self.spawn_metrics_server(metrics_addr);

        Server::builder()
            .add_service(ControlServer::new(self.control))
            .serve(addr)
            .await?;
        Ok(())
    }

    fn spawn_metrics_server(&self, addr: SocketAddr) {
        let name = self.name.clone();
        tokio::spawn(async move {
            tracing::info!("Starting metrics server for {} on {}", name, addr);
            
            let serve_future = HttpServer::bind(&addr).serve(make_service_fn(|_| async {
                Ok::<_, hyper::Error>(service_fn(metrics_handler))
            }));

            if let Err(e) = serve_future.await {
                tracing::error!("Metrics server error: {}", e);
            }
        });
    }
}

async fn metrics_handler(_req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        tracing::error!("Failed to encode metrics: {}", e);
        return Ok(Response::builder()
            .status(500)
            .body(Body::from("Internal Server Error"))
            .unwrap());
    }
    
    let response_str = String::from_utf8(buffer).unwrap();
    Ok(Response::new(Body::from(response_str)))
}

// Helper for client connection
use crate::proto::market_data_client::MarketDataClient;
use tonic::transport::Channel;

pub async fn connect_upstream(url: String) -> Result<MarketDataClient<Channel>, Box<dyn std::error::Error>> {
    let client = MarketDataClient::connect(url).await?;
    Ok(client)
}
