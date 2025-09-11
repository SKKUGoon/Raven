use super::*;
use crate::error::RavenError;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

struct TestRetryHandler {
    operation_type: String,
    success_count: Arc<AtomicU32>,
    should_fail: bool,
}

#[async_trait::async_trait]
impl RetryHandler for TestRetryHandler {
    async fn retry_operation(&self, _entry: &DeadLetterEntry) -> RavenResult<()> {
        if self.should_fail {
            Err(RavenError::internal("Test failure"))
        } else {
            self.success_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    fn operation_type(&self) -> &str {
        &self.operation_type
    }
}

#[tokio::test]
async fn test_dead_letter_entry_creation() {
    let entry = DeadLetterEntry::new("test_operation", "test_data", "test_error", 3);

    assert_eq!(entry.operation_type, "test_operation");
    assert_eq!(entry.data, "test_data");
    assert_eq!(entry.last_error, "test_error");
    assert_eq!(entry.max_retries, 3);
    assert_eq!(entry.retry_count, 0);
    assert!(!entry.is_exhausted());
}

#[tokio::test]
async fn test_dead_letter_queue_add_entry() {
    let config = DeadLetterQueueConfig::default();
    let queue = DeadLetterQueue::new(config);

    let entry = DeadLetterEntry::new("test_operation", "test_data", "test_error", 3);
    queue.add_entry(entry).await.unwrap();

    let stats = queue.get_statistics().await;
    assert_eq!(stats.total_entries, 1);
}

#[tokio::test]
async fn test_retry_handler_registration() {
    let config = DeadLetterQueueConfig::default();
    let queue = DeadLetterQueue::new(config);

    let handler = Box::new(TestRetryHandler {
        operation_type: "test_operation".to_string(),
        success_count: Arc::new(AtomicU32::new(0)),
        should_fail: false,
    });

    queue.register_retry_handler(handler).await;

    let handlers = queue.retry_handlers.read().await;
    assert!(handlers.contains_key("test_operation"));
}

#[tokio::test]
async fn test_entry_retry_backoff() {
    let mut entry = DeadLetterEntry::new("test_operation", "test_data", "test_error", 3);

    let initial_retry_time = entry.next_retry_timestamp;
    entry.prepare_for_retry("Another error".to_string());

    assert_eq!(entry.retry_count, 1);
    assert_eq!(entry.last_error, "Another error");
    assert!(entry.next_retry_timestamp > initial_retry_time);
}

#[tokio::test]
async fn test_entry_exhaustion() {
    let mut entry = DeadLetterEntry::new("test_operation", "test_data", "test_error", 2);

    assert!(!entry.is_exhausted());

    entry.prepare_for_retry("Error 1".to_string());
    assert!(!entry.is_exhausted());

    entry.prepare_for_retry("Error 2".to_string());
    assert!(entry.is_exhausted());
}

#[tokio::test]
async fn test_queue_size_limit() {
    let config = DeadLetterQueueConfig {
        max_size: 2,
        ..DeadLetterQueueConfig::default()
    };
    let queue = DeadLetterQueue::new(config);

    // Add entries up to the limit
    for i in 0..3 {
        let entry = DeadLetterEntry::new(
            "test_operation".to_string(),
            format!("test_data_{i}"),
            "test_error".to_string(),
            3,
        );
        queue.add_entry(entry).await.unwrap();
    }

    let stats = queue.get_statistics().await;
    assert_eq!(stats.total_entries, 2); // Should be limited to max_size
}
