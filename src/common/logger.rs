use fern::colors::{Color, ColoredLevelConfig};
use log::LevelFilter;

pub fn setup_logger() {
    let colors = ColoredLevelConfig {
        trace: Color::Cyan,
        debug: Color::Magenta,
        info: Color::Green,
        warn: Color::Red,
        error: Color::BrightRed,
        ..ColoredLevelConfig::new()
    };

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}] {}",
                chrono::Local::now().format("[%H:%M:%S]"),
                colors.color(record.level()),
                message
            ))
        })
        .chain(std::io::stdout())
        .level(log::LevelFilter::Error)
        .level_for("arbooo", LevelFilter::Info)
        .level_for("arbooo", LevelFilter::Debug)
        .apply()
        .expect("Shouldn't have failed to setup the logger");
}
