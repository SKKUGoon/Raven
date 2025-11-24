use crate::common::db::EnhancedInfluxClient;
use crate::common::error::RavenResult;
use crate::server::stream_router::{StreamRouter, SubscriptionDataType};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{debug, error, info};

use super::config::DataEngineConfig;
use super::metrics::DataEngineMetrics;
use super::storage::{OrderBookSnapshot, TradeSnapshot};
use super::{FundingRateData, OrderBookData, TradeData};

/// The Data Engine - Main data processing engine
pub struct DataEngine {
    config: DataEngineConfig,
    pub database_client: Arc<EnhancedInfluxClient>,
    stream_router: Arc<StreamRouter>,
    pub metrics: DataEngineMetrics,
}

impl DataEngine {
    /// Create a new DataEngine instance
    pub fn new(
        config: DataEngineConfig,
        database_client: Arc<EnhancedInfluxClient>,
        stream_router: Arc<StreamRouter>,
    ) -> Self {
        info!("▲ Initializing DataEngine with config: {:?}", config);

        Self {
            config,
            database_client,
            stream_router,
            metrics: DataEngineMetrics::default(),
        }
    }


    /// Persist order book data to database
    pub async fn persist_orderbook_data(
        &self,
        symbol: &str,
        data: OrderBookData,
    ) -> RavenResult<()> {
        self.metrics.total_ingested.fetch_add(1, Ordering::Relaxed);

        debug!("▲ Persisting orderbook data for symbol: {}", symbol);

        // Write to database
        match self.write_orderbook_data(&data).await {
            Ok(_) => {
                self.metrics.total_written.fetch_add(1, Ordering::Relaxed);
                debug!("✓ Successfully persisted orderbook data for {}", symbol);
            }
            Err(e) => {
                self.metrics.total_failed.fetch_add(1, Ordering::Relaxed);
                error!("✗ Failed to write orderbook data for {}: {}", symbol, e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Persist trade data to database
    pub async fn persist_trade_data(&self, symbol: &str, data: TradeData) -> RavenResult<()> {
        self.metrics.total_ingested.fetch_add(1, Ordering::Relaxed);

        debug!("▲ Persisting trade data for symbol: {}", symbol);

        // Write to database
        match self.write_trade_data(&data).await {
            Ok(_) => {
                self.metrics.total_written.fetch_add(1, Ordering::Relaxed);
                debug!("✓ Successfully persisted trade data for {}", symbol);
            }
            Err(e) => {
                self.metrics.total_failed.fetch_add(1, Ordering::Relaxed);
                error!("✗ Failed to write trade data for {}: {}", symbol, e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Persist funding rate data to database
    pub async fn persist_funding_rate_data(
        &self,
        symbol: &str,
        data: FundingRateData,
    ) -> RavenResult<()> {
        self.metrics.total_ingested.fetch_add(1, Ordering::Relaxed);

        debug!("▲ Persisting funding rate data for symbol: {}", symbol);

        // Write to database
        match self.write_funding_rate_data(&data).await {
            Ok(_) => {
                self.metrics.total_written.fetch_add(1, Ordering::Relaxed);
                debug!("✓ Successfully persisted funding rate data for {}", symbol);
            }
            Err(e) => {
                self.metrics.total_failed.fetch_add(1, Ordering::Relaxed);
                error!("✗ Failed to write funding rate data for {}: {}", symbol, e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Broadcast order book update to subscribers
    pub async fn broadcast_orderbook_update(
        &self,
        symbol: &str,
        data: &OrderBookData,
        _stream_router: &StreamRouter, // Deprecated argument, using self.stream_router
    ) -> RavenResult<()> {
        if !self
            .stream_router
            .has_subscribers(symbol, SubscriptionDataType::Orderbook)
        {
            return Ok(());
        }

        let snapshot = OrderBookSnapshot::from(data);
        let message = self.create_orderbook_message(&snapshot);

        self.stream_router.distribute_message(
            symbol,
            SubscriptionDataType::Orderbook,
            message,
        )?;

        Ok(())
    }

    /// Broadcast trade update to subscribers
    pub async fn broadcast_trade_update(
        &self,
        symbol: &str,
        data: &TradeData,
        _stream_router: &StreamRouter, // Deprecated argument
    ) -> RavenResult<()> {
        if !self
            .stream_router
            .has_subscribers(symbol, SubscriptionDataType::Trades)
        {
            return Ok(());
        }

        let snapshot = TradeSnapshot::from(data);
        let message = self.create_trade_message(&snapshot);

        self.stream_router.distribute_message(
            symbol,
            SubscriptionDataType::Trades,
            message,
        )?;

        Ok(())
    }

    /// Create protobuf message from orderbook snapshot
    fn create_orderbook_message(
        &self,
        snapshot: &OrderBookSnapshot,
    ) -> crate::proto::MarketDataMessage {
        use crate::proto::{MarketDataMessage, OrderBookSnapshot as ProtoOrderBook, PriceLevel};

        let proto_snapshot = ProtoOrderBook {
            symbol: snapshot.symbol.clone(),
            timestamp: snapshot.timestamp,
            bids: vec![PriceLevel {
                price: snapshot.best_bid_price,
                quantity: snapshot.best_bid_quantity,
            }],
            asks: vec![PriceLevel {
                price: snapshot.best_ask_price,
                quantity: snapshot.best_ask_quantity,
            }],
            sequence: snapshot.sequence as i64,
        };

        MarketDataMessage {
            data: Some(crate::proto::market_data_message::Data::Orderbook(
                proto_snapshot,
            )),
        }
    }

    /// Create protobuf message from trade snapshot
    fn create_trade_message(&self, snapshot: &TradeSnapshot) -> crate::proto::MarketDataMessage {
        use crate::proto::{MarketDataMessage, Trade as ProtoTrade};

        let proto_trade = ProtoTrade {
            symbol: snapshot.symbol.clone(),
            timestamp: snapshot.timestamp,
            price: snapshot.price,
            quantity: snapshot.quantity,
            side: snapshot.side.to_string(),
            trade_id: snapshot.trade_id.to_string(),
        };

        MarketDataMessage {
            data: Some(crate::proto::market_data_message::Data::Trade(proto_trade)),
        }
    }

    /// Write order book data to database
    async fn write_orderbook_data(&self, data: &OrderBookData) -> RavenResult<()> {
        let snapshot = OrderBookSnapshot::from(data);
        self.database_client
            .write_orderbook_snapshot_safe(&snapshot)
            .await
    }

    /// Write trade data to database
    async fn write_trade_data(&self, data: &TradeData) -> RavenResult<()> {
        let snapshot = TradeSnapshot::from(data);
        self.database_client
            .write_trade_snapshot_safe(&snapshot)
            .await
    }

    /// Write funding rate data to database
    async fn write_funding_rate_data(&self, data: &FundingRateData) -> RavenResult<()> {
        self.database_client.write_funding_rate_safe(data).await
    }

    /// Get DataEngine metrics
    pub fn get_metrics(&self) -> HashMap<String, u64> {
        self.metrics.to_map()
    }

    /// Get configuration
    pub fn get_config(&self) -> &DataEngineConfig {
        &self.config
    }
}
