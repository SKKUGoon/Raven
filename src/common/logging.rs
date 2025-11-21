// Structured Logging Configuration
// "Every raven's message must be recorded"

use crate::common::error::RavenError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Registry,
};

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Output format (json, pretty, compact)
    pub format: String,
    /// Whether to include timestamps
    pub include_timestamps: bool,
    /// Whether to include thread IDs
    pub include_thread_ids: bool,
    /// Whether to include target module names
    pub include_targets: bool,
    /// Whether to include file and line numbers
    pub include_file_line: bool,
    /// Whether to include span information
    pub include_spans: bool,
    /// Span events to include (new, enter, exit, close, active, full)
    pub span_events: String,
    /// Custom fields to include in all log messages
    pub custom_fields: HashMap<String, String>,
    /// Whether to enable ANSI colors in output
    pub enable_colors: bool,
    /// Log file path (optional, logs to stdout if not specified)
    pub file_path: Option<String>,
    /// Maximum log file size in bytes before rotation
    pub max_file_size: Option<u64>,
    /// Number of rotated log files to keep
    pub max_files: Option<u32>,
    /// Environment filter override
    pub env_filter: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
            include_timestamps: true,
            include_thread_ids: true,
            include_targets: false,
            include_file_line: false,
            include_spans: true,
            span_events: "new,close".to_string(),
            custom_fields: HashMap::new(),
            enable_colors: true,
            file_path: None,
            max_file_size: Some(100 * 1024 * 1024), // 100MB
            max_files: Some(5),
            env_filter: None,
        }
    }
}

/// Logging format options
#[derive(Debug, Clone, PartialEq)]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

impl std::str::FromStr for LogFormat {
    type Err = RavenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(LogFormat::Json),
            "pretty" => Ok(LogFormat::Pretty),
            "compact" => Ok(LogFormat::Compact),
            _ => {
                crate::raven_bail!(crate::raven_error!(
                    configuration,
                    format!("Invalid log format: {s}. Valid options: json, pretty, compact")
                ));
            }
        }
    }
}

/// Span events configuration
#[derive(Debug, Clone)]
pub struct SpanEvents {
    pub new: bool,
    pub enter: bool,
    pub exit: bool,
    pub close: bool,
    pub active: bool,
    pub full: bool,
}

impl SpanEvents {
    pub fn from_string(s: &str) -> Self {
        let events: Vec<String> = s.split(',').map(|s| s.trim().to_lowercase()).collect();

        Self {
            new: events.contains(&"new".to_string()) || events.contains(&"full".to_string()),
            enter: events.contains(&"enter".to_string()) || events.contains(&"full".to_string()),
            exit: events.contains(&"exit".to_string()) || events.contains(&"full".to_string()),
            close: events.contains(&"close".to_string()) || events.contains(&"full".to_string()),
            active: events.contains(&"active".to_string()) || events.contains(&"full".to_string()),
            full: events.contains(&"full".to_string()),
        }
    }

    pub fn to_fmt_span(&self) -> FmtSpan {
        let mut span = FmtSpan::NONE;

        if self.new {
            span |= FmtSpan::NEW;
        }
        if self.enter {
            span |= FmtSpan::ENTER;
        }
        if self.exit {
            span |= FmtSpan::EXIT;
        }
        if self.close {
            span |= FmtSpan::CLOSE;
        }
        if self.active {
            span |= FmtSpan::ACTIVE;
        }
        if self.full {
            FmtSpan::FULL
        } else {
            span
        }
    }
}

