use crate::proto::MarketDataMessage;
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::atomic::AtomicUsize;
use tokio::sync::mpsc;
use tracing::{error, info};

#[derive(Debug)]
pub struct ClientSubscription {
    pub id: usize,
    pub sender: mpsc::UnboundedSender<MarketDataMessage>,
    pub topics: HashSet<String>,
}

pub struct TibStreamRouter {
    subscribers: DashMap<usize, ClientSubscription>,
    _next_id: AtomicUsize,
}

impl Default for TibStreamRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl TibStreamRouter {
    pub fn new() -> Self {
        Self {
            subscribers: DashMap::new(),
            _next_id: AtomicUsize::new(0),
        }
    }

    // pub fn subscribe(&self) -> (usize, mpsc::UnboundedReceiver<MarketDataMessage>) {
    //     let (tx, rx) = mpsc::unbounded_channel();
    //     let id = self.next_id.fetch_add(1, Ordering::Relaxed);

    //     self.subscribers.insert(id, sub);
    //     info!("New client subscribed to TIB stream (ID: {id})");

    //     (id, rx)
    // }

    pub fn unsubscribe(&self, id: usize) {
        if self.subscribers.remove(&id).is_some() {
            info!("Client unsubscribed from TIB stream (ID: {id})");
        }
    }

    pub fn distribute(&self, msg: MarketDataMessage) {
        // Simple broadcast to all for now, as this client generates one stream
        for sub in self.subscribers.iter() {
            if let Err(e) = sub.sender.send(msg.clone()) {
                error!("Failed to send message to client (ID: {}): {}", sub.id, e);
                self.unsubscribe(sub.id);
            }
        }
    }
}
