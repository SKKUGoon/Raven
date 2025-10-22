use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    pub data: String,
    pub timestamp: i64,
    pub retry_count: u32,
    pub error_message: String,
}
