use std::fs::OpenOptions;

use backtrace::Backtrace;
use log::LevelFilter;
use simplelog::{Config, WriteLogger};

use crate::dirs::log_file_path;

pub fn install_logger(name: &str) {
    WriteLogger::init(
        LevelFilter::Trace,
        Config::default(),
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(log_file_path(name))
            .unwrap(),
    )
    .unwrap();
    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        prettify_backtrace(backtrace::Backtrace::new());
    }));
}

pub fn prettify_backtrace(backtrace: Backtrace) {
    for (i, frame) in backtrace.frames().iter().enumerate() {
        for symbol in frame.symbols() {
            if let Some(path) = symbol.filename() {
                let filename = path.to_str().unwrap_or("(non-utf8 path)");
                if filename.contains("/.rustup/")
                    || filename.contains("/.cargo/")
                    || filename.starts_with("/rustc/")
                {
                    continue;
                }

                error!(
                    "    #{} {}:{}, col {}",
                    i,
                    filename,
                    symbol.lineno().unwrap_or(0),
                    symbol.colno().unwrap_or(0),
                );
            }
        }
    }
}

pub fn backtrace() {
    prettify_backtrace(backtrace::Backtrace::new());
}
