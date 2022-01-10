use std::path::PathBuf;

use lsp_types::Diagnostic;
use noa_common::fast_hash::FastHash;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ToServer {
    Request { id: usize, body: Request },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ToClient {
    Response { id: usize, body: Response },
    Notification(Notification),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Request {
    UpdateFile { path: PathBuf, text: String },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Response {
    NoContent,
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
