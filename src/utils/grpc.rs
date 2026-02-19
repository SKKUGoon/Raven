use std::time::Duration;

use tonic::transport::Channel;

use crate::proto::control_client::ControlClient;
use crate::proto::ListRequest;

/// Probe a Control endpoint until it responds to ListCollections (or timeout).
pub async fn wait_for_control_ready(addr: &str, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    loop {
        if start.elapsed() > timeout {
            return false;
        }

        if let Ok(mut client) = ControlClient::<Channel>::connect(addr.to_string()).await {
            if client.list_collections(ListRequest {}).await.is_ok() {
                return true;
            }
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
