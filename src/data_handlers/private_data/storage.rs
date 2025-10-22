use dashmap::DashMap;

use super::{AccessLevel, ClientPermissions, PositionUpdateData, WalletUpdateData};
use crate::error::{RavenError, RavenResult};

/// Private data storage with client isolation
#[derive(Debug)]
pub struct PrivateDataStorage {
    wallet_updates: DashMap<String, Vec<WalletUpdateData>>,
    position_updates: DashMap<String, Vec<PositionUpdateData>>,
    client_permissions: DashMap<String, ClientPermissions>,
    max_items_per_user: usize,
}

impl PrivateDataStorage {
    pub fn new(max_items_per_user: usize) -> Self {
        Self {
            wallet_updates: DashMap::new(),
            position_updates: DashMap::new(),
            client_permissions: DashMap::new(),
            max_items_per_user,
        }
    }

    pub fn add_wallet_update(&self, data: &WalletUpdateData) {
        let mut entry = self.wallet_updates.entry(data.user_id.clone()).or_default();
        entry.push(data.clone());

        if entry.len() > self.max_items_per_user {
            let excess = entry.len() - self.max_items_per_user;
            entry.drain(0..excess);
        }
    }

    pub fn add_position_update(&self, data: &PositionUpdateData) {
        let mut entry = self
            .position_updates
            .entry(data.user_id.clone())
            .or_default();
        entry.push(data.clone());

        if entry.len() > self.max_items_per_user {
            let excess = entry.len() - self.max_items_per_user;
            entry.drain(0..excess);
        }
    }

    pub fn register_client(&self, client_id: &str, permissions: ClientPermissions) {
        self.client_permissions
            .insert(client_id.to_string(), permissions);
    }

    pub fn get_wallet_updates(
        &self,
        user_id: &str,
        client_id: &str,
    ) -> RavenResult<Option<Vec<WalletUpdateData>>> {
        if self.check_access(client_id, user_id, "wallet_updates")? {
            Ok(self.wallet_updates.get(user_id).map(|entry| entry.clone()))
        } else {
            Err(RavenError::authorization("Access denied"))
        }
    }

    pub fn get_position_updates(
        &self,
        user_id: &str,
        client_id: &str,
    ) -> RavenResult<Option<Vec<PositionUpdateData>>> {
        if self.check_access(client_id, user_id, "position_updates")? {
            Ok(self
                .position_updates
                .get(user_id)
                .map(|entry| entry.clone()))
        } else {
            Err(RavenError::authorization("Access denied"))
        }
    }

    fn check_access(&self, client_id: &str, user_id: &str, data_type: &str) -> RavenResult<bool> {
        if let Some(permissions) = self.client_permissions.get(client_id) {
            if (permissions.user_id == user_id || permissions.access_level == AccessLevel::Admin)
                && (permissions
                    .allowed_data_types
                    .contains(&data_type.to_string())
                    || permissions.allowed_data_types.contains(&"*".to_string()))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn get_all_user_ids(&self) -> Vec<String> {
        let mut user_ids = std::collections::HashSet::new();
        user_ids.extend(self.wallet_updates.iter().map(|entry| entry.key().clone()));
        user_ids.extend(
            self.position_updates
                .iter()
                .map(|entry| entry.key().clone()),
        );
        user_ids.into_iter().collect()
    }
}
