use crate::common::error::RavenResult;
use crate::common::current_timestamp_millis;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Dead letter queue entry representing a failed operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    /// Unique identifier for the entry
    pub id: String,
    /// The failed operation data (serialized)
    pub data: String,
    /// Type of operation that failed
    pub operation_type: String,
    /// Original timestamp when the operation was first attempted
    pub original_timestamp: i64,
    /// Timestamp when the entry was added to the dead letter queue
    pub dead_letter_timestamp: i64,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Maximum number of retries allowed
    pub max_retries: u32,
    /// Error message from the last failure
    pub last_error: String,
    /// Next retry timestamp (for exponential backoff)
    pub next_retry_timestamp: i64,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl DeadLetterEntry {
    /// Create a new dead letter entry
    pub fn new<S: Into<String>>(
        operation_type: S,
        data: S,
        error_message: S,
        max_retries: u32,
    ) -> Self {
        let now = current_timestamp_millis();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            data: data.into(),
            operation_type: operation_type.into(),
            original_timestamp: now,
            dead_letter_timestamp: now,
            retry_count: 0,
            max_retries,
            last_error: error_message.into(),
            next_retry_timestamp: now + 1000, // 1 second initial delay
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Check if the entry is ready for retry
    pub fn is_ready_for_retry(&self) -> bool {
        let now = current_timestamp_millis();

        self.retry_count < self.max_retries && now >= self.next_retry_timestamp
    }

    /// Check if the entry has exceeded maximum retries
    pub fn is_exhausted(&self) -> bool {
        self.retry_count >= self.max_retries
    }

    /// Update the entry for the next retry with exponential backoff
    pub fn prepare_for_retry(&mut self, error_message: String) {
        self.retry_count += 1;
        self.last_error = error_message;

        // Exponential backoff: 1s, 2s, 4s, 8s, 16s, max 60s
        let delay_seconds = std::cmp::min(1u64 << self.retry_count, 60);
        let delay_ms = delay_seconds * 1000;

        let now = current_timestamp_millis();

        self.next_retry_timestamp = now + delay_ms as i64;
    }

    /// Add metadata to the entry
    pub fn with_metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get age of the entry in milliseconds
    pub fn age_ms(&self) -> i64 {
        let now = current_timestamp_millis();
        now - self.dead_letter_timestamp
    }
}

/// Configuration for the dead letter queue
#[derive(Debug, Clone)]
pub struct DeadLetterQueueConfig {
    /// Maximum number of entries in the queue
    pub max_size: usize,
    /// Maximum age of entries before they are purged (in milliseconds)
    pub max_age_ms: i64,
    /// Interval for processing retries (in milliseconds)
    pub processing_interval_ms: u64,
    /// Default maximum retries for new entries
    pub default_max_retries: u32,
    /// Whether to persist the queue to disk
    pub persist_to_disk: bool,
    /// File path for persistence (if enabled)
    pub persistence_file: Option<String>,
}

impl Default for DeadLetterQueueConfig {
    fn default() -> Self {
        Self {
            max_size: 10000,
            max_age_ms: 24 * 60 * 60 * 1000, // 24 hours
            processing_interval_ms: 30000,   // 30 seconds
            default_max_retries: 3,
            persist_to_disk: false,
            persistence_file: None,
        }
    }
}

/// Statistics for the dead letter queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterQueueStats {
    pub total_entries: usize,
    pub ready_for_retry: usize,
    pub exhausted_entries: usize,
    pub oldest_entry_age_ms: i64,
    pub newest_entry_age_ms: i64,
    pub total_retries_attempted: u64,
    pub successful_retries: u64,
    pub failed_retries: u64,
    pub entries_by_operation_type: std::collections::HashMap<String, usize>,
}

/// Trait for handling retry operations
#[async_trait::async_trait]
pub trait RetryHandler: Send + Sync {
    /// Attempt to retry a failed operation
    async fn retry_operation(&self, entry: &DeadLetterEntry) -> RavenResult<()>;

    /// Get the operation type this handler supports
    fn operation_type(&self) -> &str;
}

/// Dead letter queue implementation
pub struct DeadLetterQueue {
    config: DeadLetterQueueConfig,
    entries: Arc<Mutex<VecDeque<DeadLetterEntry>>>,
    stats: Arc<RwLock<DeadLetterQueueStats>>,
    retry_handlers: Arc<RwLock<std::collections::HashMap<String, Box<dyn RetryHandler>>>>,
    processing_active: Arc<std::sync::atomic::AtomicBool>,
}

