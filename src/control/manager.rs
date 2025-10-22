// Collector Manager - Dynamic lifecycle management for data collectors
// "The Hand of the King orchestrates which ravens fly and which rest"

use crate::citadel::app::{spawn_orderbook_ingestor, spawn_trade_ingestor};
use crate::data_handlers::HighFrequencyHandler;
use crate::error::RavenResult;
use crate::exchanges::binance::app::futures::orderbook::initialize_binance_futures_orderbook;
use crate::exchanges::binance::app::futures::trade::initialize_binance_futures_trade;
use crate::exchanges::binance::app::spot::orderbook::initialize_binance_spot_orderbook;
use crate::exchanges::binance::app::spot::trade::initialize_binance_spot_trade;
use crate::exchanges::types::Exchange;
use crate::types::HighFrequencyStorage;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::task::JoinHandle;
use tracing::{info, warn};
use uuid::Uuid;

/// Handles for a single collection (orderbook + trade collectors)
#[derive(Debug)]
pub struct CollectorHandles {
    pub collection_id: String,
    pub exchange: Exchange,
    pub symbol: String,
    pub started_at: i64,
    pub orderbook_task: JoinHandle<()>,
    pub trade_task: JoinHandle<()>,
}

impl CollectorHandles {
    /// Stop all tasks for this collection
    pub fn stop(&self) {
        info!(
            collection_id = %self.collection_id,
            exchange = ?self.exchange,
            symbol = %self.symbol,
            "Stopping collection tasks"
        );
        self.orderbook_task.abort();
        self.trade_task.abort();
    }
}

/// Collection status information
#[derive(Debug, Clone)]
pub struct CollectionInfo {
    pub collection_id: String,
    pub exchange: Exchange,
    pub symbol: String,
    pub started_at: i64,
    pub status: String,
    pub data_types: Vec<String>,
}

/// Manager for dynamic collector lifecycle
pub struct CollectorManager {
    /// Active collections: (exchange, symbol) -> CollectorHandles
    active_collections: DashMap<(Exchange, String), CollectorHandles>,
    /// High frequency handler for data processing
    hf_handler: Arc<HighFrequencyHandler>,
    /// Citadel for data processing
    citadel: Arc<crate::citadel::Citadel>,
}

impl CollectorManager {
    /// Create a new CollectorManager
    pub fn new(
        hf_storage: Arc<HighFrequencyStorage>,
        citadel: Arc<crate::citadel::Citadel>,
    ) -> Self {
        let hf_handler = Arc::new(HighFrequencyHandler::with_storage(Arc::clone(&hf_storage)));
        Self {
            active_collections: DashMap::new(),
            hf_handler,
            citadel,
        }
    }

    /// Start collecting data for a specific exchange-symbol pair
    pub async fn start_collection(
        &self,
        exchange: Exchange,
        symbol: String,
    ) -> RavenResult<String> {
        let key = (exchange.clone(), symbol.clone());

        // Check if already collecting
        if self.active_collections.contains_key(&key) {
            let existing = self.active_collections.get(&key).unwrap();
            info!(
                collection_id = %existing.collection_id,
                exchange = ?exchange,
                symbol = %symbol,
                "Collection already active"
            );
            return Ok(existing.collection_id.clone());
        }

        let collection_id = Uuid::new_v4().to_string();
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        info!(
            collection_id = %collection_id,
            exchange = ?exchange,
            symbol = %symbol,
            "Starting new data collection"
        );

        // Initialize collectors based on exchange
        let (orderbook_receiver, trade_receiver) = match exchange {
            Exchange::BinanceFutures => {
                let (_orderbook_collector, orderbook_receiver) =
                    initialize_binance_futures_orderbook(symbol.clone()).await?;
                let (_trade_collector, trade_receiver) =
                    initialize_binance_futures_trade(symbol.clone()).await?;
                (orderbook_receiver, trade_receiver)
            }
            Exchange::BinanceSpot => {
                let (_orderbook_collector, orderbook_receiver) =
                    initialize_binance_spot_orderbook(symbol.clone()).await?;
                let (_trade_collector, trade_receiver) =
                    initialize_binance_spot_trade(symbol.clone()).await?;
                (orderbook_receiver, trade_receiver)
            }
        };

        // Spawn ingestion tasks
        let orderbook_task = spawn_orderbook_ingestor(
            orderbook_receiver,
            Arc::clone(&self.hf_handler),
            Arc::clone(&self.citadel),
        );

        let trade_task = spawn_trade_ingestor(
            trade_receiver,
            Arc::clone(&self.hf_handler),
            Arc::clone(&self.citadel),
        );

        let handles = CollectorHandles {
            collection_id: collection_id.clone(),
            exchange: exchange.clone(),
            symbol: symbol.clone(),
            started_at,
            orderbook_task,
            trade_task,
        };

        // Store the collection
        self.active_collections.insert(key, handles);

        info!(
            collection_id = %collection_id,
            exchange = ?exchange,
            symbol = %symbol,
            "Collection started successfully"
        );

        Ok(collection_id)
    }

    /// Stop collecting data for a specific exchange-symbol pair
    pub async fn stop_collection(&self, exchange: Exchange, symbol: String) -> RavenResult<()> {
        let key = (exchange.clone(), symbol.clone());

        if let Some((_, handles)) = self.active_collections.remove(&key) {
            handles.stop();
            info!(
                collection_id = %handles.collection_id,
                exchange = ?exchange,
                symbol = %symbol,
                "Collection stopped successfully"
            );
            Ok(())
        } else {
            warn!(
                exchange = ?exchange,
                symbol = %symbol,
                "Attempted to stop non-existent collection"
            );
            crate::raven_bail!(crate::raven_error!(
                client_not_found,
                format!("No active collection found for {exchange:?}:{symbol}")
            ));
        }
    }

    /// List all active collections
    pub fn list_collections(&self) -> Vec<CollectionInfo> {
        self.active_collections
            .iter()
            .map(|entry| {
                let handles = entry.value();
                CollectionInfo {
                    collection_id: handles.collection_id.clone(),
                    exchange: handles.exchange.clone(),
                    symbol: handles.symbol.clone(),
                    started_at: handles.started_at,
                    status: "running".to_string(),
                    data_types: vec!["orderbook".to_string(), "trades".to_string()],
                }
            })
            .collect()
    }

    /// Get the number of active collections
    pub fn active_count(&self) -> usize {
        self.active_collections.len()
    }

    /// Check if a specific collection is active
    pub fn is_collection_active(&self, exchange: Exchange, symbol: &str) -> bool {
        self.active_collections
            .contains_key(&(exchange, symbol.to_string()))
    }

    /// Stop all collections (for graceful shutdown)
    pub async fn stop_all_collections(&self) {
        info!("Stopping all active collections");

        for entry in self.active_collections.iter() {
            let handles = entry.value();
            handles.stop();
            info!(
                collection_id = %handles.collection_id,
                exchange = ?handles.exchange,
                symbol = %handles.symbol,
                "Stopped collection"
            );
        }

        self.active_collections.clear();
        info!("All collections stopped");
    }
}
