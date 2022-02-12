use std::path::PathBuf;

use lsp_types::Diagnostic;
use noa_common::fast_hash::FastHash;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Serialize, Hash)]
pub struct RequestId(usize);

impl From<usize> for RequestId {
    fn from(id: usize) -> Self {
        Self(id)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ToServer {
    Request { id: RequestId, body: Request },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ToClient {
    Response { id: RequestId, body: Response },
    Notification(Notification),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Request {
    Completion {
        path: PathBuf,
        position: lsp_types::Position,
    },
    UpdateFile {
        path: PathBuf,
        text: String,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Response {
    NoContent,
    Completion(Vec<lsp_types::CompletionItem>),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Notification {
    Diagnostics {
        diags: Vec<Diagnostic>,
        path: PathBuf,
    },
    FileModified {
        path: PathBuf,
        text: String,
        hash: FastHash,
    },
}
