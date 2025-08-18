use fern::colors::{Color, ColoredLevelConfig};
use log::{Level, LevelFilter};
use std::env;

pub fn setup_logger() {
    let colors = ColoredLevelConfig {
        trace: Color::Cyan,
        debug: Color::Magenta,
        info: Color::Green,
        warn: Color::Red,
        error: Color::BrightRed,
    };

    // Parse the RUST_LOG environment variable to determine the exact log level to show
    let target_level = env::var("RUST_LOG")
        .ok()
        .and_then(|level_str| match level_str.to_lowercase().as_str() {
            "trace" => Some(Level::Trace),
            "debug" => Some(Level::Debug),
            "info" => Some(Level::Info),
            "warn" => Some(Level::Warn),
            "error" => Some(Level::Error),
            _ => None,
        })
        .unwrap_or(Level::Info); // Default to Info if RUST_LOG is not set or invalid

    let result = fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}] {}",
                chrono::Local::now().format("[%H:%M:%S]"),
                colors.color(record.level()),
                message
            ))
        })
        .chain(std::io::stdout())
        .level(LevelFilter::Trace) // Allow all levels to pass through to the filter
        .filter(move |metadata| {
            // Only show logs that match the exact target level
            metadata.level() == target_level
        })
        .apply();
        
    match result {
        Ok(_) => (),
        Err(_) => {
            // Logger already initialized, which is fine for tests
            eprintln!("Logger already initialized (this is normal for tests)");
        }
    }
}
