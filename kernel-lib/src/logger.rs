use log::{Level, LevelFilter, Log, Metadata, Record};

static mut LOGGER: Option<Logger> = None;

struct Logger;
impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let level_str = match record.level() {
            Level::Error => "[ERROR]",
            Level::Warn => "[WARN ]",
            Level::Info => "[INFO ]",
            Level::Debug => "[DEBUG]",
            Level::Trace => "[TRACE]",
        };
        crate::console::_print(format_args!("{} ", level_str));
        crate::console::_print(*record.args());
        crate::console::_print(format_args!("\n"));
    }

    fn flush(&self) {
        // noop
    }
}

pub type LogLevel = LevelFilter;

/// Initialize the logger.
/// [`console::init_global_console`] must be called in advance.
pub fn init_logger(level: LogLevel) {
    log::set_logger(unsafe {
        LOGGER = Some(Logger);
        LOGGER.as_ref().unwrap()
    })
    .expect("Failed to set logger");
    log::set_max_level(level);
}