impl DeadLetterQueue {
    /// Create a new dead letter queue
    pub fn new(config: DeadLetterQueueConfig) -> Self {
        info!(
            "⚬ Initializing dead letter queue with max size: {}",
            config.max_size
        );

        Self {
            config,
            entries: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(DeadLetterQueueStats {
                total_entries: 0,
                ready_for_retry: 0,
                exhausted_entries: 0,
                oldest_entry_age_ms: 0,
                newest_entry_age_ms: 0,
                total_retries_attempted: 0,
                successful_retries: 0,
                failed_retries: 0,
                entries_by_operation_type: std::collections::HashMap::new(),
            })),
            retry_handlers: Arc::new(RwLock::new(std::collections::HashMap::new())),
            processing_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Add a failed operation to the dead letter queue
    pub async fn add_entry(&self, entry: DeadLetterEntry) -> RavenResult<()> {
        let mut entries = self.entries.lock().await;

        // Check if queue is full
        if entries.len() >= self.config.max_size {
            // Remove oldest entry to make space
            if let Some(removed) = entries.pop_front() {
                warn!(
                    "⚬ Dead letter queue full, removing oldest entry: {} (age: {}ms)",
                    removed.id,
                    removed.age_ms()
                );
            }
        }

        let operation_type = entry.operation_type.clone();
        entries.push_back(entry.clone());

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_entries = entries.len();
        *stats
            .entries_by_operation_type
            .entry(operation_type)
            .or_insert(0) += 1;

        info!(
            "⚬ Added entry to dead letter queue: {} (type: {}, retry: {}/{})",
            entry.id, entry.operation_type, entry.retry_count, entry.max_retries
        );

        Ok(())
    }

    /// Register a retry handler for a specific operation type
    pub async fn register_retry_handler(&self, handler: Box<dyn RetryHandler>) {
        let operation_type = handler.operation_type().to_string();
        let mut handlers = self.retry_handlers.write().await;
        handlers.insert(operation_type.clone(), handler);

        info!(
            "⚬ Registered retry handler for operation type: {}",
            operation_type
        );
    }

    /// Start the background processing loop
    pub async fn start_processing(&self) -> RavenResult<()> {
        if self
            .processing_active
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
        {
            let queue = Arc::new(self.clone());
            tokio::spawn(async move {
                queue.processing_loop().await;
            });

            info!("⚬ Dead letter queue processing started");
        }

        Ok(())
    }

    /// Stop the background processing loop
    pub fn stop_processing(&self) {
        self.processing_active
            .store(false, std::sync::atomic::Ordering::Relaxed);
        info!("⚬ Dead letter queue processing stopped");
    }

    /// Background processing loop
    async fn processing_loop(&self) {
        let mut interval = interval(Duration::from_millis(self.config.processing_interval_ms));

        while self
            .processing_active
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            interval.tick().await;

            if let Err(e) = self.process_entries().await {
                error!("⚬ Error processing dead letter queue: {}", e);
            }

            if let Err(e) = self.cleanup_old_entries().await {
                error!("⚬ Error cleaning up old entries: {}", e);
            }

            self.update_statistics().await;
        }
    }

    /// Process entries that are ready for retry
    async fn process_entries(&self) -> RavenResult<()> {
        let mut entries = self.entries.lock().await;
        let handlers = self.retry_handlers.read().await;
        let mut stats = self.stats.write().await;

        let mut processed_indices = Vec::new();

        for (index, entry) in entries.iter_mut().enumerate() {
            if !entry.is_ready_for_retry() {
                continue;
            }

            if let Some(handler) = handlers.get(&entry.operation_type) {
                stats.total_retries_attempted += 1;

                match handler.retry_operation(entry).await {
                    Ok(()) => {
                        stats.successful_retries += 1;
                        processed_indices.push(index);
                        info!(
                            "✓ Successfully retried dead letter entry: {} (type: {})",
                            entry.id, entry.operation_type
                        );
                    }
                    Err(e) => {
                        stats.failed_retries += 1;
                        entry.prepare_for_retry(e.to_string());

                        if entry.is_exhausted() {
                            warn!(
                                "⚬ Dead letter entry exhausted after {} retries: {} (type: {})",
                                entry.retry_count, entry.id, entry.operation_type
                            );
                        } else {
                            let remaining_ms =
                                entry.next_retry_timestamp - current_timestamp_millis();
                            debug!(
                                "⟲ Will retry dead letter entry: {} in {}ms (attempt {}/{})",
                                entry.id,
                                remaining_ms,
                                entry.retry_count + 1,
                                entry.max_retries
                            );
                        }
                    }
                }
            } else {
                warn!(
                    "⚬ No retry handler found for operation type: {} (entry: {})",
                    entry.operation_type, entry.id
                );
            }
        }

        // Remove successfully processed entries (in reverse order to maintain indices)
        for &index in processed_indices.iter().rev() {
            entries.remove(index);
        }

        Ok(())
    }

    /// Clean up old entries that have exceeded the maximum age
    async fn cleanup_old_entries(&self) -> RavenResult<()> {
        let mut entries = self.entries.lock().await;
        let now = current_timestamp_millis();

        let initial_count = entries.len();
        entries.retain(|entry| {
            let age = now - entry.dead_letter_timestamp;
            if age > self.config.max_age_ms {
                warn!(
                    "⚬ Removing expired dead letter entry: {} (age: {}ms, type: {})",
                    entry.id, age, entry.operation_type
                );
                false
            } else {
                true
            }
        });

        let removed_count = initial_count - entries.len();
        if removed_count > 0 {
            info!("⚬ Cleaned up {} expired dead letter entries", removed_count);
        }

        Ok(())
    }

