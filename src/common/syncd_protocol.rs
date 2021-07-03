use std::path::PathBuf;

use lsp_types::{CompletionItem, Diagnostic};
use serde::{Deserialize, Serialize};

use noa_buffer::Point;

pub use lsp_types;

use crate::fast_hash::FastHash;

#[derive(Deserialize, Serialize, Debug)]
pub enum ToServer<R> {
    Request(RawRequest<R>),
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ToClient<R> {
    Response(RawResponse<R>),
    Notification(Notification),
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RawRequest<T> {
    pub id: usize,
    pub body: T,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RawResponse<T> {
    pub id: usize,
    pub body: T,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum Notification {
    Diagnostics(Vec<Diagnostic>),
    FileModified {
        path: PathBuf,
        text: String,
        hash: FastHash,
    },
}

#[derive(Deserialize, Serialize, Debug)]
pub enum LspRequest {
    OpenFile {
        path: PathBuf,
        text: String,
    },
    UpdateFile {
        path: PathBuf,
        version: usize,
        text: String,
    },
    Completion {
        path: PathBuf,
        position: Point,
    },
    GoToDefinition {
        path: PathBuf,
        position: Point,
    },
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct FileLocation {
    pub path: PathBuf,
    pub pos: Point,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum LspResponse {
    NoContent,
    Completion(Vec<CompletionItem>),
    GoToDefinition(Vec<FileLocation>),
}

#[derive(Deserialize, Serialize, Debug)]
pub enum BufferSyncRequest {
    OpenFile { path: PathBuf },
    UpdateFile { path: PathBuf, text: String },
}

#[derive(Deserialize, Serialize, Debug)]
pub enum BufferSyncResponse {
    NoContent,
}

unsafe impl<T: Send> Send for RawRequest<T> {}
unsafe impl<T: Send> Send for RawResponse<T> {}
unsafe impl Send for Notification {}
unsafe impl Send for LspRequest {}
unsafe impl Send for LspResponse {}
