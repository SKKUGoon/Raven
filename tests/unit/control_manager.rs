use raven::common::db::{DeadLetterQueue, EnhancedInfluxClient, InfluxClient, InfluxConfig};
use raven::server::data_engine::storage::HighFrequencyStorage;
use raven::server::data_engine::{DataEngine, DataEngineConfig};
use raven::server::grpc::controller_service::CollectorManager;
use raven::server::stream_router::StreamRouter;
use std::sync::Arc;

fn build_test_data_engine() -> (Arc<DataEngine>, Arc<StreamRouter>) {
    let influx_client = Arc::new(InfluxClient::new(InfluxConfig::default()));
    let dead_letter_queue = Arc::new(DeadLetterQueue::new(Default::default()));
    let enhanced_client = Arc::new(EnhancedInfluxClient::new(
        influx_client,
        Arc::clone(&dead_letter_queue),
    ));
    let subscription_manager = Arc::new(StreamRouter::new());

    let data_engine = Arc::new(DataEngine::new(
        DataEngineConfig::default(),
        enhanced_client,
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
