use std::path::PathBuf;

use lsp_types::{CompletionItem, Diagnostic, HoverContents, SignatureHelp};
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
    Request {
        id: RequestId,
        body: serde_json::Value,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum LspRequest {
    Completion {
        path: PathBuf,
        position: lsp_types::Position,
    },
    OpenFile {
        path: PathBuf,
        text: String,
    },
    UpdateFile {
        path: PathBuf,
        text: String,
        version: usize,
    },
    Hover {
        path: PathBuf,
        position: lsp_types::Position,
    },
    SignatureHelp {
        path: PathBuf,
        position: lsp_types::Position,
    },
    GoToDefinition {
        path: PathBuf,
        position: lsp_types::Position,
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
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Response {
    Ok { body: serde_json::Value },
    Err { reason: String },
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct FileLocation {
    pub path: PathBuf,
    pub position: lsp_types::Position,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum LspResponse {
    NoContent,
    Completion(Vec<CompletionItem>),
    Hover(Option<HoverContents>),
    SignatureHelp(Option<SignatureHelp>),
    GoToDefinition(Vec<FileLocation>),
}
