use raven::common::logging::{LogFormat, LoggingConfig, PerformanceTimer, SpanEvents};
use std::time::Duration;

#[test]
fn test_log_format_parsing() {
    assert_eq!("json".parse::<LogFormat>().unwrap(), LogFormat::Json);
    assert_eq!("pretty".parse::<LogFormat>().unwrap(), LogFormat::Pretty);
    assert_eq!("compact".parse::<LogFormat>().unwrap(), LogFormat::Compact);
    assert!("invalid".parse::<LogFormat>().is_err());
}

#[test]
fn test_span_events_parsing() {
    let events = SpanEvents::from_string("new,close");
    assert!(events.new);
    assert!(events.close);
    assert!(!events.enter);
    assert!(!events.exit);
    assert!(!events.active);
    assert!(!events.full);

    let full_events = SpanEvents::from_string("full");
    assert!(full_events.new);
    assert!(full_events.enter);
    assert!(full_events.exit);
    assert!(full_events.close);
    assert!(full_events.active);
    assert!(full_events.full);
}

#[test]
fn test_logging_config_default() {
    let config = LoggingConfig::default();
    assert_eq!(config.level, "info");
    assert_eq!(config.format, "pretty");
    assert!(config.include_timestamps);
    assert!(config.include_thread_ids);
    assert!(!config.include_targets);
}

#[test]
fn test_performance_timer() {
    let timer = PerformanceTimer::start("test_operation").with_metadata("test_key", "test_value");

    std::thread::sleep(Duration::from_millis(1));
    timer.finish();
}
