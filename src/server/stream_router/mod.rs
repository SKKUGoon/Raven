pub mod router;
pub mod topic;

pub use crate::proto::{DataType, MarketDataMessage};

use crate::common::current_timestamp_millis;
use crate::common::error::RavenResult;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Data types that clients can subscribe to
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubscriptionDataType {
    Orderbook,
    Trades,
    Candles1M,
    Candles5M,
    Candles1H,
    FundingRates,
    WalletUpdates,
}

impl From<DataType> for SubscriptionDataType {
    fn from(data_type: DataType) -> Self {
        match data_type {
            DataType::Orderbook => SubscriptionDataType::Orderbook,
            DataType::Trades => SubscriptionDataType::Trades,
            DataType::Candles1m => SubscriptionDataType::Candles1M,
            DataType::Candles5m => SubscriptionDataType::Candles5M,
            DataType::Candles1h => SubscriptionDataType::Candles1H,
            DataType::FundingRates => SubscriptionDataType::FundingRates,
            DataType::WalletUpdates => SubscriptionDataType::WalletUpdates,
        }
    }
}

impl From<SubscriptionDataType> for DataType {
    fn from(data_type: SubscriptionDataType) -> Self {
        match data_type {
            SubscriptionDataType::Orderbook => DataType::Orderbook,
            SubscriptionDataType::Trades => DataType::Trades,
            SubscriptionDataType::Candles1M => DataType::Candles1m,
            SubscriptionDataType::Candles5M => DataType::Candles5m,
            SubscriptionDataType::Candles1H => DataType::Candles1h,
            SubscriptionDataType::FundingRates => DataType::FundingRates,
            SubscriptionDataType::WalletUpdates => DataType::WalletUpdates,
        }
    }
}

/// Client subscription information with filtering and session tracking
#[derive(Debug, Clone)]
pub struct ClientSubscription {
    pub client_id: String,
    pub session_id: String,
    pub symbols: HashSet<String>,
    pub exchanges: HashSet<String>,
    pub data_types: HashSet<SubscriptionDataType>,
    pub filters: HashMap<String, String>,
    pub sender: mpsc::UnboundedSender<MarketDataMessage>,
    pub last_heartbeat: Arc<AtomicI64>,
    pub created_at: i64,
}

impl ClientSubscription {
    /// Create a new client subscription
    pub fn new(
        client_id: String,
        symbols: HashSet<String>,
        exchanges: HashSet<String>,
        data_types: HashSet<SubscriptionDataType>,
        filters: HashMap<String, String>,
        sender: mpsc::UnboundedSender<MarketDataMessage>,
    ) -> Self {
        let now = current_timestamp_millis();

        Self {
            client_id,
            session_id: Uuid::new_v4().to_string(),
            symbols,
            exchanges,
            data_types,
            filters,
            sender,
            last_heartbeat: Arc::new(AtomicI64::new(now)),
            created_at: now,
        }
    }

    /// Update the last heartbeat timestamp
    pub fn update_heartbeat(&self) {
        let now = current_timestamp_millis();
        self.last_heartbeat.store(now, Ordering::Relaxed);
    }

    /// Check if the client is still alive based on heartbeat
    pub fn is_alive(&self, timeout_ms: i64) -> bool {
        let now = current_timestamp_millis();
        let last_heartbeat = self.last_heartbeat.load(Ordering::Relaxed);
        (now - last_heartbeat) < timeout_ms
    }

    /// Check if this subscription matches the given symbol and data type
    pub fn matches(&self, symbol: &str, data_type: &SubscriptionDataType, exchange: &str) -> bool {
        (self.symbols.is_empty() || self.symbols.contains(symbol))
            && (self.data_types.is_empty() || self.data_types.contains(data_type))
            && (self.exchanges.is_empty() || self.exchanges.contains(exchange))
    }

    /// Send a message to the client
    pub fn send_message(&self, message: MarketDataMessage) -> RavenResult<()> {
        self.sender.send(message).map_err(|_| {
            crate::raven_error!(
                stream_error,
                format!("Failed to send message to client {}", self.client_id)
            )
        })
    }

    /// Check if the sender channel is closed
    pub fn is_sender_closed(&self) -> bool {
        self.sender.is_closed()
    }

    /// Generate topic keys for this subscription
    pub fn get_topic_keys(&self) -> Vec<String> {
        let mut topics = Vec::new();

        if self.symbols.is_empty() && self.data_types.is_empty() {
            // Subscribe to all
            topics.push("*".to_string());
        } else if self.symbols.is_empty() {
            // Subscribe to all symbols for specific data types
            for data_type in &self.data_types {
                topics.push(format!("*:{data_type:?}"));
            }
        } else if self.data_types.is_empty() {
            // Subscribe to all data types for specific symbols
            for symbol in &self.symbols {
                topics.push(format!("{symbol}:*"));
            }
        } else {
            // Subscribe to specific symbol-datatype combinations
            for symbol in &self.symbols {
                for data_type in &self.data_types {
                    topics.push(format!("{symbol}:{data_type:?}"));
                }
            }
        }

        topics
    }
}

/// Subscription persistence data for reconnection support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSubscription {
    pub client_id: String,
    pub symbols: HashSet<String>,
    pub exchanges: HashSet<String>,
    pub data_types: HashSet<SubscriptionDataType>,
    pub filters: HashMap<String, String>,
    pub created_at: i64,
    pub last_seen: i64,
}
