use crate::types::{CandleData, FundingRateData};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use super::{
    ChannelMessage, LowFrequencyConfig, LowFrequencyData, LowFrequencyMetrics, LowFrequencyStorage,
};

/// Low-frequency data ingestion handler with async channel-based processing
/// Designed for non-critical data like candles and funding rates
pub struct LowFrequencyHandler {
    config: LowFrequencyConfig,
    storage: Arc<LowFrequencyStorage>,
    sender: mpsc::UnboundedSender<ChannelMessage>,
    receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<ChannelMessage>>>>,
    metrics: Arc<LowFrequencyMetrics>,
    processing_active: Arc<RwLock<bool>>,
}

impl LowFrequencyHandler {
    /// Create a new LowFrequencyHandler with default configuration
    pub fn new() -> Self {
        Self::with_config(LowFrequencyConfig::default())
    }

    /// Create a new LowFrequencyHandler with custom configuration
    pub fn with_config(config: LowFrequencyConfig) -> Self {
        info!("◦ Low frequency ravens are preparing for flight...");

        let (sender, receiver) = mpsc::unbounded_channel();
        let storage = Arc::new(LowFrequencyStorage::new(config.max_items_per_symbol));

        Self {
            config,
            storage,
            sender,
            receiver: Arc::new(RwLock::new(Some(receiver))),
            metrics: Arc::new(LowFrequencyMetrics::default()),
            processing_active: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the async processing loop
    pub async fn start(&self) -> Result<()> {
        let mut processing_active = self.processing_active.write().await;
        if *processing_active {
            return Ok(()); // Already started
        }

        let mut receiver_guard = self.receiver.write().await;
        let receiver = receiver_guard
            .take()
            .context("Low frequency handler already started or receiver consumed")?;
        drop(receiver_guard);

        *processing_active = true;
        drop(processing_active);

        // Start the processing loop
        let handler = self.clone();
        tokio::spawn(async move {
            handler.processing_loop(receiver).await;
        });

        info!("✓ Low frequency handler processing started");
        Ok(())
    }

    /// Stop the async processing loop
    pub async fn stop(&self) -> Result<()> {
        let mut processing_active = self.processing_active.write().await;
        *processing_active = false;
        info!("■ Low frequency handler processing stopped");
        Ok(())
    }

    /// Ingest candle data via async channel
    pub async fn ingest_candle(&self, data: &CandleData) -> Result<()> {
        let start = Instant::now();

        // Validate candle data
        self.validate_candle_data(data)?;

        // Create channel message
        let message = ChannelMessage {
            data: LowFrequencyData::Candle(data.clone()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            retry_count: 0,
        };

        // Send to async processing channel
        self.sender
            .send(message)
            .context("Failed to send candle data to processing channel")?;

        self.metrics
            .channel_queue_size
            .fetch_add(1, Ordering::Relaxed);

        debug!(
            "◦ Candle data queued for {} {} in {:?}",
            data.symbol,
            data.interval,
            start.elapsed()
        );

        Ok(())
    }

    /// Ingest funding rate data via async channel
    pub async fn ingest_funding_rate(&self, data: &FundingRateData) -> Result<()> {
        let start = Instant::now();

        // Validate funding rate data
        self.validate_funding_rate_data(data)?;

        // Create channel message
        let message = ChannelMessage {
            data: LowFrequencyData::FundingRate(data.clone()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            retry_count: 0,
        };

        // Send to async processing channel
        self.sender
            .send(message)
            .context("Failed to send funding rate data to processing channel")?;

        self.metrics
            .channel_queue_size
            .fetch_add(1, Ordering::Relaxed);

        debug!(
            "$ Funding rate data queued for {} in {:?}",
            data.symbol,
            start.elapsed()
        );

        Ok(())
    }

    /// Get recent candles for a symbol and interval
    pub async fn get_candles(&self, symbol: &str, interval: &str) -> Option<Vec<CandleData>> {
        self.storage.get_candles(symbol, interval)
    }

    /// Get recent funding rates for a symbol
    pub async fn get_funding_rates(&self, symbol: &str) -> Option<Vec<FundingRateData>> {
        self.storage.get_funding_rates(symbol)
    }

    /// Get all active symbols
    pub async fn get_active_symbols(&self) -> Vec<String> {
        self.storage.get_all_symbols()
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> HashMap<String, u64> {
        let mut metrics = HashMap::new();

        metrics.insert(
            "candles_processed".to_string(),
            self.metrics.candles_processed.load(Ordering::Relaxed),
        );
        metrics.insert(
            "funding_rates_processed".to_string(),
            self.metrics.funding_rates_processed.load(Ordering::Relaxed),
        );
        metrics.insert(
            "total_failed".to_string(),
            self.metrics.total_failed.load(Ordering::Relaxed),
        );
        metrics.insert(
            "channel_queue_size".to_string(),
            self.metrics.channel_queue_size.load(Ordering::Relaxed),
        );
        metrics.insert(
            "last_processed_time".to_string(),
            self.metrics.last_processed_time.load(Ordering::Relaxed),
        );

        metrics
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();

        stats.insert(
            "candle_symbols".to_string(),
            self.storage.get_candle_symbols().len(),
        );
        stats.insert(
            "funding_rate_symbols".to_string(),
            self.storage.get_funding_rate_symbols().len(),
        );
        stats.insert(
            "total_candle_entries".to_string(),
            self.storage.candles.len(),
        );
        stats.insert(
            "total_funding_rate_entries".to_string(),
            self.storage.funding_rates.len(),
        );

        stats
    }

    // Private methods

    /// Main processing loop for async channel messages
    async fn processing_loop(&self, mut receiver: mpsc::UnboundedReceiver<ChannelMessage>) {
        let mut interval = interval(self.config.processing_interval);
        let mut batch = Vec::with_capacity(self.config.processing_batch_size);

        info!("◦ Low frequency processing loop started");

        while *self.processing_active.read().await {
            interval.tick().await;

            // Collect batch of messages
            while batch.len() < self.config.processing_batch_size {
                match receiver.try_recv() {
                    Ok(message) => {
                        batch.push(message);
                        self.metrics
                            .channel_queue_size
                            .fetch_sub(1, Ordering::Relaxed);
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        warn!("◦ Low frequency processing channel disconnected");
                        return;
                    }
                }
            }

            // Process batch if not empty
            if !batch.is_empty() {
                self.process_batch(&mut batch).await;
                batch.clear();
            }
        }

        // Process remaining messages before shutdown
        while let Ok(message) = receiver.try_recv() {
            batch.push(message);
        }
        if !batch.is_empty() {
            self.process_batch(&mut batch).await;
        }

        info!("◦ Low frequency processing loop stopped");
    }

    /// Process a batch of channel messages
    async fn process_batch(&self, batch: &mut [ChannelMessage]) {
        let start = Instant::now();
        let mut processed_count = 0;
        let mut failed_count = 0;

        for message in batch.iter() {
            match self.process_message(message).await {
                Ok(_) => {
                    processed_count += 1;
                    match &message.data {
                        LowFrequencyData::Candle(_) => {
                            self.metrics
                                .candles_processed
                                .fetch_add(1, Ordering::Relaxed);
                        }
                        LowFrequencyData::FundingRate(_) => {
                            self.metrics
                                .funding_rates_processed
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Err(e) => {
                    failed_count += 1;
                    self.metrics.total_failed.fetch_add(1, Ordering::Relaxed);
                    error!("✗ Failed to process low frequency message: {}", e);
                }
            }
        }

        if processed_count > 0 || failed_count > 0 {
            debug!(
                "◦ Processed batch: {} successful, {} failed in {:?}",
                processed_count,
                failed_count,
                start.elapsed()
            );
        }

        // Update last processed time
        self.metrics.last_processed_time.store(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Ordering::Relaxed,
        );
    }

    /// Process a single channel message
    async fn process_message(&self, message: &ChannelMessage) -> Result<()> {
        match &message.data {
            LowFrequencyData::Candle(candle) => {
                self.storage.add_candle(candle);
                debug!(
                    "◦ Processed candle for {} {}",
                    candle.symbol, candle.interval
                );
            }
            LowFrequencyData::FundingRate(funding) => {
                self.storage.add_funding_rate(funding);
                debug!("$ Processed funding rate for {}", funding.symbol);
            }
        }
        Ok(())
    }

    /// Validate candle data
    fn validate_candle_data(&self, data: &CandleData) -> Result<()> {
        if data.symbol.is_empty() {
            return Err(anyhow::anyhow!("Candle symbol cannot be empty"));
        }

        if data.interval.is_empty() {
            return Err(anyhow::anyhow!("Candle interval cannot be empty"));
        }

        if data.timestamp <= 0 {
            return Err(anyhow::anyhow!("Candle timestamp must be positive"));
        }

        if data.open <= 0.0 || data.high <= 0.0 || data.low <= 0.0 || data.close <= 0.0 {
            return Err(anyhow::anyhow!("Candle OHLC prices must be positive"));
        }

        if data.volume < 0.0 {
            return Err(anyhow::anyhow!("Candle volume cannot be negative"));
        }

        if data.high < data.low {
            return Err(anyhow::anyhow!(
                "Candle high price cannot be less than low price"
            ));
        }

        if data.high < data.open || data.high < data.close {
            return Err(anyhow::anyhow!(
                "Candle high price must be >= open and close prices"
            ));
        }

        if data.low > data.open || data.low > data.close {
            return Err(anyhow::anyhow!(
                "Candle low price must be <= open and close prices"
            ));
        }

        Ok(())
    }

    /// Validate funding rate data
    fn validate_funding_rate_data(&self, data: &FundingRateData) -> Result<()> {
        if data.symbol.is_empty() {
            return Err(anyhow::anyhow!("Funding rate symbol cannot be empty"));
        }

        if data.timestamp <= 0 {
            return Err(anyhow::anyhow!("Funding rate timestamp must be positive"));
        }

        if data.next_funding_time <= data.timestamp {
            return Err(anyhow::anyhow!(
                "Next funding time must be after current timestamp"
            ));
        }

        // Funding rates are typically between -1% and 1% (expressed as decimals)
        if data.rate.abs() > 0.01 {
            warn!(
                "Funding rate {} for {} seems unusually high (>1%)",
                data.rate, data.symbol
            );
        }

        Ok(())
    }
}

impl Clone for LowFrequencyHandler {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            storage: Arc::clone(&self.storage),
            sender: self.sender.clone(),
            receiver: Arc::clone(&self.receiver),
            metrics: Arc::clone(&self.metrics),
            processing_active: Arc::clone(&self.processing_active),
        }
    }
}

impl Default for LowFrequencyHandler {
    fn default() -> Self {
        Self::new()
    }
}
