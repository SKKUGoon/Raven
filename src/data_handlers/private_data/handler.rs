use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info};

use super::{
    ClientPermissions, PositionUpdateData, PrivateData, PrivateDataConfig, PrivateDataMetrics,
    PrivateDataStorage, SecureChannelMessage, WalletUpdateData,
};
use crate::error::{RavenError, RavenResult};

/// Private data handler with secure processing and client isolation
pub struct PrivateDataHandler {
    config: PrivateDataConfig,
    storage: Arc<PrivateDataStorage>,
    sender: mpsc::UnboundedSender<SecureChannelMessage>,
    receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<SecureChannelMessage>>>>,
    metrics: Arc<PrivateDataMetrics>,
    processing_active: Arc<RwLock<bool>>,
}

impl PrivateDataHandler {
    pub fn new() -> Self {
        Self::with_config(PrivateDataConfig::default())
    }

    pub fn with_config(config: PrivateDataConfig) -> Self {
        info!("⚿ Private data ravens are preparing for secure flight...");

        let (sender, receiver) = mpsc::unbounded_channel();
        let storage = Arc::new(PrivateDataStorage::new(config.max_items_per_user));

        Self {
            config,
            storage,
            sender,
            receiver: Arc::new(RwLock::new(Some(receiver))),
            metrics: Arc::new(PrivateDataMetrics::default()),
            processing_active: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn start(&self) -> RavenResult<()> {
        let mut processing_active = self.processing_active.write().await;
        if *processing_active {
            return Ok(());
        }

        let mut receiver_guard = self.receiver.write().await;
        let receiver = receiver_guard
            .take()
            .ok_or_else(|| RavenError::internal("Handler already started"))?;
        drop(receiver_guard);

        *processing_active = true;
        drop(processing_active);

        let handler = self.clone();
        tokio::spawn(async move {
            handler.processing_loop(receiver).await;
        });

        info!("✓ Private data handler started");
        Ok(())
    }

    pub async fn stop(&self) -> RavenResult<()> {
        let mut processing_active = self.processing_active.write().await;
        *processing_active = false;
        info!("■ Private data handler stopped");
        Ok(())
    }

    pub async fn ingest_wallet_update(&self, data: &WalletUpdateData) -> RavenResult<()> {
        self.validate_wallet_data(data)?;

        let message = SecureChannelMessage {
            user_id: data.user_id.clone(),
            data: PrivateData::WalletUpdate(data.clone()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        };

        self.sender
            .send(message)
            .map_err(|e| RavenError::internal(format!("Failed to enqueue wallet update: {e}")))?;
        debug!("⚿ Wallet update queued for user {}", data.user_id);
        Ok(())
    }

    pub async fn ingest_position_update(&self, data: &PositionUpdateData) -> RavenResult<()> {
        self.validate_position_data(data)?;

        let message = SecureChannelMessage {
            user_id: data.user_id.clone(),
            data: PrivateData::PositionUpdate(data.clone()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        };

        self.sender
            .send(message)
            .map_err(|e| RavenError::internal(format!("Failed to enqueue position update: {e}")))?;
        debug!(
            "⚿ Position update queued for user {} symbol {}",
            data.user_id, data.symbol
        );
        Ok(())
    }

    pub async fn register_client(
        &self,
        client_id: &str,
        permissions: ClientPermissions,
    ) -> RavenResult<()> {
        self.storage.register_client(client_id, permissions);
        info!("⚿ Client {} registered", client_id);
        Ok(())
    }

    pub async fn get_wallet_updates(
        &self,
        user_id: &str,
        client_id: &str,
    ) -> RavenResult<Option<Vec<WalletUpdateData>>> {
        match self.storage.get_wallet_updates(user_id, client_id) {
            Ok(data) => Ok(data),
            Err(_) => {
                self.metrics
                    .access_denied_count
                    .fetch_add(1, Ordering::Relaxed);
                Err(RavenError::authorization("Access denied"))
            }
        }
    }

    pub async fn get_position_updates(
        &self,
        user_id: &str,
        client_id: &str,
    ) -> RavenResult<Option<Vec<PositionUpdateData>>> {
        match self.storage.get_position_updates(user_id, client_id) {
            Ok(data) => Ok(data),
            Err(_) => {
                self.metrics
                    .access_denied_count
                    .fetch_add(1, Ordering::Relaxed);
                Err(RavenError::authorization("Access denied"))
            }
        }
    }

    pub fn get_metrics(&self) -> HashMap<String, u64> {
        let mut metrics = HashMap::new();
        metrics.insert(
            "wallet_updates_processed".to_string(),
            self.metrics
                .wallet_updates_processed
                .load(Ordering::Relaxed),
        );
        metrics.insert(
            "position_updates_processed".to_string(),
            self.metrics
                .position_updates_processed
                .load(Ordering::Relaxed),
        );
        metrics.insert(
            "total_failed".to_string(),
            self.metrics.total_failed.load(Ordering::Relaxed),
        );
        metrics.insert(
            "access_denied_count".to_string(),
            self.metrics.access_denied_count.load(Ordering::Relaxed),
        );
        metrics
    }

    async fn processing_loop(&self, mut receiver: mpsc::UnboundedReceiver<SecureChannelMessage>) {
        info!("⚿ Private data processing loop started");

        while *self.processing_active.read().await {
            if let Ok(message) = receiver.try_recv() {
                if (self.process_message(&message).await).is_err() {
                    self.metrics.total_failed.fetch_add(1, Ordering::Relaxed);
                }
            }
            tokio::time::sleep(self.config.processing_interval).await;
        }

        info!("⚿ Private data processing loop stopped");
    }

    async fn process_message(&self, message: &SecureChannelMessage) -> RavenResult<()> {
        match &message.data {
            PrivateData::WalletUpdate(wallet) => {
                self.storage.add_wallet_update(wallet);
                self.metrics
                    .wallet_updates_processed
                    .fetch_add(1, Ordering::Relaxed);
                debug!("⚿ Processed wallet update for user {}", wallet.user_id);
            }
            PrivateData::PositionUpdate(position) => {
                self.storage.add_position_update(position);
                self.metrics
                    .position_updates_processed
                    .fetch_add(1, Ordering::Relaxed);
                debug!(
                    "⚿ Processed position update for user {} symbol {}",
                    position.user_id, position.symbol
                );
            }
        }
        Ok(())
    }

    fn validate_wallet_data(&self, data: &WalletUpdateData) -> RavenResult<()> {
        if data.user_id.is_empty() {
            return Err(RavenError::data_validation("User ID cannot be empty"));
        }
        if data.timestamp <= 0 {
            return Err(RavenError::data_validation("Invalid timestamp"));
        }
        if data.balances.is_empty() {
            return Err(RavenError::data_validation("Balances cannot be empty"));
        }
        for balance in &data.balances {
            if balance.available < 0.0 || balance.locked < 0.0 {
                return Err(RavenError::data_validation(
                    "Balance amounts cannot be negative",
                ));
            }
        }
        Ok(())
    }

    fn validate_position_data(&self, data: &PositionUpdateData) -> RavenResult<()> {
        if data.user_id.is_empty() {
            return Err(RavenError::data_validation("User ID cannot be empty"));
        }
        if data.symbol.is_empty() {
            return Err(RavenError::data_validation("Symbol cannot be empty"));
        }
        if data.timestamp <= 0 {
            return Err(RavenError::data_validation("Invalid timestamp"));
        }
        if !matches!(data.side.as_str(), "long" | "short") {
            return Err(RavenError::data_validation("Invalid position side"));
        }
        if data.entry_price <= 0.0 || data.mark_price <= 0.0 {
            return Err(RavenError::data_validation("Prices must be positive"));
        }
        Ok(())
    }
}

impl Clone for PrivateDataHandler {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            storage: Arc::clone(&self.storage),
            sender: self.sender.clone(),
            receiver: Arc::clone(&self.receiver),
            metrics: Arc::clone(&self.metrics),
            processing_active: Arc::clone(&self.processing_active),
        }
    }
}

impl Default for PrivateDataHandler {
    fn default() -> Self {
        Self::new()
    }
}
