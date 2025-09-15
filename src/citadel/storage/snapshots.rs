// Snapshot structures for periodic captures and conversion functions

use super::{OrderBookData, TradeData, TradeSide};
use serde::{Deserialize, Serialize};

// Snapshot structures for periodic captures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub symbol: String,
    pub timestamp: i64,
    pub best_bid_price: f64,
    pub best_bid_quantity: f64,
    pub best_ask_price: f64,
    pub best_ask_quantity: f64,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSnapshot {
    pub symbol: String,
    pub timestamp: i64,
    pub price: f64,
    pub quantity: f64,
    pub side: TradeSide,
    pub trade_id: u64,
}

// Conversion functions between atomic storage and snapshot types
impl From<&OrderBookData> for OrderBookSnapshot {
    fn from(data: &OrderBookData) -> Self {
        let (best_bid_price, best_bid_quantity) = data
            .bids
            .first()
            .map(|(price, qty)| (*price, *qty))
            .unwrap_or((0.0, 0.0));

        let (best_ask_price, best_ask_quantity) = data
            .asks
            .first()
            .map(|(price, qty)| (*price, *qty))
            .unwrap_or((0.0, 0.0));

        Self {
            symbol: data.symbol.clone(),
            timestamp: data.timestamp,
            best_bid_price,
            best_bid_quantity,
            best_ask_price,
            best_ask_quantity,
            sequence: data.sequence,
        }
    }
}

impl From<&TradeData> for TradeSnapshot {
    fn from(data: &TradeData) -> Self {
        // Convert trade_id string to hash for snapshot
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.trade_id.hash(&mut hasher);
        let trade_id_hash = hasher.finish();

        Self {
            symbol: data.symbol.clone(),
            timestamp: data.timestamp,
            price: data.price,
            quantity: data.quantity,
            side: data.side.clone(),
            trade_id: trade_id_hash,
        }
    }
}