/// Initialize logging with the given configuration
pub fn init_logging(config: &LoggingConfig) -> Result<(), RavenError> {
    // Parse log level
    let level = config.level.parse::<Level>().map_err(|_| {
        crate::raven_error!(
            configuration,
            format!("Invalid log level: {}", config.level)
        )
    })?;

    // Parse log format
    let format = config.format.parse::<LogFormat>()?;

    // Create environment filter
    let env_filter = if let Some(ref filter) = config.env_filter {
        EnvFilter::try_new(filter)
            .map_err(|e| crate::raven_error!(configuration, format!("Invalid env filter: {e}")))?
    } else {
        EnvFilter::from_default_env()
            .add_directive(
                format!("market_data_subscription_server={level}")
                    .parse()
                    .unwrap(),
            )
            .add_directive(format!("raven={level}").parse().unwrap())
    };

    // Parse span events
    let span_events = SpanEvents::from_string(&config.span_events);

    // Create the subscriber based on format and configuration
    let subscriber = Registry::default().with(env_filter);

    match format {
        LogFormat::Json => {
            let layer = fmt::layer()
                .with_target(config.include_targets)
                .with_thread_ids(config.include_thread_ids)
                .with_file(config.include_file_line)
                .with_line_number(config.include_file_line)
                .with_span_events(span_events.to_fmt_span());

            if let Some(ref file_path) = config.file_path {
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file_path)
                    .map_err(|e| {
                        crate::raven_error!(configuration, format!("Failed to open log file: {e}"))
                    })?;

                subscriber.with(layer.with_writer(file)).init();
            } else {
                subscriber.with(layer.with_writer(io::stdout)).init();
            }
        }
        LogFormat::Pretty => {
            let layer = fmt::layer()
                .pretty()
                .with_target(config.include_targets)
                .with_thread_ids(config.include_thread_ids)
                .with_file(config.include_file_line)
                .with_line_number(config.include_file_line)
                .with_span_events(span_events.to_fmt_span())
                .with_ansi(config.enable_colors);

            if let Some(ref file_path) = config.file_path {
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file_path)
                    .map_err(|e| {
                        crate::raven_error!(configuration, format!("Failed to open log file: {e}"))
                    })?;

                subscriber.with(layer.with_writer(file)).init();
            } else {
                subscriber.with(layer.with_writer(io::stdout)).init();
            }
        }
        LogFormat::Compact => {
            let layer = fmt::layer()
                .compact()
                .with_target(config.include_targets)
                .with_thread_ids(config.include_thread_ids)
                .with_file(config.include_file_line)
                .with_line_number(config.include_file_line)
                .with_span_events(span_events.to_fmt_span())
                .with_ansi(config.enable_colors);

            if let Some(ref file_path) = config.file_path {
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file_path)
                    .map_err(|e| {
                        crate::raven_error!(configuration, format!("Failed to open log file: {e}"))
                    })?;

                subscriber.with(layer.with_writer(file)).init();
            } else {
                subscriber.with(layer.with_writer(io::stdout)).init();
            }
        }
    }

    tracing::info!(
        "⚬ Logging initialized with level: {}, format: {}",
        config.level,
        config.format
    );

    // Log custom fields if any
    if !config.custom_fields.is_empty() {
        tracing::info!("⚬ Custom log fields: {:?}", config.custom_fields);
    }

    Ok(())
}

/// Structured logging macros for common operations
#[macro_export]
macro_rules! log_operation {
    ($level:expr, $operation:expr, $($field:tt)*) => {
        tracing::event!(
            $level,
            operation = $operation,
            $($field)*
        );
    };
}

#[macro_export]
macro_rules! log_client_event {
    ($level:expr, $event:expr, $client_id:expr, $($field:tt)*) => {
        tracing::event!(
            $level,
            event = $event,
            client_id = $client_id,
            $($field)*
        );
    };
}

#[macro_export]
macro_rules! log_database_event {
    ($level:expr, $event:expr, $measurement:expr, $($field:tt)*) => {
        tracing::event!(
            $level,
            event = $event,
            measurement = $measurement,
            category = "database",
            $($field)*
        );
    };
}

#[macro_export]
macro_rules! log_subscription_event {
    ($level:expr, $event:expr, $client_id:expr, $symbol:expr, $($field:tt)*) => {
        tracing::event!(
            $level,
            event = $event,
            client_id = $client_id,
            symbol = $symbol,
            category = "subscription",
            $($field)*
        );
    };
}

#[macro_export]
macro_rules! log_performance {
    ($operation:expr, $duration_ms:expr $(, $($field:tt)*)?) => {
        tracing::info!(
            operation = $operation,
            duration_ms = $duration_ms,
            category = "performance"
            $(, $($field)*)?
        );
    };
}

/// Performance measurement helper
pub struct PerformanceTimer {
    operation: String,
    start_time: std::time::Instant,
    metadata: HashMap<String, String>,
}

