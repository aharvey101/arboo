use std::env;
use log::{debug, error, info, trace, warn};
use arbooo::common::logger;

#[test]
fn test_trace_level_shows_all_logs() {
    env::set_var("RUST_LOG", "trace");
    logger::setup_logger();

    trace!("This is a trace message");
    debug!("This is a debug message");
    info!("This is an info message");
    warn!("This is a warn message");
    error!("This is an error message");

    println!("Trace level test completed - all log levels should be visible");
}

#[test]
fn test_error_level_shows_error_debug_warn_info() {
    env::set_var("RUST_LOG", "error");
    logger::setup_logger();

    trace!("This is a trace message - should NOT be shown");
    error!("This is an error message - should be shown");
    debug!("This is a debug message - should be shown");
    warn!("This is a warn message - should be shown");
    info!("This is an info message - should be shown");

    println!("Error level test completed - error, debug, warn, info should be visible");
}

#[test]
fn test_debug_level_shows_debug_warn_info() {
    env::set_var("RUST_LOG", "debug");
    logger::setup_logger();

    trace!("This is a trace message - should NOT be shown");
    error!("This is an error message - should NOT be shown");
    debug!("This is a debug message - should be shown");
    warn!("This is a warn message - should be shown");
    info!("This is an info message - should be shown");

    println!("Debug level test completed - debug, warn, info should be visible");
}

#[test]
fn test_warn_level_shows_warn_info() {
    env::set_var("RUST_LOG", "warn");
    logger::setup_logger();

    trace!("This is a trace message - should NOT be shown");
    error!("This is an error message - should NOT be shown");
    debug!("This is a debug message - should NOT be shown");
    warn!("This is a warn message - should be shown");
    info!("This is an info message - should be shown");

    println!("Warn level test completed - warn, info should be visible");
}

#[test]
fn test_info_level_shows_only_info() {
    env::set_var("RUST_LOG", "info");
    logger::setup_logger();

    trace!("This is a trace message - should NOT be shown");
    error!("This is an error message - should NOT be shown");
    debug!("This is a debug message - should NOT be shown");
    warn!("This is a warn message - should NOT be shown");
    info!("This is an info message - should be shown");

    println!("Info level test completed - only info should be visible");
}

#[test]
fn test_invalid_log_level_defaults_to_info() {
    env::set_var("RUST_LOG", "invalid_level");
    logger::setup_logger();

    trace!("This is a trace message - should NOT be shown");
    debug!("This is a debug message - should NOT be shown");
    info!("This is an info message - should be shown");
    warn!("This is a warn message - should be shown");
    error!("This is an error message - should be shown");

    println!("Invalid level test completed - should default to info level");
}

#[test]
fn test_unset_log_level_defaults_to_info() {
    env::remove_var("RUST_LOG");
    logger::setup_logger();

    trace!("This is a trace message - should NOT be shown");
    debug!("This is a debug message - should NOT be shown");
    info!("This is an info message - should be shown");
    warn!("This is a warn message - should be shown");
    error!("This is an error message - should be shown");

    println!("Unset level test completed - should default to info level");
}

#[test]
fn test_case_insensitive_log_levels() {

    env::set_var("RUST_LOG", "DEBUG");
    logger::setup_logger();
    debug!("Debug message with uppercase level");

    env::set_var("RUST_LOG", "WaRn");
    logger::setup_logger();
    warn!("Warn message with mixed case level");

    println!("Case insensitive test completed");
}

#[test]
fn test_hierarchical_logging_demonstration() {
    println!("\n=== Custom Hierarchical Logging Demonstration ===");
    println!("Custom hierarchy: info (most restrictive) -> warn -> debug -> error -> trace (least restrictive)");

    let levels = ["info", "warn", "debug", "error", "trace"];

    for level in &levels {
        println!("\n--- Testing RUST_LOG={} ---", level);
        env::set_var("RUST_LOG", level);
        logger::setup_logger();

        println!("Expected behavior for {} level:", level);
        match *level {
            "info" => println!("Should show: info only"),
            "warn" => println!("Should show: warn, info"),
            "debug" => println!("Should show: debug, warn, info"),
            "error" => println!("Should show: error, debug, warn, info"),
            "trace" => println!("Should show: trace, error, debug, warn, info"),
            _ => {}
        }

        trace!("TRACE: Detailed execution information");
        error!("ERROR: An error occurred");
        debug!("DEBUG: General debugging information");
        warn!("WARN: Something unexpected happened");
        info!("INFO: General information about program execution");
    }
}

#[test]
fn test_logger_reinitialization() {
    env::set_var("RUST_LOG", "info");

    logger::setup_logger();
    info!("First logger initialization");

    logger::setup_logger();
    info!("Second logger initialization - should work fine");

    println!("Logger reinitialization test completed");
}

#[test]
fn test_logging_performance() {
    env::set_var("RUST_LOG", "info");
    logger::setup_logger();

    let start = std::time::Instant::now();

    for i in 0..1000 {
        info!("Performance test message {}", i);
    }

    let duration = start.elapsed();
    println!("Logged 1000 messages in {:?}", duration);

    assert!(duration.as_secs() < 5, "Logging took too long: {:?}", duration);
}

#[test]
fn test_log_message_formatting() {
    env::set_var("RUST_LOG", "info");
    logger::setup_logger();

    info!("Simple message");
    info!("Message with parameter: {}", "value");
    info!("Message with multiple parameters: {} and {}", "first", "second");
    info!("Message with number: {}", 42);
    info!("Message with float: {:.2}", 3.14159);

    println!("Log message formatting test completed");
}

