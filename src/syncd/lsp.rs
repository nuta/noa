use std::{collections::HashMap, path::Path, process::Stdio};

use anyhow::{Context, Result};
use async_trait::async_trait;
use lsp_types::{
    notification::{DidChangeTextDocument, DidOpenTextDocument},
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, TextDocumentContentChangeEvent,
    VersionedTextDocumentIdentifier,
};
use noa_common::syncd_protocol::{LspNotification, LspRequest, LspResponse};
use tokio::{
    io::BufReader,
    process::{ChildStdin, ChildStdout, Command},
    sync::mpsc::UnboundedSender,
};

use crate::eventloop::Daemon;

fn parse_path_as_uri(path: &Path) -> lsp_types::Url {
    let uri = &format!("file://{}", path.to_str().unwrap());
    lsp_types::Url::parse(uri).unwrap()
}

fn serialize_lsp_request<T: lsp_types::request::Request>(id: usize, params: T::Params) -> String {
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

fn serialize_lsp_notification<T: lsp_types::notification::Notification>(
    params: T::Params,
) -> String {
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

async fn receive_responses(_clients: UnboundedSender<LspNotification>, stdout: ChildStdout) {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};

    let mut reader = BufReader::new(stdout);
    loop {
        // Read headers.
        let mut headers = HashMap::new();
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    warn!("failed to read from the LSP server: EOF");
                    return;
                }
                Ok(_) => {}
                Err(err) => {
                    warn!("failed to read from the LSP server: {}", err);
                    return;
                }
            }

            if line.trim().is_empty() {
                break;
            }

            let words: Vec<&str> = line.split(':').collect();
            if words.len() != 2 {
                warn!("malformed LSP header: '{}'", line);
                continue;
            }

            headers.insert(words[0].trim().to_owned(), words[1].trim().to_owned());
        }

        // Parse Content-Length.
        let len = match headers
            .get("Content-Length")
            .and_then(|value| value.parse::<usize>().ok())
        {
            Some(len) => len,
            None => {
                warn!("missing valid LSP Content-Length header");
                continue;
            }
        };

        // Read the content.
        let mut buf = vec![0; len];
        let body = match reader.read_exact(&mut buf).await {
            Ok(_) => String::from_utf8(buf).unwrap(),
            Err(err) => {
                warn!("failed to read from the LSP server: {}", err);
                continue;
            }
        };

        // Parse the JSON.
        trace!("body = '{}'", body);
        let resp = match serde_json::from_str::<jsonrpc_core::Output>(&body) {
            Ok(jsonrpc_core::Output::Success(json)) => json,
            Ok(jsonrpc_core::Output::Failure(failure)) => {
                warn!("LSP: {:?}", failure);
                continue;
            }
            Err(_) => {
                // Perhaps it is a notification from the server.
                match serde_json::from_str::<jsonrpc_core::Request>(&body) {
                    Ok(jsonrpc_core::Request::Single(req)) => {
                        trace!("request from server = {:?}", req);
                        continue;
                    }
                    Ok(jsonrpc_core::Request::Batch(reqs)) => {
                        trace!("request from server = {:?}", reqs);
                        continue;
                    }
                    Err(err) => {
                        warn!("failed to parse the body from the LSP server: {}", err);
                        continue;
                    }
                }
            }
        };

        trace!("LSP: {:#?}", resp);
    }
}

pub struct LspDaemon {
    lsp_stdin: ChildStdin,
    lang: String,
}

impl LspDaemon {
    pub async fn spawn(
        clients: UnboundedSender<LspNotification>,
        workspace_dir: &Path,
        lang: String,
    ) -> Result<LspDaemon> {
        let mut lsp_server = Command::new("clangd")
            .args(&["-j=8", "--log=verbose", "--pretty"])
            .current_dir(workspace_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to spawn LSP server for {}", lang))?;

        let lsp_stdin = lsp_server.stdin.take().unwrap();
        let lsp_stdout = lsp_server.stdout.take().unwrap();
        tokio::spawn(async move { receive_responses(clients, lsp_stdout).await });

        Ok(LspDaemon { lsp_stdin, lang })
    }
}

#[async_trait]
impl Daemon for LspDaemon {
    type Request = LspRequest;
    type Response = LspResponse;
    type Notification = LspNotification;

    async fn process_request(&mut self, request: Self::Request) -> Result<Self::Response> {
        match request {
            LspRequest::OpenFile { path, text } => {
                info!("DidOpenTextDocument(path={})", path.display());
                let body =
                    serialize_lsp_notification::<DidOpenTextDocument>(DidOpenTextDocumentParams {
                        text_document: lsp_types::TextDocumentItem {
                            uri: parse_path_as_uri(&path),
                            language_id: self.lang.clone(),
                            version: 0,
                            text,
                        },
                    });

                send_requests(&mut self.lsp_stdin, &body).await?;
                Ok(LspResponse::NoContent)
            }
            LspRequest::UpdateFile {
                path,
                text,
                version,
            } => {
                info!("DidChangeTextDocument(path={})", path.display());
                let body = serialize_lsp_notification::<DidChangeTextDocument>(
                    DidChangeTextDocumentParams {
                        text_document: VersionedTextDocumentIdentifier {
                            uri: parse_path_as_uri(&path),
                            version: version as i32,
                        },
                        content_changes: vec![TextDocumentContentChangeEvent {
                            range: None,
                            range_length: None,
                            text,
                        }],
                    },
                );

                send_requests(&mut self.lsp_stdin, &body).await?;
                Ok(LspResponse::NoContent)
            }
        }
    }
}

unsafe impl Send for LspDaemon {}