impl PerformanceTimer {
    /// Start a new performance timer
    pub fn start<S: Into<String>>(operation: S) -> Self {
        Self {
            operation: operation.into(),
            start_time: std::time::Instant::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the timer
    pub fn with_metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Finish the timer and log the duration
    pub fn finish(self) {
        let duration_ms = self.start_time.elapsed().as_millis() as u64;

        if self.metadata.is_empty() {
            log_performance!(self.operation, duration_ms);
        } else {
            tracing::info!(
                operation = self.operation,
                duration_ms = duration_ms,
                category = "performance",
                metadata = ?self.metadata
            );
        }
    }

    /// Finish the timer with a custom log level
    pub fn finish_with_level(self, level: Level) {
        let duration_ms = self.start_time.elapsed().as_millis() as u64;

        // Use match instead of tracing::event! with dynamic level
        match level {
            Level::ERROR => tracing::error!(
                operation = self.operation,
                duration_ms = duration_ms,
                category = "performance",
                metadata = ?self.metadata
            ),
            Level::WARN => tracing::warn!(
                operation = self.operation,
                duration_ms = duration_ms,
                category = "performance",
                metadata = ?self.metadata
            ),
            Level::INFO => tracing::info!(
                operation = self.operation,
                duration_ms = duration_ms,
                category = "performance",
                metadata = ?self.metadata
            ),
            Level::DEBUG => tracing::debug!(
                operation = self.operation,
                duration_ms = duration_ms,
                category = "performance",
                metadata = ?self.metadata
            ),
            Level::TRACE => tracing::trace!(
                operation = self.operation,
                duration_ms = duration_ms,
                category = "performance",
                metadata = ?self.metadata
            ),
        }
    }
}

/// Error logging helper
pub fn log_error_with_context(error: &RavenError, context: &str) {
    let level = error.severity();
    match level {
        Level::ERROR => tracing::error!(
            error = %error,
            context = context,
            category = error.category(),
            retryable = error.is_retryable(),
            "Operation failed with error"
        ),
        Level::WARN => tracing::warn!(
            error = %error,
            context = context,
            category = error.category(),
            retryable = error.is_retryable(),
            "Operation failed with error"
        ),
        Level::INFO => tracing::info!(
            error = %error,
            context = context,
            category = error.category(),
            retryable = error.is_retryable(),
            "Operation failed with error"
        ),
        Level::DEBUG => tracing::debug!(
            error = %error,
            context = context,
            category = error.category(),
            retryable = error.is_retryable(),
            "Operation failed with error"
        ),
        Level::TRACE => tracing::trace!(
            error = %error,
            context = context,
            category = error.category(),
            retryable = error.is_retryable(),
            "Operation failed with error"
        ),
    }
}

/// Request/response logging helper
pub fn log_request<T: std::fmt::Debug>(operation: &str, request: &T) {
    tracing::debug!(
        operation = operation,
        request = ?request,
        category = "request",
        "Processing request"
    );
}

pub fn log_response<T: std::fmt::Debug>(operation: &str, response: &T, duration_ms: u64) {
    tracing::debug!(
        operation = operation,
        response = ?response,
        duration_ms = duration_ms,
        category = "response",
        "Sending response"
    );
}

/// Health check logging
pub fn log_health_check(component: &str, healthy: bool, details: Option<&str>) {
    if healthy {
        tracing::info!(
            component = component,
            status = "healthy",
            details = details,
            category = "health",
            "Health check passed"
        );
    } else {
        tracing::warn!(
            component = component,
            status = "unhealthy",
            details = details,
            category = "health",
            "Health check failed"
        );
    }
}

/// Metrics logging
pub fn log_metrics(metrics: &HashMap<String, u64>) {
    tracing::info!(
        metrics = ?metrics,
        category = "metrics",
        "System metrics"
    );
}

/// Configuration validation logging
pub fn log_config_validation(component: &str, valid: bool, warnings: &[String]) {
    if valid && warnings.is_empty() {
        tracing::info!(
            component = component,
            status = "valid",
            category = "config",
            "Configuration validation passed"
        );
    } else if valid && !warnings.is_empty() {
        tracing::warn!(
            component = component,
            status = "valid_with_warnings",
            warnings = ?warnings,
            category = "config",
            "Configuration validation passed with warnings"
        );
    } else {
        tracing::error!(
            component = component,
            status = "invalid",
            warnings = ?warnings,
            category = "config",
            "Configuration validation failed"
        );
    }
}
