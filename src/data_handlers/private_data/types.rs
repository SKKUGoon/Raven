use serde::{Deserialize, Serialize};

/// Private data types for secure processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletUpdateData {
    pub user_id: String,
    pub timestamp: i64,
    pub balances: Vec<BalanceData>,
    pub exchange: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceData {
    pub asset: String,
    pub available: f64,
    pub locked: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionUpdateData {
    pub user_id: String,
    pub symbol: String,
    pub timestamp: i64,
    pub size: f64,
    pub entry_price: f64,
    pub mark_price: f64,
    pub unrealized_pnl: f64,
    pub side: String,
    pub exchange: String,
}

/// Private data message types
#[derive(Debug, Clone)]
pub enum PrivateData {
    WalletUpdate(WalletUpdateData),
    PositionUpdate(PositionUpdateData),
}

/// Secure channel message
#[derive(Debug, Clone)]
pub struct SecureChannelMessage {
    pub user_id: String,
    pub data: PrivateData,
    pub timestamp: i64,
}

/// Client permissions
#[derive(Debug, Clone)]
pub struct ClientPermissions {
    pub user_id: String,
    pub allowed_data_types: Vec<String>,
    pub access_level: AccessLevel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccessLevel {
    ReadOnly,
    ReadWrite,
    Admin,
}
