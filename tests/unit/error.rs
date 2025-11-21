use raven::common::error::ErrorContext;
use tonic::Status;

#[test]
fn test_error_creation() {
    let error = raven::raven_error!(database_connection, "Connection failed");
    assert_eq!(error.category(), "database");
    assert!(error.is_retryable());
    assert_eq!(error.severity(), tracing::Level::ERROR);
}

#[test]
fn test_error_conversion_to_status() {
    let error = raven::raven_error!(client_not_found, "test-client");
    let status: Status = error.into();
    assert_eq!(status.code(), tonic::Code::NotFound);
}

#[test]
fn test_error_categories() {
    assert_eq!(
        raven::raven_error!(database_write, "test").category(),
        "database"
    );
    assert_eq!(
        raven::raven_error!(grpc_connection, "test").category(),
        "network"
    );
    assert_eq!(
        raven::raven_error!(subscription_failed, "test").category(),
        "subscription"
    );
    assert_eq!(
        raven::raven_error!(data_validation, "test").category(),
        "data"
    );
    assert_eq!(
        raven::raven_error!(configuration, "test").category(),
        "configuration"
    );
    assert_eq!(
        raven::raven_error!(timeout, "test", 1000).category(),
        "system"
    );
    assert_eq!(
        raven::raven_error!(authentication, "test").category(),
        "security"
    );
    assert_eq!(
        raven::raven_error!(dead_letter_queue_full, 10, 100).category(),
        "dead_letter"
    );
    assert_eq!(raven::raven_error!(internal, "test").category(), "general");
}

#[test]
fn test_retryable_errors() {
    assert!(raven::raven_error!(database_connection, "test").is_retryable());
    assert!(raven::raven_error!(timeout, "test", 1000).is_retryable());
    assert!(!raven::raven_error!(data_validation, "test").is_retryable());
    assert!(!raven::raven_error!(authentication, "test").is_retryable());
}

#[test]
fn test_severity_levels() {
    assert_eq!(
        raven::raven_error!(database_connection, "test").severity(),
        tracing::Level::ERROR
    );
    assert_eq!(
        raven::raven_error!(database_write, "test").severity(),
        tracing::Level::WARN
    );
    assert_eq!(
        raven::raven_error!(client_disconnected, "test").severity(),
        tracing::Level::INFO
    );
    assert_eq!(
        raven::raven_error!(data_validation, "test").severity(),
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
