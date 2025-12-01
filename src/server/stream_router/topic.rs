pub use crate::proto::{DataType, MarketDataMessage};
use dashmap::DashMap;
use std::collections::HashSet;

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