    /// Update queue statistics
    async fn update_statistics(&self) {
        let entries = self.entries.lock().await;
        let mut stats = self.stats.write().await;

        stats.total_entries = entries.len();
        stats.ready_for_retry = entries.iter().filter(|e| e.is_ready_for_retry()).count();
        stats.exhausted_entries = entries.iter().filter(|e| e.is_exhausted()).count();

        if let (Some(oldest), Some(newest)) = (entries.front(), entries.back()) {
            stats.oldest_entry_age_ms = oldest.age_ms();
            stats.newest_entry_age_ms = newest.age_ms();
        } else {
            stats.oldest_entry_age_ms = 0;
            stats.newest_entry_age_ms = 0;
        }

        // Update entries by operation type
        stats.entries_by_operation_type.clear();
        for entry in entries.iter() {
            *stats
                .entries_by_operation_type
                .entry(entry.operation_type.clone())
                .or_insert(0) += 1;
        }
    }

    /// Get current queue statistics
    pub async fn get_statistics(&self) -> DeadLetterQueueStats {
        self.stats.read().await.clone()
    }

    /// Get all entries (for debugging/monitoring)
    pub async fn get_all_entries(&self) -> Vec<DeadLetterEntry> {
        self.entries.lock().await.iter().cloned().collect()
    }

    /// Get entries by operation type
    pub async fn get_entries_by_type(&self, operation_type: &str) -> Vec<DeadLetterEntry> {
        self.entries
            .lock()
            .await
            .iter()
            .filter(|e| e.operation_type == operation_type)
            .cloned()
            .collect()
    }

    /// Manually retry a specific entry by ID
    pub async fn retry_entry_by_id(&self, entry_id: &str) -> RavenResult<()> {
        let mut entries = self.entries.lock().await;
        let handlers = self.retry_handlers.read().await;

        if let Some(entry) = entries.iter_mut().find(|e| e.id == entry_id) {
            if let Some(handler) = handlers.get(&entry.operation_type) {
                match handler.retry_operation(entry).await {
                    Ok(()) => {
                        // Remove the successfully processed entry
                        entries.retain(|e| e.id != entry_id);
                        info!("✓ Manually retried dead letter entry: {}", entry_id);
                        Ok(())
                    }
                    Err(e) => {
                        entry.prepare_for_retry(e.to_string());
                        crate::raven_bail!(crate::raven_error!(
                            dead_letter_processing,
                            format!("Manual retry failed for entry {entry_id}: {e}")
                        ));
                    }
                }
            } else {
                crate::raven_bail!(crate::raven_error!(
                    dead_letter_processing,
                    format!(
                        "No retry handler found for entry {} (type: {})",
                        entry_id, entry.operation_type
                    )
                ));
            }
        } else {
            crate::raven_bail!(crate::raven_error!(
                dead_letter_processing,
                format!("Entry not found: {entry_id}")
            ));
        }
    }

    /// Clear all entries (for testing/emergency situations)
    pub async fn clear_all_entries(&self) -> usize {
        let mut entries = self.entries.lock().await;
        let count = entries.len();
        entries.clear();

        warn!("⚬ Cleared all {} dead letter entries", count);
        count
    }

    /// Persist queue to disk (if configured)
    pub async fn persist_to_disk(&self) -> RavenResult<()> {
        if !self.config.persist_to_disk {
            return Ok(());
        }

        let Some(ref file_path) = self.config.persistence_file else {
            crate::raven_bail!(crate::raven_error!(
                configuration,
                "Persistence enabled but no file path configured",
            ));
        };

        let entries = self.entries.lock().await;
        let serialized = serde_json::to_string_pretty(&*entries)
            .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;

        tokio::fs::write(file_path, serialized).await.map_err(|e| {
            crate::raven_error!(
                internal,
                format!("Failed to persist dead letter queue: {e}")
            )
        })?;

        debug!("⚬ Persisted {} dead letter entries to disk", entries.len());
        Ok(())
    }

    /// Load queue from disk (if configured)
    pub async fn load_from_disk(&self) -> RavenResult<()> {
        if !self.config.persist_to_disk {
            return Ok(());
        }

        let Some(ref file_path) = self.config.persistence_file else {
            return Ok(());
        };

        if !tokio::fs::try_exists(file_path).await.unwrap_or(false) {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(file_path).await.map_err(|e| {
            crate::raven_error!(internal, format!("Failed to load dead letter queue: {e}"))
        })?;

        let loaded_entries: VecDeque<DeadLetterEntry> = serde_json::from_str(&content)
            .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;

        let mut entries = self.entries.lock().await;
        *entries = loaded_entries;

        info!("⚬ Loaded {} dead letter entries from disk", entries.len());
        Ok(())
    }
}

impl Clone for DeadLetterQueue {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            entries: Arc::clone(&self.entries),
            stats: Arc::clone(&self.stats),
            retry_handlers: Arc::clone(&self.retry_handlers),
            processing_active: Arc::clone(&self.processing_active),
        }
    }
}
