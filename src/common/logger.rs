use fern::colors::{Color, ColoredLevelConfig};
use log::LevelFilter;
use std::env;

pub fn setup_logger() {
    let colors = ColoredLevelConfig {
        trace: Color::Cyan,
        debug: Color::Magenta,
        info: Color::Green,
        warn: Color::Red,
        error: Color::BrightRed,
    };

    // Parse the RUST_LOG environment variable to determine the log level filter
    let target_level_filter = env::var("RUST_LOG")
        .ok()
        .and_then(|level_str| match level_str.to_lowercase().as_str() {
            "trace" => Some(LevelFilter::Trace),
            "debug" => Some(LevelFilter::Debug),
            "info" => Some(LevelFilter::Info),
            "warn" => Some(LevelFilter::Warn),
            "error" => Some(LevelFilter::Error),
            _ => None,
        })
        .unwrap_or(LevelFilter::Info); // Default to Info if RUST_LOG is not set or invalid

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
        .level(target_level_filter) // Use hierarchical filtering (shows target level and higher priority levels)
        .apply();
        
    match result {
        Ok(_) => (),
        Err(_) => {
            // Logger already initialized, which is fine for tests
            eprintln!("Logger already initialized (this is normal for tests)");
        }
    }
}
