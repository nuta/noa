use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use lsp_types::{
    notification::{DidChangeTextDocument, DidOpenTextDocument},
    request::{Completion, Initialize, Request},
    CompletionParams, CompletionResponse, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, PartialResultParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentPositionParams, VersionedTextDocumentIdentifier, WorkDoneProgressParams,
};
use noa_buffer::Point;
use noa_common::syncd_protocol::{LspNotification, LspRequest, LspResponse, Notification};
use tokio::{
    io::BufReader,
    process::{ChildStdin, ChildStdout, Command},
    sync::{mpsc::UnboundedSender, oneshot, Mutex},
};

use crate::eventloop::Daemon;

fn parse_path_as_uri(path: &Path) -> lsp_types::Url {
    let uri = &format!("file://{}", path.to_str().unwrap());
    lsp_types::Url::parse(uri).unwrap()
}

fn serialize_lsp_request<T: lsp_types::request::Request>(
    id: jsonrpc_core::Id,
    params: T::Params,
) -> String {
    let obj = match serde_json::to_value(params) {
        Ok(serde_json::value::Value::Object(obj)) => obj,
        _ => unreachable!(),
    };

    let msg = &jsonrpc_core::types::request::MethodCall {
        id,
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

type ReqIdTxMap = Arc<
    Mutex<
        HashMap<
            jsonrpc_core::Id,
            (
                &'static str, /* request method */
                oneshot::Sender<LspResponse>,
            ),
        >,
    >,
>;

async fn receive_responses(
    req_id_tx_map: ReqIdTxMap,
    _clients: UnboundedSender<Notification>,
    stdout: ChildStdout,
) {
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
        match serde_json::from_str::<jsonrpc_core::Output>(&body) {
            Ok(jsonrpc_core::Output::Success(json)) => {
                // Received a response to a request.
                let (request_method, tx) = req_id_tx_map
                    .lock()
                    .await
                    .remove(&json.id)
                    .expect("dangling response id from the LSP server");

                let resp = match request_method {
                    Completion::METHOD => {
                        let _params: CompletionResponse =
                            serde_json::from_value(json.result).unwrap();
                        // TODO:
                        LspResponse::NoContent
                    }
                    _ => {
                        warn!("ignored unsupported response: {}", body);
                        LspResponse::NoContent
                    }
                };

                tx.send(resp).unwrap();
            }
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
        }
    }
}

trait IntoPosition {
    fn into_position(self) -> lsp_types::Position;
}

impl IntoPosition for Point {
    fn into_position(self) -> lsp_types::Position {
        lsp_types::Position {
            line: self.y as u32,
            character: self.x as u32,
        }
    }
}

pub struct LspDaemon {
    workspace_dir: PathBuf,
    lsp_stdin: ChildStdin,
    lang: String,
    next_req_id: usize,
    req_id_tx_map: ReqIdTxMap,
}

impl LspDaemon {
    pub async fn spawn(
        clients: UnboundedSender<Notification>,
        workspace_dir: &Path,
        lang: String,
    ) -> Result<LspDaemon> {
        let mut lsp_server = Command::new("/usr/bin/clangd")
            .args(&["-j=8", "--log=verbose", "--pretty"])
            .current_dir(workspace_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| {
                format!(
                    "failed to spawn LSP server for {} (have you installed required packages?)",
                    lang
                )
            })?;

        let lsp_stdin = lsp_server.stdin.take().unwrap();
        let lsp_stdout = lsp_server.stdout.take().unwrap();
        let req_id_tx_map = Arc::new(Mutex::new(HashMap::new()));
        {
            let req_id_tx_map = req_id_tx_map.clone();
            tokio::spawn(
                async move { receive_responses(req_id_tx_map, clients, lsp_stdout).await },
            );
        }

        Ok(LspDaemon {
            workspace_dir: workspace_dir.to_path_buf(),
            lsp_stdin,
            lang,
            next_req_id: 1,
            req_id_tx_map,
        })
    }

    pub async fn initialize(&mut self) -> Result<()> {
        self.send_request::<Initialize>(
            // `root_path` is deprecated. We already use root_uri instead.
            #[allow(deprecated)]
            InitializeParams {
                process_id: None,
                root_path: None,
                root_uri: Some(parse_path_as_uri(&self.workspace_dir)),
                locale: None,
                initialization_options: None,
                capabilities: lsp_types::ClientCapabilities {
                    workspace: None,
                    text_document: None,
                    window: None,
                    experimental: None,
                    general: None,
                },
                trace: None,
                workspace_folders: None,
                client_info: None,
            },
        )
        .await?;

        Ok(())
    }

    async fn send_message(&mut self, body: &str) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        self.lsp_stdin
            .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
            .await?;
        self.lsp_stdin.write_all(body.as_bytes()).await?;

        Ok(())
    }

    async fn send_request<T: lsp_types::request::Request>(
        &mut self,
        params: T::Params,
    ) -> Result<LspResponse> {
        let (id, rx) = self.alloc_req_id(T::METHOD).await;
        let body = serialize_lsp_request::<T>(id, params);
        self.send_message(&body).await?;
        trace!("Waiting for response....");
        Ok(rx.await?)
    }

    async fn alloc_req_id(
        &mut self,
        method_name: &'static str,
    ) -> (jsonrpc_core::Id, oneshot::Receiver<LspResponse>) {
        let req_id = jsonrpc_core::Id::Num(self.next_req_id as u64);
        self.next_req_id += 1;

        let (tx, rx) = oneshot::channel();
        self.req_id_tx_map
            .lock()
            .await
            .insert(req_id.clone(), (method_name, tx));
        (req_id, rx)
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
                trace!("DidOpenTextDocument(path={})", path.display());
                let body =
                    serialize_lsp_notification::<DidOpenTextDocument>(DidOpenTextDocumentParams {
                        text_document: lsp_types::TextDocumentItem {
                            uri: parse_path_as_uri(&path),
                            language_id: self.lang.clone(),
                            version: 0,
                            text,
                        },
                    });

                self.send_message(&body).await?;
                Ok(LspResponse::NoContent)
            }
            LspRequest::UpdateFile {
                path,
                text,
                version,
            } => {
                trace!("DidChangeTextDocument(path={})", path.display());
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

                self.send_message(&body).await?;
                Ok(LspResponse::NoContent)
            }
            LspRequest::Completion { path, position } => {
                trace!("Completion(path={}, position={})", path.display(), position);
                self.send_request::<Completion>(CompletionParams {
                    text_document_position: TextDocumentPositionParams {
                        position: position.into_position(),
                        text_document: TextDocumentIdentifier {
                            uri: parse_path_as_uri(&path),
                        },
                    },
                    context: None,
                    partial_result_params: PartialResultParams {
                        partial_result_token: None,
                    },
                    work_done_progress_params: WorkDoneProgressParams {
                        work_done_token: None,
                    },
                })
                .await
            }
        }
    }
}

unsafe impl Send for LspDaemon {}
