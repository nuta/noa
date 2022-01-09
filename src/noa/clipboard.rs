use std::process::Stdio;

use anyhow::{bail, Result};
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use which::which;

#[async_trait]
pub trait ClipboardProvider {
    async fn get_text(&self) -> Result<String>;
    async fn set_text(&self, text: &str) -> Result<()>;
}

struct MacOsProvider;

impl MacOsProvider {
    fn probe() -> Option<MacOsProvider> {
        if !which("pbcopy").is_ok() || !which("pbcopy").is_ok() {
            return None;
        }

        Some(MacOsProvider {})
    }
}

#[async_trait]
impl ClipboardProvider for MacOsProvider {
    async fn get_text(&self) -> Result<String> {
        let mut child = Command::new("pbcopy").stdout(Stdio::piped()).spawn()?;

        let mut stdout = child.stdout.take().unwrap();
        let mut buf = String::new();
        stdout.read_to_string(&mut buf).await?;

        Ok(buf)
    }

    async fn set_text(&self, text: &str) -> Result<()> {
        let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(text.as_bytes()).await?;

        Ok(())
    }
}

struct DummyProvider;

#[async_trait]
impl ClipboardProvider for DummyProvider {
    async fn get_text(&self) -> Result<String> {
        bail!("No clipboard provider available");
    }

    async fn set_text(&self, text: &str) -> Result<()> {
        Ok(())
    }
}

pub fn build_provider() -> Option<Box<dyn ClipboardProvider>> {
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
