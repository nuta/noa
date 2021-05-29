use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use lsp_types::{notification::DidOpenTextDocument, DidOpenTextDocumentParams};
use serde::{Deserialize, Serialize};
use tokio::process::{ChildStdin, Command};

use crate::eventloop::Daemon;

#[derive(Deserialize, Debug)]
pub enum Request {
    OpenFile { path: PathBuf, text: String },
    // UpdateFile {
    //     path: PathBuf,
    //     text: String,
    //     version: usize,
    // },
}

#[derive(Serialize, Debug)]
pub enum Response {
    NoContent,
}

fn parse_path_as_uri(path: &Path) -> lsp_types::Url {
    let uri = &format!("file://{}", path.to_str().unwrap());
    lsp_types::Url::parse(uri).unwrap()
}

fn serialize_request<T: lsp_types::request::Request>(id: usize, params: T::Params) -> String {
    let obj = match serde_json::to_value(params) {
        Ok(serde_json::value::Value::Object(obj)) => obj,
        _ => unreachable!(),
    };

    let msg = &jsonrpc_core::types::request::MethodCall {
        id: jsonrpc_core::Id::Num(id as u64),
        jsonrpc: Some(jsonrpc_core::Version::V2),
        method: T::METHOD.to_string(),
        params: jsonrpc_core::Params::Map(obj),
    };

    serde_json::to_string(msg).unwrap()
}

fn serialize_notification<T: lsp_types::notification::Notification>(params: T::Params) -> String {
    let obj = match serde_json::to_value(params) {
        Ok(serde_json::value::Value::Object(obj)) => obj,
        _ => unreachable!(),
    };

    let msg = &jsonrpc_core::types::request::Notification {
        jsonrpc: Some(jsonrpc_core::Version::V2),
        method: T::METHOD.to_string(),
        params: jsonrpc_core::Params::Map(obj),
    };

    serde_json::to_string(msg).unwrap()
}

async fn send_requests(stdin: &mut ChildStdin, body: &str) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    stdin
        .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .await?;
    stdin.write_all(body.as_bytes()).await?;

    Ok(())
}

pub struct LspDaemon {
    lsp_stdin: ChildStdin,
    lang: String,
}

impl LspDaemon {
    pub fn new(workspace_dir: &Path, lang: String) -> Result<LspDaemon> {
        let mut lsp_server = Command::new("clangd")
            .args(&["-j=8", "--log=verbose", "--pretty"])
            .current_dir(workspace_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to spawn LSP server for {}", lang))?;

        let lsp_stdout = lsp_server.stdout.take().unwrap();
        let lsp_stdin = lsp_server.stdin.take().unwrap();
        Ok(LspDaemon { lsp_stdin, lang })
    }
}

#[async_trait]
impl Daemon for LspDaemon {
    type Request = Request;

    async fn process(&mut self, request: Self::Request) -> Result<()> {
        match request {
            Request::OpenFile { path, text } => {
                info!("DidOpenTextDocument(path={})", path.display());
                let body =
                    serialize_notification::<DidOpenTextDocument>(DidOpenTextDocumentParams {
                        text_document: lsp_types::TextDocumentItem {
                            uri: parse_path_as_uri(&path),
                            language_id: self.lang.clone(),
                            version: 0,
                            text,
                        },
                    });

                send_requests(&mut self.lsp_stdin, &body).await?;
            }
        }

        Ok(())
    }
}
