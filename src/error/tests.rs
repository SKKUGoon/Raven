use crate::error::{ErrorContext, RavenError};
use tonic::Status;

#[test]
fn test_error_creation() {
    let error = RavenError::database_connection("Connection failed");
    assert_eq!(error.category(), "database");
    assert!(error.is_retryable());
    assert_eq!(error.severity(), tracing::Level::ERROR);
}

#[test]
fn test_error_conversion_to_status() {
    let error = RavenError::client_not_found("test-client");
    let status: Status = error.into();
    assert_eq!(status.code(), tonic::Code::NotFound);
}

#[test]
fn test_error_categories() {
    assert_eq!(RavenError::database_write("test").category(), "database");
    assert_eq!(RavenError::grpc_connection("test").category(), "network");
    assert_eq!(
        RavenError::subscription_failed("test").category(),
        "subscription"
    );
    assert_eq!(RavenError::data_validation("test").category(), "data");
    assert_eq!(
        RavenError::configuration("test").category(),
        "configuration"
    );
    assert_eq!(RavenError::timeout("test", 1000).category(), "system");
    assert_eq!(RavenError::authentication("test").category(), "security");
    assert_eq!(
        RavenError::dead_letter_queue_full(10, 100).category(),
        "dead_letter"
    );
    assert_eq!(RavenError::internal("test").category(), "general");
}

#[test]
fn test_retryable_errors() {
    assert!(RavenError::database_connection("test").is_retryable());
    assert!(RavenError::timeout("test", 1000).is_retryable());
    assert!(!RavenError::data_validation("test").is_retryable());
    assert!(!RavenError::authentication("test").is_retryable());
}

#[test]
fn test_severity_levels() {
    assert_eq!(
        RavenError::database_connection("test").severity(),
        tracing::Level::ERROR
    );
    assert_eq!(
        RavenError::database_write("test").severity(),
        tracing::Level::WARN
    );
    assert_eq!(
        RavenError::client_disconnected("test").severity(),
        tracing::Level::INFO
    );
    assert_eq!(
        RavenError::data_validation("test").severity(),
        tracing::Level::DEBUG
    );
}

#[test]
fn test_error_context() {
    let result: Result<(), std::io::Error> = Err(std::io::Error::new(
        std::io::ErrorKind::ConnectionRefused,
        "Connection refused",
    ));

    let raven_result = result.with_grpc_context();
    assert!(raven_result.is_err());

    if let Err(error) = raven_result {
        assert_eq!(error.category(), "network");
    }
}
