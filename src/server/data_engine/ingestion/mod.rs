use super::DataEngine;
use crate::server::data_engine::storage::{OrderBookData, TradeData, TradeSide as StorageTradeSide};
use crate::server::data_handlers::HighFrequencyHandler;
use crate::server::exchanges::types::{Exchange, MarketData, MarketDataMessage, TradeSide as ExchangeTradeSide};
use crate::server::subscription_manager::SubscriptionManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;
use tracing::{error, warn};

/// Spawn a task that ingests order book updates into the high-frequency handler.
pub fn spawn_orderbook_ingestor(
    mut receiver: UnboundedReceiver<MarketDataMessage>,
    handler: Arc<HighFrequencyHandler>,
    data_engine: Arc<DataEngine>,
    subscription_manager: Arc<SubscriptionManager>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // Map key: (Exchange, Symbol) -> Sequence Number
        let mut sequence_map: HashMap<(Exchange, String), u64> = HashMap::new();

        while let Some(message) = receiver.recv().await {
            let MarketDataMessage {
                exchange,
                symbol,
                timestamp,
                data,
            } = message;

            match data {
                MarketData::OrderBook { bids, asks } => {
                    let sequence_key = (exchange.clone(), symbol.clone());
                    let sequence_entry = sequence_map.entry(sequence_key).or_insert(0);
                    *sequence_entry = sequence_entry.saturating_add(1);

                    let orderbook = OrderBookData {
                        symbol: symbol.clone(),
                        timestamp,
                        bids,
                        asks,
                        sequence: *sequence_entry,
                        exchange: exchange.clone(),
                    };

                    // 1. Atomic Update (In-Memory)
                    if let Err(err) = handler.ingest_orderbook_atomic(&symbol, &orderbook) {
                        error!(symbol = %symbol, ?err, "Failed to ingest order book update");
                    }

                    // 2. Broadcast to Clients (Real-time)
                    if let Err(err) = data_engine
                        .broadcast_orderbook_update(&symbol, &orderbook, &subscription_manager)
                        .await
                    {
                         error!(
                            symbol = %symbol,
                            ?err,
                            "Failed to broadcast order book update"
                        );
                    }

                    // 3. Persistence (Database)
                    if let Err(err) = data_engine
                        .process_orderbook_data(&symbol, orderbook.clone())
                        .await
                    {
                        error!(
                            symbol = %symbol,
                            ?err,
                            "Failed to process order book data in DataEngine"
                        );
                    }
                }
                other => {
                    warn!(
                        ?other,
                        "Unexpected market data variant on order book channel"
                    );
                }
            }
        }
    })
}

/// Spawn a task that ingests trade updates into the high-frequency handler.
pub fn spawn_trade_ingestor(
    mut receiver: UnboundedReceiver<MarketDataMessage>,
    handler: Arc<HighFrequencyHandler>,
    data_engine: Arc<DataEngine>,
    subscription_manager: Arc<SubscriptionManager>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(message) = receiver.recv().await {
            let MarketDataMessage {
                exchange,
                symbol,
                timestamp,
                data,
            } = message;

            match data {
                MarketData::FutureTrade {
                    price,
                    size,
                    side,
                    trade_id,
                    ..
                }
                | MarketData::SpotTrade {
                    price,
                    size,
                    side,
                    trade_id,
                    ..
                } => {
                    let storage_side = match side {
                        ExchangeTradeSide::Buy => StorageTradeSide::Buy,
                        ExchangeTradeSide::Sell => StorageTradeSide::Sell,
                    };

                    let trade = TradeData {
                        symbol: symbol.clone(),
                        timestamp,
                        price,
                        quantity: size,
                        side: storage_side,
                        trade_id,
                        exchange: exchange.clone(),
                    };

                    // 1. Atomic Update (In-Memory)
                    if let Err(err) = handler.ingest_trade_atomic(&symbol, &trade) {
                        error!(symbol = %symbol, ?err, "Failed to ingest trade update");
                    }

                     // 2. Broadcast to Clients (Real-time)
                    if let Err(err) = data_engine
                        .broadcast_trade_update(&symbol, &trade, &subscription_manager)
                        .await
                    {
                         error!(
                            symbol = %symbol,
                            ?err,
                            "Failed to broadcast trade update"
                        );
                    }

                    // 3. Persistence (Database)
                    if let Err(err) = data_engine.process_trade_data(&symbol, trade.clone()).await {
                        error!(
                            symbol = %symbol,
                            ?err,
                            "Failed to process trade data in DataEngine"
                        );
                    }
                }
                other => {
                    warn!(?other, "Unexpected market data variant on trade channel");
                }
            }
        }
    })
}
