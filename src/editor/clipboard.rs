use std::io::prelude::*;
use std::process::{Command, Stdio};

use anyhow::{bail, Result};
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use which::which;

/// Represents data in the clipboard with more detailed contexts.
#[derive(Clone, Debug)]
pub struct ClipboardData {
    /// The texts copied into the clipboard. It's more than one if multiple
    /// cursors were selected.
    pub texts: Vec<String>,
}

impl ClipboardData {
    pub fn from_buffer(buffer: &Buffer) -> ClipboardData {
        let mut texts = Vec::new();
        for c in doc.buffer().cursors() {
            texts.push(doc.buffer().substr(c.selection()));
        }

        ClipboardData { texts }
    }

    pub fn equals_to_str(&self, text: &str) -> bool {
        self.texts.join("\n") == text
    }

    pub fn texts(&self) -> &[String] {
        &self.texts
    }

    fn write_all<W>(&self, writer: &mut W) -> Result<()>
    where
        W: Write + Unpin,
    {
        writer.write_all(self.to_string().as_bytes())?;
        Ok(())
    }
}

impl ToString for ClipboardData {
    fn to_string(&self) -> String {
        self.texts.join("\n")
    }
}

impl Default for ClipboardData {
    fn default() -> Self {
        ClipboardData {
            texts: vec!["".to_string()],
        }
    }
}

#[derive(Clone, Debug)]
pub enum SystemClipboardData {
    Ours(ClipboardData),
    Others(String),
}

pub trait ClipboardProvider {
    fn copy_from_clipboard(&self) -> Result<SystemClipboardData>;
    fn copy_into_clipboard(&self, data: ClipboardData) -> Result<()>;
}

static LAST_OUR_DATA: Lazy<Mutex<ClipboardData>> =
    Lazy::new(|| Mutex::new(ClipboardData::default()));

/// Uses Cmd + V (or Ctrl + Shift + V in some Linux's terminal) to paste
/// contents and uses the OSC52 escape sequence to copy contents into the
/// system's clipboard.
///
/// This is quite useful when you're using noa in remote.
struct Osc52Provider;

impl Osc52Provider {
    fn probe() -> Option<Osc52Provider> {
        Some(Osc52Provider)
    }
}

impl ClipboardProvider for Osc52Provider {
    // User should not use this: they should use Cmd + V to paste from the
    // clipboard instead.
    fn copy_from_clipboard(&self) -> Result<SystemClipboardData> {
        // Use LAST_OUR_DATA as clipboard.
        Ok(SystemClipboardData::Ours(LAST_OUR_DATA.lock().clone()))
    }

    fn copy_into_clipboard(&self, data: ClipboardData) -> Result<()> {
        use std::io::Write;

        let mut stdout = std::io::stdout();

        // OSC52
        write!(
            stdout,
            "\x1b]52;c;{}\x07",
            base64::encode(&data.to_string())
        )
        .ok();
        stdout.flush().ok();

        Ok(())
    }
}

struct MacOsProvider;

impl MacOsProvider {
    fn probe() -> Option<MacOsProvider> {
        if which("pbcopy").is_err() || which("pbcopy").is_err() {
            return None;
        }

        Some(MacOsProvider)
    }
}

impl ClipboardProvider for MacOsProvider {
    fn copy_from_clipboard(&self) -> Result<SystemClipboardData> {
        let mut child = Command::new("pbpaste").stdout(Stdio::piped()).spawn()?;

        let mut stdout = child.stdout.take().unwrap();
        let mut buf = String::new();
        stdout.read_to_string(&mut buf)?;

        Ok(get_last_clipboard_data(&buf)
            .map(SystemClipboardData::Ours)
            .unwrap_or_else(|| SystemClipboardData::Others(buf)))
    }

    fn copy_into_clipboard(&self, data: ClipboardData) -> Result<()> {
        let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        data.write_all(&mut stdin)?;

        save_last_clipboard_data(data);
        Ok(())
    }
}

struct DummyProvider;

impl ClipboardProvider for DummyProvider {
    fn copy_from_clipboard(&self) -> Result<SystemClipboardData> {
        // Use LAST_OUR_DATA as clipboard.
        Ok(SystemClipboardData::Ours(LAST_OUR_DATA.lock().clone()))
    }

    fn copy_into_clipboard(&self, data: ClipboardData) -> Result<()> {
        // Use LAST_OUR_DATA as clipboard.
        *LAST_OUR_DATA.lock() = data;
        Ok(())
    }
}

pub fn build_provider() -> Option<Box<dyn ClipboardProvider>> {
    if std::env::var("SSH_CONNECTION").is_ok() {
        if let Some(provider) = Osc52Provider::probe() {
            return Some(Box::new(provider));
        }
    }

    if cfg!(target_os = "macos") {
        if let Some(provider) = MacOsProvider::probe() {
            return Some(Box::new(provider));
        }
    }

    // No clipboard provider found.
    None
}

pub fn build_dummy_provider() -> Box<dyn ClipboardProvider> {
    Box::new(DummyProvider)
}

/// Returns `ClipboardData` if `text` matches to the lastly pasted data.
pub fn get_last_clipboard_data(text: &str) -> Option<ClipboardData> {
    let last_data = LAST_OUR_DATA.lock();
    if last_data.equals_to_str(text) {
        Some(last_data.clone())
    } else {
        None
    }
}

fn save_last_clipboard_data(data: ClipboardData) {
    *LAST_OUR_DATA.lock() = data;
}
