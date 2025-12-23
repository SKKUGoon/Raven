use crate::proto::control_server::{Control, ControlServer};
use crate::proto::market_data_server::{MarketData, MarketDataServer};
use std::net::SocketAddr;
use tonic::transport::Server;

// Metrics
use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use prometheus::{Encoder, TextEncoder};
use tokio::net::TcpListener;

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

            let listener = match TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Failed to bind metrics server: {}", e);
                    return;
                }
            };

            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("Failed to accept metrics connection: {}", e);
                        continue;
                    }
                };

                let io = TokioIo::new(stream);

                tokio::task::spawn(async move {
                    if let Err(err) = http1::Builder::new()
                        .serve_connection(io, service_fn(metrics_handler))
                        .await
                    {
                        tracing::error!("Error serving metrics connection: {:?}", err);
                    }
                });
            }
        });
    }
}

async fn metrics_handler(
    _req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];

    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        tracing::error!("Failed to encode metrics: {}", e);
        return Ok(Response::builder()
            .status(500)
            .body(Full::new(Bytes::from("Internal Server Error")))
            .unwrap());
    }

    let response_bytes = Bytes::from(buffer);
    Ok(Response::new(Full::new(response_bytes)))
}

// Helper for client connection
use crate::proto::market_data_client::MarketDataClient;
use tonic::transport::Channel;

pub async fn connect_upstream(
    url: String,
) -> Result<MarketDataClient<Channel>, Box<dyn std::error::Error>> {
    let client = MarketDataClient::connect(url).await?;
    Ok(client)
}
