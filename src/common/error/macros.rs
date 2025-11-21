// Error Macros
// "Standardized error handling macros for consistent patterns"

/// Macro for creating errors with context
#[macro_export]
macro_rules! raven_error {
    ($variant:ident $(, $arg:expr)* $(,)?) => {
        $crate::common::error::RavenError::$variant($($arg),*)
    };
}

/// Macro for early return with error logging
#[macro_export]
macro_rules! raven_bail {
    ($error:expr) => {
        {
            let error = $error;
            match error.severity() {
                tracing::Level::ERROR => tracing::error!(error = %error, "Operation failed"),
                tracing::Level::WARN => tracing::warn!(error = %error, "Operation failed"),
                tracing::Level::INFO => tracing::info!(error = %error, "Operation failed"),
                tracing::Level::DEBUG => tracing::debug!(error = %error, "Operation failed"),
                tracing::Level::TRACE => tracing::trace!(error = %error, "Operation failed"),
            }
            return Err(error);
        }
    };
}

/// Macro for error logging with context
#[macro_export]
macro_rules! log_error {
    ($error:expr, $($field:tt)*) => {
        {
            let error = $error;
            tracing::event!(
                error.severity(),
                error = %error,
                category = error.category(),
                retryable = error.is_retryable(),
                $($field)*
            );
        }
    };
}

/// Enhanced error handling macros for common patterns
#[macro_export]
macro_rules! handle_database_error {
    ($result:expr, $operation:expr, $table:expr) => {
        $result.with_database_context($operation, Some($table))
    };
    ($result:expr, $operation:expr) => {
        $result.with_database_context($operation, None)
    };
}

#[macro_export]
macro_rules! handle_grpc_error {
    ($result:expr, $service:expr, $method:expr) => {
        $result.with_grpc_context($service, $method)
    };
}

#[macro_export]
macro_rules! handle_subscription_error {
    ($result:expr, $client_id:expr, $subscription_type:expr) => {
        $result.with_subscription_context($client_id, $subscription_type)
    };
}

#[macro_export]
macro_rules! handle_config_error {
    ($result:expr, $config_key:expr) => {
        $result.with_config_context($config_key)
    };
}

#[macro_export]
macro_rules! handle_exchange_error {
    ($result:expr, $exchange:expr, $symbol:expr) => {
        $result.with_exchange_context($exchange, Some($symbol))
    };
    ($result:expr, $exchange:expr) => {
        $result.with_exchange_context($exchange, None)
    };
}

#[macro_export]
macro_rules! handle_operation_error {
    ($result:expr, $operation:expr, $component:expr) => {
        $result.with_operation_context($operation, Some($component))
    };
    ($result:expr, $operation:expr) => {
        $result.with_operation_context($operation, None)
    };
}

/// Macro for creating contextual errors with automatic context generation
#[macro_export]
macro_rules! contextual_error {
    ($error:expr, $operation:expr, $component:expr) => {{
        let mut context = $crate::common::error::ErrorContextInfo::default();
        context.operation_id = Some($operation.to_string());
        context.component = Some($component.to_string());
        $error.with_context(context)
    }};
    ($error:expr, $operation:expr) => {{
        let mut context = $crate::common::error::ErrorContextInfo::default();
        context.operation_id = Some($operation.to_string());
        $error.with_context(context)
    }};
}

/// Macro for error recovery with automatic strategy application
#[macro_export]
macro_rules! recover_error {
    ($error:expr, $metrics:expr) => {
        {
            let error = $error;
            let strategy = error.recovery_strategy();

            // Track the error
            $metrics.track_error(&error);

            // Log with context
            tracing::error!(
                error = %error,
                strategy = ?strategy,
                category = error.category(),
                retryable = error.is_retryable(),
                "Error occurred with recovery strategy"
            );

            Err(error)
        }
    };
}
