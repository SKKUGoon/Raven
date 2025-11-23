// Stream Router - The Data Distribution System
// "The central switchboard for routing real-time data streams to clients"

// Note: types module not needed for this implementation
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Re-export protobuf types for convenience
use crate::common::error::RavenResult;
pub use crate::proto::{DataType, MarketDataMessage};
use crate::common::current_timestamp_millis;

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
        data_types: HashSet<SubscriptionDataType>,
        filters: HashMap<String, String>,
        sender: mpsc::UnboundedSender<MarketDataMessage>,
    ) -> Self {
        let now = current_timestamp_millis();

        Self {
            client_id,
            session_id: Uuid::new_v4().to_string(),
            symbols,
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
    pub fn matches(&self, symbol: &str, data_type: &SubscriptionDataType) -> bool {
        (self.symbols.is_empty() || self.symbols.contains(symbol))
            && (self.data_types.is_empty() || self.data_types.contains(data_type))
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
    pub data_types: HashSet<SubscriptionDataType>,
    pub filters: HashMap<String, String>,
    pub created_at: i64,
    pub last_seen: i64,
}

/// Topic-based index for efficient data distribution
#[derive(Debug)]
pub struct TopicIndex {
    /// Map from topic to set of client IDs
    topic_to_clients: DashMap<String, HashSet<String>>,
    /// Map from client ID to set of topics
    client_to_topics: DashMap<String, HashSet<String>>,
}

impl TopicIndex {
    pub fn new() -> Self {
        Self {
            topic_to_clients: DashMap::new(),
            client_to_topics: DashMap::new(),
        }
    }

    /// Add a client subscription to topics
    pub fn add_subscription(&self, client_id: &str, topics: Vec<String>) {
        // Add client to each topic
        for topic in &topics {
            self.topic_to_clients
                .entry(topic.clone())
                .or_default()
                .insert(client_id.to_string());
        }

        // Add topics to client
        self.client_to_topics
            .entry(client_id.to_string())
            .or_default()
            .extend(topics);
    }

    /// Remove a client subscription from topics
    pub fn remove_subscription(&self, client_id: &str, topics: Vec<String>) {
        // Remove client from each topic
        for topic in &topics {
            if let Some(mut clients) = self.topic_to_clients.get_mut(topic) {
                clients.remove(client_id);
                if clients.is_empty() {
                    drop(clients);
                    self.topic_to_clients.remove(topic);
                }
            }
        }

        // Remove topics from client
        if let Some(mut client_topics) = self.client_to_topics.get_mut(client_id) {
            for topic in &topics {
                client_topics.remove(topic);
            }
            if client_topics.is_empty() {
                drop(client_topics);
                self.client_to_topics.remove(client_id);
            }
        }
    }

    /// Remove all subscriptions for a client
    pub fn remove_client(&self, client_id: &str) {
        if let Some((_, topics)) = self.client_to_topics.remove(client_id) {
            for topic in topics {
                if let Some(mut clients) = self.topic_to_clients.get_mut(&topic) {
                    clients.remove(client_id);
                    if clients.is_empty() {
                        drop(clients);
                        self.topic_to_clients.remove(&topic);
                    }
                }
            }
        }
    }

    /// Get all clients subscribed to a topic
    pub fn get_clients_for_topic(&self, topic: &str) -> Vec<String> {
        let mut clients = Vec::new();

        // Direct topic match
        if let Some(topic_clients) = self.topic_to_clients.get(topic) {
            clients.extend(topic_clients.iter().cloned());
        }

        // Wildcard matches
        if let Some(wildcard_clients) = self.topic_to_clients.get("*") {
            clients.extend(wildcard_clients.iter().cloned());
        }

        // Symbol wildcard (e.g., "BTCUSDT:*")
        if let Some(colon_pos) = topic.find(':') {
            let symbol = &topic[..colon_pos];
            let wildcard_topic = format!("{symbol}:*");
            if let Some(symbol_clients) = self.topic_to_clients.get(&wildcard_topic) {
                clients.extend(symbol_clients.iter().cloned());
            }
        }

        // Data type wildcard (e.g., "*:Orderbook")
        if let Some(colon_pos) = topic.find(':') {
            let data_type = &topic[colon_pos + 1..];
            let wildcard_topic = format!("*:{data_type}");
            if let Some(datatype_clients) = self.topic_to_clients.get(&wildcard_topic) {
                clients.extend(datatype_clients.iter().cloned());
            }
        }

        // Remove duplicates
        clients.sort();
        clients.dedup();
        clients
    }

    /// Get statistics about topic routing
    pub fn get_stats(&self) -> (usize, usize) {
        (self.topic_to_clients.len(), self.client_to_topics.len())
    }
}

impl Default for TopicIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// The Stream Router - Central data distribution system.
/// Tracks client subscriptions and routes data streams; connection admission and limits are
/// enforced separately by `server::connection::ConnectionManager`.
pub struct StreamRouter {
    /// Active client subscriptions
    subscriptions: DashMap<String, ClientSubscription>,
    /// Topic-based routing index for efficient distribution
    topic_index: TopicIndex,
    /// Persisted subscriptions for reconnection support
    persisted_subscriptions: DashMap<String, PersistedSubscription>,
    /// Heartbeat timeout in milliseconds
    heartbeat_timeout_ms: i64,
    /// Cleanup interval for dead clients
    cleanup_interval: Duration,
}
impl StreamRouter {
    /// Create a new stream router
    pub fn new() -> Self {
        Self {
            subscriptions: DashMap::new(),
            topic_index: TopicIndex::new(),
            persisted_subscriptions: DashMap::new(),
            heartbeat_timeout_ms: 30_000,              // 30 seconds
            cleanup_interval: Duration::from_secs(60), // 1 minute
        }
    }

    /// Create a new stream router with custom settings
    pub fn with_settings(heartbeat_timeout_ms: i64, cleanup_interval: Duration) -> Self {
        Self {
            subscriptions: DashMap::new(),
            topic_index: TopicIndex::new(),
            persisted_subscriptions: DashMap::new(),
            heartbeat_timeout_ms,
            cleanup_interval,
        }
    }

    /// Subscribe a client to market data streams
    pub fn subscribe(
        &self,
        client_id: String,
        symbols: Vec<String>,
        data_types: Vec<SubscriptionDataType>,
        filters: HashMap<String, String>,
        sender: mpsc::UnboundedSender<MarketDataMessage>,
    ) -> RavenResult<Vec<String>> {
        let symbols_set: HashSet<String> = symbols.into_iter().collect();
        let data_types_set: HashSet<SubscriptionDataType> = data_types.into_iter().collect();

        // Create or update subscription
        let subscription = if let Some(mut existing) = self.subscriptions.get_mut(&client_id) {
            // Update existing subscription
            existing.symbols.extend(symbols_set.clone());
            existing.data_types.extend(data_types_set.clone());
            existing.filters.extend(filters.clone());
            existing.update_heartbeat();
            existing.clone()
        } else {
            // Create new subscription
            let subscription = ClientSubscription::new(
                client_id.clone(),
                symbols_set.clone(),
                data_types_set.clone(),
                filters.clone(),
                sender,
            );
            self.subscriptions
                .insert(client_id.clone(), subscription.clone());
            subscription
        };

        // Get topic keys for routing
        let topics = subscription.get_topic_keys();

        // Add to topic index
        self.topic_index
            .add_subscription(&client_id, topics.clone());

        // Persist subscription for reconnection support
        self.persist_subscription(&subscription);

        info!(
            client_id = %client_id,
            session_id = %subscription.session_id,
            symbols = ?symbols_set,
            data_types = ?data_types_set,
            topics = ?topics,
            "Client subscribed to market data streams"
        );

        Ok(topics)
    }

    /// Unsubscribe a client from specific market data streams
    pub fn unsubscribe(
        &self,
        client_id: &str,
        symbols: Vec<String>,
        data_types: Vec<SubscriptionDataType>,
    ) -> RavenResult<Vec<String>> {
        let mut unsubscribed_topics = Vec::new();

        if let Some(mut subscription) = self.subscriptions.get_mut(client_id) {
            let symbols_set: HashSet<String> = symbols.into_iter().collect();
            let data_types_set: HashSet<SubscriptionDataType> = data_types.into_iter().collect();

            // Get topics before modification for removal from index
            let old_topics = subscription.get_topic_keys();

            // Remove specified symbols and data types
            if !symbols_set.is_empty() {
                for symbol in &symbols_set {
                    subscription.symbols.remove(symbol);
                }
            }

            if !data_types_set.is_empty() {
                for data_type in &data_types_set {
                    subscription.data_types.remove(data_type);
                }
            }

            // Get new topics after modification
            let new_topics = subscription.get_topic_keys();

            // Find topics to remove
            let topics_to_remove: Vec<String> = old_topics
                .into_iter()
                .filter(|topic| !new_topics.contains(topic))
                .collect();

            // Remove from topic index
            self.topic_index
                .remove_subscription(client_id, topics_to_remove.clone());
            unsubscribed_topics = topics_to_remove;

            // Update persistence
            self.persist_subscription(&subscription);

            info!(
                client_id = %client_id,
                unsubscribed_topics = ?unsubscribed_topics,
                "Client unsubscribed from market data streams"
            );
        } else {
            warn!(client_id = %client_id, "Attempted to unsubscribe non-existent client");
        }

        Ok(unsubscribed_topics)
    }

    /// Unsubscribe a client from all market data streams (keeps persistence for reconnection)
    pub fn unsubscribe_all(&self, client_id: &str) -> RavenResult<()> {
        if let Some((_, subscription)) = self.subscriptions.remove(client_id) {
            // Remove from topic index
            self.topic_index.remove_client(client_id);

            // Keep persistence for reconnection support
            // self.persisted_subscriptions.remove(client_id); // Don't remove

            info!(
                client_id = %client_id,
                session_id = %subscription.session_id,
                "Client unsubscribed from all market data streams (persistence kept)"
            );
        } else {
            warn!(client_id = %client_id, "Attempted to unsubscribe non-existent client");
        }

        Ok(())
    }

    /// Completely remove a client including persistence (for permanent cleanup)
    pub fn remove_client_completely(&self, client_id: &str) -> RavenResult<()> {
        if let Some((_, subscription)) = self.subscriptions.remove(client_id) {
            // Remove from topic index
            self.topic_index.remove_client(client_id);

            // Remove from persistence
            self.persisted_subscriptions.remove(client_id);

            info!(
                client_id = %client_id,
                session_id = %subscription.session_id,
                "Client completely removed including persistence"
            );
        } else {
            warn!(client_id = %client_id, "Attempted to remove non-existent client");
        }

        Ok(())
    }

    /// Update client heartbeat
    pub fn update_heartbeat(&self, client_id: &str) -> RavenResult<()> {
        if let Some(subscription) = self.subscriptions.get(client_id) {
            subscription.update_heartbeat();
            debug!(client_id = %client_id, "Updated client heartbeat");
        } else {
            warn!(client_id = %client_id, "Attempted to update heartbeat for non-existent client");
        }
        Ok(())
    }

    /// Distribute a message to all subscribed clients
    pub fn distribute_message(
        &self,
        symbol: &str,
        data_type: SubscriptionDataType,
        message: MarketDataMessage,
    ) -> RavenResult<usize> {
        let topic = format!("{symbol}:{data_type:?}");
        let client_ids = self.topic_index.get_clients_for_topic(&topic);
        let mut sent_count = 0;
        let mut failed_clients = Vec::new();

        for client_id in client_ids {
            if let Some(subscription) = self.subscriptions.get(&client_id) {
                if subscription.is_sender_closed() {
                    debug!(
                        client_id = %client_id,
                        "Skipping broadcast for closed client channel"
                    );
                    failed_clients.push(client_id);
                    continue;
                }

                if subscription.matches(symbol, &data_type) {
                    match subscription.send_message(message.clone()) {
                        Ok(_) => {
                            sent_count += 1;
                            debug!(
                                client_id = %client_id,
                                symbol = %symbol,
                                data_type = ?data_type,
                                "Message sent to client"
                            );
                        }
                        Err(e) => {
                            error!(
                                client_id = %client_id,
                                error = %e,
                                "Failed to send message to client"
                            );
                            failed_clients.push(client_id);
                        }
                    }
                }
            }
        }

        // Clean up failed clients
        for client_id in failed_clients {
            self.cleanup_dead_client(&client_id);
        }

        Ok(sent_count)
    }

    /// Determine whether any active subscribers exist for the given symbol/data type
    pub fn has_subscribers(&self, symbol: &str, data_type: SubscriptionDataType) -> bool {
        let topic = format!("{symbol}:{data_type:?}");
        !self.topic_index.get_clients_for_topic(&topic).is_empty()
    }

    /// Get subscription information for a client
    pub fn get_subscription(&self, client_id: &str) -> Option<ClientSubscription> {
        self.subscriptions.get(client_id).map(|sub| sub.clone())
    }

    /// Get all active client IDs
    pub fn get_active_clients(&self) -> Vec<String> {
        self.subscriptions
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get subscription statistics
    pub fn get_stats(&self) -> StreamStats {
        let active_clients = self.subscriptions.len();
        let persisted_subscriptions = self.persisted_subscriptions.len();
        let (topics, client_topics) = self.topic_index.get_stats();

        // Calculate total subscriptions across all clients
        let total_subscriptions = self
            .subscriptions
            .iter()
            .map(|entry| entry.value().symbols.len() * entry.value().data_types.len())
            .sum();

        StreamStats {
            active_clients,
            total_subscriptions,
            persisted_subscriptions,
            topics,
            client_topics,
        }
    }

    /// Start the cleanup task for dead clients
    pub async fn start_cleanup_task(self: Arc<Self>) {
        let mut interval = interval(self.cleanup_interval);

        loop {
            interval.tick().await;
            self.cleanup_dead_clients().await;
        }
    }

    /// Clean up dead clients based on heartbeat timeout
    pub async fn cleanup_dead_clients(&self) {
        let mut dead_clients = Vec::new();

        // Find dead clients
        for entry in self.subscriptions.iter() {
            let client_id = entry.key();
            let subscription = entry.value();

            if !subscription.is_alive(self.heartbeat_timeout_ms) || subscription.is_sender_closed()
            {
                dead_clients.push(client_id.clone());
            }
        }

        // Remove dead clients
        for client_id in dead_clients {
            self.cleanup_dead_client(&client_id);
        }
    }

    /// Clean up a specific dead client (keeps persistence for potential reconnection)
    fn cleanup_dead_client(&self, client_id: &str) {
        if let Some((_, subscription)) = self.subscriptions.remove(client_id) {
            // Remove from topic index
            self.topic_index.remove_client(client_id);

            // Keep persistence for potential reconnection
            // Only remove persistence after extended inactivity

            warn!(
                client_id = %client_id,
                session_id = %subscription.session_id,
                "Cleaned up dead client (persistence kept for reconnection)"
            );
        }
    }

    /// Persist subscription for reconnection support
    fn persist_subscription(&self, subscription: &ClientSubscription) {
        let persisted = PersistedSubscription {
            client_id: subscription.client_id.clone(),
            symbols: subscription.symbols.clone(),
            data_types: subscription.data_types.clone(),
            filters: subscription.filters.clone(),
            created_at: subscription.created_at,
            last_seen: subscription.last_heartbeat.load(Ordering::Relaxed),
        };

        self.persisted_subscriptions
            .insert(subscription.client_id.clone(), persisted);
    }

    /// Restore subscription from persistence (for reconnection)
    pub fn restore_subscription(
        &self,
        client_id: &str,
        sender: mpsc::UnboundedSender<MarketDataMessage>,
    ) -> RavenResult<Option<Vec<String>>> {
        if let Some(persisted) = self.persisted_subscriptions.get(client_id) {
            let symbols: Vec<String> = persisted.symbols.iter().cloned().collect();
            let data_types: Vec<SubscriptionDataType> =
                persisted.data_types.iter().cloned().collect();
            let filters = persisted.filters.clone();

            let topics =
                self.subscribe(client_id.to_string(), symbols, data_types, filters, sender)?;

            info!(
                client_id = %client_id,
                topics = ?topics,
                "Restored subscription from persistence"
            );

            Ok(Some(topics))
        } else {
            Ok(None)
        }
    }

    /// Check if a client has a persisted subscription
    pub fn has_persisted_subscription(&self, client_id: &str) -> bool {
        self.persisted_subscriptions.contains_key(client_id)
    }

    /// Remove persisted subscription
    pub fn remove_persisted_subscription(&self, client_id: &str) {
        self.persisted_subscriptions.remove(client_id);
    }
}

impl Default for StreamRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Stream statistics
#[derive(Debug, Clone)]
pub struct StreamStats {
    pub active_clients: usize,
    pub total_subscriptions: usize,
    pub persisted_subscriptions: usize,
    pub topics: usize,
    pub client_topics: usize,
}
