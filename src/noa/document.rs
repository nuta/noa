use std::path::{Path, PathBuf};

use anyhow::Result;

use noa_buffer::buffer::Buffer;
use noa_languages::language::Language;

pub struct Document {
    /// It's `None` if the document is not backed by a file (e.g. a scrach buffer).
    path: Option<PathBuf>,
    name: String,
    buffer: Buffer,
    lang: &'static Language,
}

impl Document {
    pub fn new(name: &str) -> Result<Document> {
        unimplemented!()
    }

    pub fn open_file(path: &Path) -> Result<Document> {
        unimplemented!()
    }

    pub fn save_to_file(&self) -> Result<()> {
        if let Some(ref path) = self.path {
            self.buffer.save_to_file(path)?;
        }

        Ok(())
    }
}

pub struct DocumentManager {}

impl DocumentManager {
    pub fn new() -> DocumentManager {
        DocumentManager {}
    }
}
