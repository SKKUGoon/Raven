use raven::citadel::{Citadel, CitadelConfig};
use raven::control::CollectorManager;
use raven::database::influx_client::{InfluxClient, InfluxConfig};
use raven::subscription_manager::SubscriptionManager;
use raven::types::HighFrequencyStorage;
use std::sync::Arc;

fn build_test_citadel() -> Arc<Citadel> {
    let influx_client = Arc::new(InfluxClient::new(InfluxConfig::default()));
    let subscription_manager = Arc::new(SubscriptionManager::new());

    Arc::new(Citadel::new(
        CitadelConfig::default(),
        influx_client,
        subscription_manager,
    ))
}

#[tokio::test]
async fn test_collector_manager_creation() {
    let hf_storage = Arc::new(HighFrequencyStorage::new());
    let citadel = build_test_citadel();

    let manager = CollectorManager::new(hf_storage, citadel);
    assert_eq!(manager.active_count(), 0);
}

#[tokio::test]
async fn test_list_empty_collections() {
    let hf_storage = Arc::new(HighFrequencyStorage::new());
    let citadel = build_test_citadel();

    let manager = CollectorManager::new(hf_storage, citadel);
    let collections = manager.list_collections();
    assert!(collections.is_empty());
}
