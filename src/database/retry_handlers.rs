// Database Retry Handlers for Dead Letter Queue
// "Even failed ravens deserve another chance to fly"

use crate::data_engine::storage::{CandleData, FundingRateData, OrderBookSnapshot, TradeSnapshot};
use crate::database::dead_letter_queue::{DeadLetterEntry, RetryHandler};
use crate::error::{EnhancedErrorContext, RavenResult};
use influxdb2::models::DataPoint;
use serde_json;
use std::sync::Arc;
use tracing::{debug, warn};

use super::influx_client::InfluxClient;

/// Retry handler for InfluxDB write operations
pub struct InfluxWriteRetryHandler {
    influx_client: Arc<InfluxClient>,
}

impl InfluxWriteRetryHandler {
    /// Create a new InfluxDB write retry handler
    pub fn new(influx_client: Arc<InfluxClient>) -> Self {
        Self { influx_client }
    }
}

#[async_trait::async_trait]
impl RetryHandler for InfluxWriteRetryHandler {
    async fn retry_operation(&self, entry: &DeadLetterEntry) -> RavenResult<()> {
        debug!("⟲ Retrying InfluxDB write operation: {}", entry.id);

        // Parse the operation data based on metadata
        let operation_subtype = entry.metadata.get("subtype").ok_or_else(|| {
            crate::raven_error!(dead_letter_processing, "Missing operation subtype")
        })?;

        match operation_subtype.as_str() {
            "orderbook_snapshot" => {
                let snapshot: OrderBookSnapshot = serde_json::from_str(&entry.data)
                    .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;
                crate::handle_database_error!(
                    self.influx_client.write_orderbook_snapshot(&snapshot).await,
                    "write_orderbook_snapshot",
                    "orderbook_snapshots"
                )
            }
            "trade_snapshot" => {
                let snapshot: TradeSnapshot = serde_json::from_str(&entry.data)
                    .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;
                crate::handle_database_error!(
                    self.influx_client.write_trade_snapshot(&snapshot).await,
                    "write_trade_snapshot",
                    "trade_snapshots"
                )
            }
            "candle" => {
                let candle: CandleData = serde_json::from_str(&entry.data)
                    .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;
                crate::handle_database_error!(
                    self.influx_client.write_candle(&candle).await,
                    "write_candle",
                    "candles"
                )
            }
            "funding_rate" => {
                let funding: FundingRateData = serde_json::from_str(&entry.data)
                    .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;
                crate::handle_database_error!(
                    self.influx_client.write_funding_rate(&funding).await,
                    "write_funding_rate",
                    "funding_rates"
                )
            }
            "wallet_update" => {
                // Parse wallet update data
                let wallet_data: WalletUpdateData = serde_json::from_str(&entry.data)
                    .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;

                let balances: Vec<(String, f64, f64)> = wallet_data
                    .balances
                    .into_iter()
                    .map(|b| (b.asset, b.available, b.locked))
                    .collect();

                crate::handle_database_error!(
                    self.influx_client
                        .write_wallet_update(&wallet_data.user_id, &balances, wallet_data.timestamp)
                        .await,
                    "write_wallet_update",
                    "wallet_updates"
                )
            }
            "batch_write" => {
                // For batch writes, we store a simplified representation
                // In a real implementation, you'd need a more sophisticated approach
                // to serialize/deserialize DataPoint objects
                crate::raven_bail!(crate::raven_error!(
                    dead_letter_processing,
                    "Batch write retry not implemented - DataPoint serialization not supported"
                        .to_string(),
                ));
            }
            _ => {
                crate::raven_bail!(crate::raven_error!(
                    dead_letter_processing,
                    format!("Unknown operation subtype: {operation_subtype}")
                ));
            }
        }
    }

    fn operation_type(&self) -> &str {
        "influx_write"
    }
}

