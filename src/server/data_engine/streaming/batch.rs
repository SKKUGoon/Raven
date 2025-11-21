use crate::server::data_engine::storage::{OrderBookSnapshot, TradeSnapshot};
use crate::common::time::current_timestamp_millis;

/// Snapshot batch for efficient database writes
#[derive(Debug, Clone)]
pub struct SnapshotBatch {
    pub orderbook_snapshots: Vec<OrderBookSnapshot>,
    pub trade_snapshots: Vec<TradeSnapshot>,
    pub timestamp: i64,
}

impl SnapshotBatch {
    pub fn new() -> Self {
        Self {
            orderbook_snapshots: Vec::new(),
            trade_snapshots: Vec::new(),
            timestamp: current_timestamp_millis(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.orderbook_snapshots.is_empty() && self.trade_snapshots.is_empty()
    }

    pub fn len(&self) -> usize {
        self.orderbook_snapshots.len() + self.trade_snapshots.len()
    }

    pub fn add_orderbook_snapshot(&mut self, snapshot: OrderBookSnapshot) {
        self.orderbook_snapshots.push(snapshot);
    }

    pub fn add_trade_snapshot(&mut self, snapshot: TradeSnapshot) {
        self.trade_snapshots.push(snapshot);
    }
}

impl Default for SnapshotBatch {
    fn default() -> Self {
        Self::new()
    }
}

