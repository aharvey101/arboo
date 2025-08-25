use log::LevelFilter;

pub fn setup_test_logger() {
    use fern::colors::{Color, ColoredLevelConfig};

    let colors = ColoredLevelConfig {
        trace: Color::Cyan,
        debug: Color::Blue,
        info: Color::Green,
        warn: Color::Yellow,
        error: Color::Red,
    };

    let result = fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%H:%M:%S]"),
                colors.color(record.level()),
                record.target(),
                message
            ))
        })
        .chain(std::io::stdout())
        .level(LevelFilter::Info)
        .level_for("test", LevelFilter::Debug)
        .apply();

    if let Err(_) = result {

        eprintln!("Logger already initialized");
    }
}

