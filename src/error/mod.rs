// Error Handling Module
// "Winter is coming - prepare for failures"

// Sub-modules
mod classification;
mod context;
mod conversions;
mod handler;
mod macros;
mod metrics;
mod recovery;
mod traits;
mod types;

// Re-export all public types and functionality
pub use context::{ContextualRavenError, ErrorContextInfo};
pub use handler::ErrorHandler;
pub use metrics::ErrorMetrics;
pub use recovery::RecoveryStrategy;
pub use traits::{EnhancedErrorContext, ErrorContext};
pub use types::RavenError;

// Note: Macros are exported globally via #[macro_export]
// Note: Conversions (From implementations) are automatically available
// Note: Classification methods are available as methods on RavenError

// Result type alias for convenience
pub type RavenResult<T> = Result<T, RavenError>;

#[cfg(test)]
mod tests;
