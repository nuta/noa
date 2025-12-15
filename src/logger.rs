use std::fs::OpenOptions;
use std::fs::File;
use std::io::{ Write};
use std::sync::Mutex;

use log::{LevelFilter, Log, Metadata, Record};

const MAX_LOG_FILE_SIZE: u64 = 1024 * 1024 * 10;

/// A file-based logger.
pub struct Logger {
    file: Mutex<File>,
}

impl Logger {
    pub fn new(file: File) -> Self {
        Self { file: Mutex::new(file) }
    }
}

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let mut file = self.file.lock().unwrap();
            let _ = writeln!(&mut file, "{}", record.args());
        }
    }

    fn flush(&self) {
        let mut file = self.file.lock().unwrap();
        let _ = file.flush();
    }
}

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let home_dir = std::env::home_dir().expect("failed to get home directory");
    let log_path = home_dir.join(".noa.log");
    let file = OpenOptions::new().create(true).append(true).open(log_path).expect("failed to open log file");

    // If the log file istoo big, truncate it.
    if file.metadata().unwrap().len() > MAX_LOG_FILE_SIZE {
        let _ = file.set_len(0);
    }

    let logger = Logger::new(file);
    log::set_boxed_logger(Box::new(logger)).expect("failed to set logger");
    log::set_max_level(LevelFilter::Trace);
    Ok(())
}
