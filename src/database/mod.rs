// Database Module
// "The Iron Bank - where all data is stored, protected by failsafes"

pub mod circuit_breaker;
pub mod dead_letter_queue;
pub mod influx_client;
pub mod retry_handlers;
pub mod tests;

// Re-export commonly used types
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerRegistry, CircuitBreakerState,
    CircuitBreakerStats,
};
pub use dead_letter_queue::{
    DeadLetterEntry, DeadLetterQueue, DeadLetterQueueConfig, DeadLetterQueueStats, RetryHandler,
};
pub use influx_client::{InfluxClient, InfluxConfig};
pub use retry_handlers::{DatabaseDeadLetterHelper, EnhancedInfluxClient, InfluxWriteRetryHandler};
