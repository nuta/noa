use anyhow::Result;
use backtrace::Backtrace;
use log::{Level, LevelFilter};
use std::{
    fmt::Debug,
    io::{BufRead, BufReader, ErrorKind, Seek, SeekFrom, },
    path::Path,
};

use crate::dirs::log_file_path;

const LOG_FILE_LEN_MAX: usize = 256 * 1024;

struct Logger {
}

impl Logger {
    pub fn new() -> Self {
        Logger { }
    }
}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
     }

    fn flush(&self) {

    }

    fn log(&self, record: &log::Record) {
        let color_start = match record.level() {
            Level::Error => "\x1b[1;31m",
            Level::Warn => "\x1b[1;33m",
            _ => "\x1b[34m",
        };
        let color_end = match record.level() {
            Level::Error => "\x1b[1;31m",
            Level::Warn => "\x1b[1;33m",
            _ => "\x1b[0m",
        };
        let prefix = match record.level() {
            Level::Error => " Error:",
            Level::Warn => " Warn:",
            _ => "",
        };
        let filename = record.file().unwrap_or_else(|| record.target());
        let lineno = record.line().unwrap_or(0);

        // self.log_file.write_all(
        //     format!(
        //         "{color_start}[{filename}:{lineno}]{prefix}{color_end} {}\n",
        //         record.args()
        //     )
        //     .as_bytes(),
        // ).unwrap_or_else(|e| {
        //     eprintln!("failed to write to the log file: {}", e);
        // });
    }
}

pub fn install_logger(name: &str) {
    let log_path = log_file_path(name);
    shrink_file(&log_path, LOG_FILE_LEN_MAX).expect("failed to shrink the log file");

    log::set_max_level(if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });
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

fn shrink_file(path: &Path, max_len: usize) -> Result<()> {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    let current_len: usize = meta.len().try_into()?;
    if current_len <= max_len {
        return Ok(());
    }

    let new_len = current_len - max_len;

    // Look for the nearest newline character.
    let mut file = std::fs::OpenOptions::new().read(true).open(path)?;
    file.seek(SeekFrom::Current(new_len.try_into()?))?;
    let mut reader = BufReader::new(file);
    let mut buf = Vec::new();
    reader.read_until(b'\n', &mut buf)?;

    // Copy contents after the newline character and replace the old file.
    let mut new_file = tempfile::NamedTempFile::new()?;
    std::io::copy(&mut reader, &mut new_file)?;
    new_file.persist(path)?;

    Ok(())
}

pub fn backtrace() {
    prettify_backtrace(backtrace::Backtrace::new());
}

pub trait OopsExt: Sized {
    fn oops(self);
}

impl<T, E: Debug> OopsExt for std::result::Result<T, E> {
    fn oops(self) {
        match self {
            Ok(_) => {}
            Err(err)=> {
                warn!("oops: {:?}", err);
                crate::logger::backtrace();
            }
        }
    }
}

#[macro_export]
macro_rules! debug_warn {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        {
            warn!($($arg)*);
        }
    }}
}

#[macro_export]
macro_rules! warn_once {
    ($($arg:tt)*) => {{
        static WARN_ONCE: ::std::sync::Once = ::std::sync::Once::new();
        WARN_ONCE.call_once(|| warn!($($arg)*));
    }}
}

#[macro_export]
macro_rules! debug_warn_once {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        {
            $crate::warn_once!($($arg)*);
        }
    }}
}
