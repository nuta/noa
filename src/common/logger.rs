use anyhow::Result;
use backtrace::Backtrace;
use log::Level;
use std::{
    fmt::Debug,
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::Path,
};

use crate::dirs::log_file_path;

pub fn install_logger(name: &str) {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{color_start}[{filename}:{lineno}] {prefix}{color_end}{message}\x1b[0m",
                color_start = match record.level() {
                    Level::Error => "\x1b[1;31m",
                    Level::Warn => "\x1b[1;33m",
                    _ => "\x1b[34m",
                },
                color_end = match record.level() {
                    Level::Error => "\x1b[1;31m",
                    Level::Warn => "\x1b[1;33m",
                    _ => "\x1b[0m",
                },
                prefix = match record.level() {
                    Level::Error => "Error: ",
                    Level::Warn => "Warn: ",
                    _ => "",
                },
                filename = record.file().unwrap_or_else(|| record.target()),
                lineno = record.line().unwrap_or(0),
                message = message
            ))
        })
        .level(if cfg!(debug_assertions) {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Info
        })
        .chain(fern::log_file(log_file_path(name)).unwrap())
        .apply()
        .expect("failed to initialize the logger");

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

pub fn shrink_file(path: &Path, max_len: usize) -> Result<()> {
    let meta = std::fs::metadata(path)?;
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
    fn oops_with_reason(self, reason: &str);

    fn oops(self) {
        self.oops_with_reason("");
    }

    fn oops_with<F: FnOnce() -> String>(self, reason: F) {
        self.oops_with_reason(&reason())
    }
}

impl<T, E: Debug> OopsExt for std::result::Result<T, E> {
    fn oops_with_reason(self, reason: &str) {
        match self {
            Ok(_) => {}
            Err(err) if reason.is_empty() => {
                warn!("oops: {:?}", err);
                crate::logger::backtrace();
            }
            Err(err) => {
                warn!("oops: {}: {:?}", reason, err);
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
