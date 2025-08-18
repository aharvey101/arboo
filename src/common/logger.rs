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

    // Get the target level from environment variable
    let target_level = env::var("RUST_LOG")
        .unwrap_or_else(|_| "info".to_string())
        .to_lowercase();

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
        .level(LevelFilter::Trace) // Allow all levels, we'll filter with custom logic
        .filter(move |metadata| {
            let level = metadata.level();
            
            // Custom hierarchy: info is highest priority (most restrictive)
            // info: shows only info
            // warn: shows warn + info
            // debug: shows debug + warn + info  
            // error: shows error + debug + warn + info
            // trace: shows trace + error + debug + warn + info
            match target_level.as_str() {
                "info" => level == Level::Info,
                "warn" => matches!(level, Level::Warn | Level::Info),
                "debug" => matches!(level, Level::Debug | Level::Warn | Level::Info),
                "error" => matches!(level, Level::Error | Level::Debug | Level::Warn | Level::Info),
                "trace" => matches!(level, Level::Trace | Level::Error | Level::Debug | Level::Warn | Level::Info),
                _ => true, // Default: show all
            }
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
