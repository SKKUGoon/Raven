// Database Module
// "The Iron Bank - where all data is stored, protected by failsafes"

pub mod influx_client;
pub mod retry_handlers;
pub mod tests;
pub mod circuit_breaker;
pub mod dead_letter_queue;

// Re-export commonly used types
pub use influx_client::{InfluxClient, InfluxConfig};
pub use retry_handlers::{DatabaseDeadLetterHelper, EnhancedInfluxClient, InfluxWriteRetryHandler};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerRegistry, CircuitBreakerState, CircuitBreakerStats};
pub use dead_letter_queue::{DeadLetterQueue, DeadLetterQueueConfig, DeadLetterQueueStats, DeadLetterEntry, RetryHandler};