/// Wallet update data structure for dead letter queue
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WalletUpdateData {
    user_id: String,
    timestamp: i64,
    balances: Vec<BalanceData>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BalanceData {
    asset: String,
    available: f64,
    locked: f64,
}

/// Helper functions for creating dead letter entries for database operations
pub struct DatabaseDeadLetterHelper;

impl DatabaseDeadLetterHelper {
    /// Create a dead letter entry for orderbook snapshot write failure
    pub fn create_orderbook_entry(
        snapshot: &OrderBookSnapshot,
        error_message: String,
    ) -> RavenResult<DeadLetterEntry> {
        let data = serde_json::to_string(snapshot)
            .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;

        let entry = crate::database::dead_letter_queue::DeadLetterEntry::new(
            "influx_write".to_string(),
            data,
            error_message,
            3, // max retries
        )
        .with_metadata("subtype", "orderbook_snapshot")
        .with_metadata("symbol", &snapshot.symbol)
        .with_metadata("timestamp", snapshot.timestamp.to_string());

        Ok(entry)
    }

    /// Create a dead letter entry for trade snapshot write failure
    pub fn create_trade_entry(
        snapshot: &TradeSnapshot,
        error_message: String,
    ) -> RavenResult<DeadLetterEntry> {
        let data = serde_json::to_string(snapshot)
            .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;

        let entry = crate::database::dead_letter_queue::DeadLetterEntry::new(
            "influx_write".to_string(),
            data,
            error_message,
            3, // max retries
        )
        .with_metadata("subtype", "trade_snapshot")
        .with_metadata("symbol", &snapshot.symbol)
        .with_metadata("timestamp", snapshot.timestamp.to_string());

        Ok(entry)
    }

    /// Create a dead letter entry for candle write failure
    pub fn create_candle_entry(
        candle: &CandleData,
        error_message: String,
    ) -> RavenResult<DeadLetterEntry> {
        let data = serde_json::to_string(candle)
            .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;

        let entry = crate::database::dead_letter_queue::DeadLetterEntry::new(
            "influx_write".to_string(),
            data,
            error_message,
            3, // max retries
        )
        .with_metadata("subtype", "candle")
        .with_metadata("symbol", &candle.symbol)
        .with_metadata("interval", &candle.interval)
        .with_metadata("timestamp", candle.timestamp.to_string());

        Ok(entry)
    }

    /// Create a dead letter entry for funding rate write failure
    pub fn create_funding_rate_entry(
        funding: &FundingRateData,
        error_message: String,
    ) -> RavenResult<DeadLetterEntry> {
        let data = serde_json::to_string(funding)
            .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;

        let entry = crate::database::dead_letter_queue::DeadLetterEntry::new(
            "influx_write".to_string(),
            data,
            error_message,
            3, // max retries
        )
        .with_metadata("subtype", "funding_rate")
        .with_metadata("symbol", &funding.symbol)
        .with_metadata("timestamp", funding.timestamp.to_string());

        Ok(entry)
    }

    /// Create a dead letter entry for wallet update write failure
    pub fn create_wallet_update_entry(
        user_id: &str,
        balances: &[(String, f64, f64)],
        timestamp: i64,
        error_message: String,
    ) -> RavenResult<DeadLetterEntry> {
        let wallet_data = WalletUpdateData {
            user_id: user_id.to_string(),
            timestamp,
            balances: balances
                .iter()
                .map(|(asset, available, locked)| BalanceData {
                    asset: asset.clone(),
                    available: *available,
                    locked: *locked,
                })
                .collect(),
        };

        let data = serde_json::to_string(&wallet_data)
            .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;

        let entry = crate::database::dead_letter_queue::DeadLetterEntry::new(
            "influx_write".to_string(),
            data,
            error_message,
            3, // max retries
        )
        .with_metadata("subtype", "wallet_update")
        .with_metadata("user_id", user_id)
        .with_metadata("timestamp", timestamp.to_string());

        Ok(entry)
    }

    /// Create a dead letter entry for batch write failure
    pub fn create_batch_write_entry(
        data_points: &[DataPoint],
        error_message: String,
    ) -> RavenResult<DeadLetterEntry> {
        // Serialize DataPoint is complex, so we'll store a simplified representation
        let data_point_info: Vec<String> = data_points
            .iter()
            .map(|dp| format!("{dp:?}")) // This is not ideal but DataPoint doesn't implement Serialize
            .collect();

        let data = serde_json::to_string(&data_point_info)
            .map_err(|e| crate::raven_error!(data_serialization, e.to_string()))?;

        let entry = crate::database::dead_letter_queue::DeadLetterEntry::new(
            "influx_write".to_string(),
            data,
            error_message,
            3, // max retries
        )
        .with_metadata("subtype", "batch_write")
        .with_metadata("data_point_count", data_points.len().to_string());

        Ok(entry)
    }
}

/// Enhanced InfluxDB client with dead letter queue integration
pub struct EnhancedInfluxClient {
    client: Arc<InfluxClient>,
    dead_letter_queue: Arc<crate::database::dead_letter_queue::DeadLetterQueue>,
}

impl EnhancedInfluxClient {
    /// Create a new enhanced InfluxDB client
    pub fn new(
        client: Arc<InfluxClient>,
        dead_letter_queue: Arc<crate::database::dead_letter_queue::DeadLetterQueue>,
    ) -> Self {
        Self {
            client,
            dead_letter_queue,
        }
    }

    /// Write orderbook snapshot with dead letter queue fallback
    pub async fn write_orderbook_snapshot_safe(
        &self,
        snapshot: &OrderBookSnapshot,
    ) -> RavenResult<()> {
        match self.client.write_orderbook_snapshot(snapshot).await {
            Ok(()) => Ok(()),
            Err(e) => {
                warn!(
                    "⚬ Orderbook write failed, adding to dead letter queue: {}",
                    e
                );
                let raven_error = crate::raven_error!(database_write, e.to_string());
                let entry = DatabaseDeadLetterHelper::create_orderbook_entry(
                    snapshot,
                    raven_error.to_string(),
                )?;
                self.dead_letter_queue.add_entry(entry).await?;
                Err(raven_error)
            }
        }
    }

    /// Write trade snapshot with dead letter queue fallback
    pub async fn write_trade_snapshot_safe(&self, snapshot: &TradeSnapshot) -> RavenResult<()> {
        match self.client.write_trade_snapshot(snapshot).await {
            Ok(()) => Ok(()),
            Err(e) => {
                warn!("⚬ Trade write failed, adding to dead letter queue: {}", e);
                let raven_error = crate::raven_error!(database_write, e.to_string());
                let entry = DatabaseDeadLetterHelper::create_trade_entry(
                    snapshot,
                    raven_error.to_string(),
                )?;
                self.dead_letter_queue.add_entry(entry).await?;
                Err(raven_error)
            }
        }
    }

    /// Write candle with dead letter queue fallback
    pub async fn write_candle_safe(&self, candle: &CandleData) -> RavenResult<()> {
        match self.client.write_candle(candle).await {
            Ok(()) => Ok(()),
            Err(e) => {
                warn!("⚬ Candle write failed, adding to dead letter queue: {}", e);
                let raven_error = crate::raven_error!(database_write, e.to_string());
                let entry =
                    DatabaseDeadLetterHelper::create_candle_entry(candle, raven_error.to_string())?;
                self.dead_letter_queue.add_entry(entry).await?;
                Err(raven_error)
            }
        }
    }

    /// Write funding rate with dead letter queue fallback
    pub async fn write_funding_rate_safe(&self, funding: &FundingRateData) -> RavenResult<()> {
        match self.client.write_funding_rate(funding).await {
            Ok(()) => Ok(()),
            Err(e) => {
                warn!(
                    "⚬ Funding rate write failed, adding to dead letter queue: {}",
                    e
                );
                let raven_error = crate::raven_error!(database_write, e.to_string());
                let entry = DatabaseDeadLetterHelper::create_funding_rate_entry(
                    funding,
                    raven_error.to_string(),
                )?;
                self.dead_letter_queue.add_entry(entry).await?;
                Err(raven_error)
            }
        }
    }

    /// Write wallet update with dead letter queue fallback
    pub async fn write_wallet_update_safe(
        &self,
        user_id: &str,
        balances: &[(String, f64, f64)],
        timestamp: i64,
    ) -> RavenResult<()> {
        match self
            .client
            .write_wallet_update(user_id, balances, timestamp)
            .await
        {
            Ok(()) => Ok(()),
            Err(e) => {
                warn!(
                    "⚬ Wallet update write failed, adding to dead letter queue: {}",
                    e
                );
                let raven_error = crate::raven_error!(database_write, e.to_string());
                let entry = DatabaseDeadLetterHelper::create_wallet_update_entry(
                    user_id,
                    balances,
                    timestamp,
                    raven_error.to_string(),
                )?;
                self.dead_letter_queue.add_entry(entry).await?;
                Err(raven_error)
            }
        }
    }

    /// Write batch with dead letter queue fallback
    pub async fn write_batch_safe(&self, data_points: Vec<DataPoint>) -> RavenResult<()> {
        match self.client.write_batch(data_points.clone()).await {
            Ok(()) => Ok(()),
            Err(e) => {
                warn!("⚬ Batch write failed, adding to dead letter queue: {}", e);
                let raven_error = crate::raven_error!(database_write, e.to_string());
                let entry = DatabaseDeadLetterHelper::create_batch_write_entry(
                    &data_points,
                    raven_error.to_string(),
                )?;
                self.dead_letter_queue.add_entry(entry).await?;
                Err(raven_error)
            }
        }
    }

    /// Get the underlying InfluxDB client
    pub fn client(&self) -> &Arc<InfluxClient> {
        &self.client
    }

    /// Get the dead letter queue
    pub fn dead_letter_queue(&self) -> &Arc<crate::database::dead_letter_queue::DeadLetterQueue> {
        &self.dead_letter_queue
    }
}
