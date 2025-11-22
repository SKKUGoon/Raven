use raven::server::controller_service::CollectorManager;
use raven::server::data_engine::storage::HighFrequencyStorage;
use raven::server::data_engine::{DataEngine, DataEngineConfig};
use raven::server::database::influx_client::{InfluxClient, InfluxConfig};
use raven::server::database::DeadLetterQueue;
use raven::server::subscription_manager::SubscriptionManager;
use std::sync::Arc;

fn build_test_data_engine() -> (Arc<DataEngine>, Arc<SubscriptionManager>) {
    let influx_client = Arc::new(InfluxClient::new(InfluxConfig::default()));
    let subscription_manager = Arc::new(SubscriptionManager::new());
    let dead_letter_queue = Arc::new(DeadLetterQueue::new(Default::default()));

    let data_engine = Arc::new(DataEngine::new(
        DataEngineConfig::default(),
        influx_client,
        Arc::clone(&subscription_manager),
        dead_letter_queue,
    ));

    (data_engine, subscription_manager)
}

#[tokio::test]
async fn test_collector_manager_creation() {
    let hf_storage = Arc::new(HighFrequencyStorage::new());
    let (data_engine, subscription_manager) = build_test_data_engine();

    let manager = CollectorManager::new(hf_storage, data_engine, subscription_manager);
    assert_eq!(manager.active_count(), 0);
}

#[tokio::test]
async fn test_list_empty_collections() {
    let hf_storage = Arc::new(HighFrequencyStorage::new());
    let (data_engine, subscription_manager) = build_test_data_engine();

    let manager = CollectorManager::new(hf_storage, data_engine, subscription_manager);
    let collections = manager.list_collections();
    assert!(collections.is_empty());
}
