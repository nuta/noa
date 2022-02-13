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
pub enum ToClient {
    Notification(Notification),
    Response { id: RequestId, body: Response },
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Response {
    Ok { results: serde_json::Value },
    Err { reason: String },
}

pub mod results {
    use lsp_types::CompletionItem;

    use super::*;

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub struct NoContent;

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub struct Completion {
        items: Vec<CompletionItem>,
    }
}
