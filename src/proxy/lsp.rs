use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use jsonrpc_core::Call;
use lsp_types::{
    notification::{
        DidChangeTextDocument, DidOpenTextDocument, Initialized,
        Notification as LspNotificationTrait, PublishDiagnostics,
    },
    request::{Completion, Formatting, GotoDefinition, HoverRequest, Initialize, Request},
    CompletionParams, CompletionResponse, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams,
    InitializeParams, InitializedParams, PartialResultParams, PublishDiagnosticsParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentPositionParams, TextEdit,
    VersionedTextDocumentIdentifier, WorkDoneProgressParams,
};

use noa_languages::Lsp;
use tokio::{
    io::BufReader,
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{mpsc::UnboundedSender, oneshot, Mutex},
};

use crate::{
    protocol::{FileLocation, LspRequest, LspResponse, Notification},
    server::Server,
};

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
    notification_tx: UnboundedSender<Notification>,
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
        match serde_json::from_str::<jsonrpc_core::Output>(&body) {
            Ok(jsonrpc_core::Output::Success(json)) => {
                // Received a response to a request.
                let (request_method, tx) = req_id_tx_map
                    .lock()
                    .await
                    .remove(&json.id)
                    .expect("dangling response id from the LSP server");
                trace!("request_method={:?}", request_method);
                let resp = match request_method {
                    Completion::METHOD => {
                        let resp: CompletionResponse = serde_json::from_value(json.result).unwrap();
                        let items = match resp {
                            CompletionResponse::Array(items) => items,
                            CompletionResponse::List(list) => list.items,
                        };
                        LspResponse::Completion(items)
                    }
                    Formatting::METHOD => {
                        let resp: Option<Vec<TextEdit>> =
                            serde_json::from_value(json.result).unwrap();
                        LspResponse::Edits(resp.unwrap_or_default())
                    }
                    HoverRequest::METHOD => {
                        let contents = serde_json::from_value::<Hover>(json.result)
                            .ok()
                            .map(|resp| resp.contents);
                        LspResponse::Hover(contents)
                    }
                    GotoDefinition::METHOD => {
                        let resp: GotoDefinitionResponse =
                            serde_json::from_value(json.result).unwrap();
                        let items = match resp {
                            GotoDefinitionResponse::Array(items) => items,
                            GotoDefinitionResponse::Scalar(item) => vec![item],
                            GotoDefinitionResponse::Link(links) => {
                                warn!("GotoDefinitionResponse::Link is not supported: {:?}", links);
                                continue;
                            }
                        };

                        let items = items
                            .iter()
                            .map(|loc| FileLocation {
                                path: PathBuf::from(loc.uri.path()),
                                position: loc.range.start,
                            })
                            .collect();
                        LspResponse::GoToDefinition(items)
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
                    Ok(jsonrpc_core::Request::Single(Call::Notification(noti))) => {
                        trace!("notification from server = {:?}", noti);
                        match noti.method.as_str() {
                            PublishDiagnostics::METHOD => {
                                let resp: PublishDiagnosticsParams = noti.params.parse().unwrap();
                                notification_tx
                                    .send(Notification::Diagnostics {
                                        path: PathBuf::from(resp.uri.path()),
                                        diags: resp.diagnostics,
                                    })
                                    .unwrap();
                            }
                            _ => {
                                trace!("ignored notification: {}", noti.method);
                            }
                        }
                        continue;
                    }
                    Ok(jsonrpc_core::Request::Single(call)) => {
                        trace!("RPC from server = {:?}", call);
                        continue;
                    }
                    Ok(jsonrpc_core::Request::Batch(reqs)) => {
                        trace!("batch from server = {:?}", reqs);
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

pub struct LspServer {
    language_id: String,
    workspace_dir: PathBuf,
    lsp_stdin: ChildStdin,
    _lsp_server: Child,
    next_req_id: usize,
    req_id_tx_map: ReqIdTxMap,
}

impl LspServer {
    pub async fn spawn(
        name: String,
        notification_tx: UnboundedSender<Notification>,
        lsp_config: &'static Lsp,
        workspace_dir: &Path,
    ) -> Result<LspServer> {
        trace!("spawning lsp server {} ({})", name, workspace_dir.display());
        let argv = &lsp_config.argv;
        let envp: Vec<(&str, &str)> = lsp_config.envp.iter().map(|(k, v)| (*k, *v)).collect();

        let mut lsp_server = Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(workspace_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .envs(envp)
            .kill_on_drop(true)
            .spawn()
            .with_context(|| {
                format!(
                    "failed to spawn LSP server for {} (have you installed required packages?)",
                    name
                )
            })?;

        let lsp_stdin = lsp_server.stdin.take().unwrap();
        let lsp_stdout = lsp_server.stdout.take().unwrap();
        let req_id_tx_map = Arc::new(Mutex::new(HashMap::new()));
        {
            let req_id_tx_map = req_id_tx_map.clone();
            tokio::spawn(async move {
                receive_responses(req_id_tx_map, notification_tx, lsp_stdout).await
            });
        }

        Ok(LspServer {
            language_id: name,
            workspace_dir: workspace_dir.to_path_buf(),
            lsp_stdin,
            _lsp_server: lsp_server,
            next_req_id: 1,
            req_id_tx_map,
        })
    }

    pub async fn initialize(&mut self) -> Result<()> {
        let capabilities = lsp_types::ClientCapabilities {
            workspace: None,
            text_document: Some(lsp_types::TextDocumentClientCapabilities {
                completion: Some(lsp_types::CompletionClientCapabilities {
                    completion_item: Some(lsp_types::CompletionItemCapability {
                        insert_replace_support: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                hover: Some(lsp_types::HoverClientCapabilities {
                    content_format: Some(vec![
                        lsp_types::MarkupKind::Markdown,
                        lsp_types::MarkupKind::PlainText,
                    ]),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            window: None,
            experimental: None,
            general: None,
        };

        self.call_method::<Initialize>(
            // `root_path` is deprecated. We already use root_uri instead.
            #[allow(deprecated)]
            InitializeParams {
                process_id: None,
                root_path: None,
                root_uri: Some(parse_path_as_uri(&self.workspace_dir)),
                locale: None,
                initialization_options: None,
                capabilities,
                trace: None,
                workspace_folders: None,
                client_info: None,
            },
        )
        .await?;

        self.send_notification::<Initialized>(InitializedParams {})
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

    async fn call_method<T: lsp_types::request::Request>(
        &mut self,
        params: T::Params,
    ) -> Result<LspResponse> {
        let (id, rx) = self.alloc_req_id(T::METHOD).await;
        let req = serialize_lsp_request::<T>(id.clone(), params);
        self.send_message(&req).await?;
        let resp = rx.await?;
        Ok(resp)
    }

    async fn send_notification<T: lsp_types::notification::Notification>(
        &mut self,
        params: T::Params,
    ) -> Result<()> {
        let req = serialize_lsp_notification::<T>(params);
        self.send_message(&req).await?;
        Ok(())
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
impl Server for LspServer {
    type Request = LspRequest;
    type Response = LspResponse;

    async fn process_request(&mut self, request: Self::Request) -> Result<Self::Response> {
        match request {
            LspRequest::OpenFile { path, text } => {
                trace!("DidOpenTextDocument(path={})", path.display());
                self.send_notification::<DidOpenTextDocument>(DidOpenTextDocumentParams {
                    text_document: lsp_types::TextDocumentItem {
                        uri: parse_path_as_uri(&path),
                        language_id: self.language_id.clone(),
                        version: 0,
                        text,
                    },
                })
                .await?;
                Ok(LspResponse::NoContent)
            }
            LspRequest::UpdateFile {
                path,
                text,
                version,
            } => {
                trace!("DidChangeTextDocument(path={})", path.display());
                self.send_notification::<DidChangeTextDocument>(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: parse_path_as_uri(&path),
                        version: version as i32,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text,
                    }],
                })
                .await?;
                Ok(LspResponse::NoContent)
            }
            LspRequest::IncrementalUpdateFile {
                path,
                edits,
                version,
            } => {
                trace!(
                    "DidChangeTextDocument(path={}, edits={})",
                    path.display(),
                    edits.len()
                );
                let content_changes = edits
                    .into_iter()
                    .map(|edit| TextDocumentContentChangeEvent {
                        range: Some(edit.range),
                        range_length: None,
                        text: edit.new_text,
                    })
                    .collect();

                self.send_notification::<DidChangeTextDocument>(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: parse_path_as_uri(&path),
                        version: version as i32,
                    },
                    content_changes,
                })
                .await?;
                Ok(LspResponse::NoContent)
            }
            LspRequest::Completion { path, position } => {
                trace!(
                    "Completion(path={}, position={:?})",
                    path.display(),
                    position
                );
                self.call_method::<Completion>(CompletionParams {
                    text_document_position: TextDocumentPositionParams {
                        position,
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
            LspRequest::Format { path, options } => {
                trace!("Format(path={})", path.display());
                self.call_method::<Formatting>(DocumentFormattingParams {
                    text_document: TextDocumentIdentifier {
                        uri: parse_path_as_uri(&path),
                    },
                    options,
                    work_done_progress_params: WorkDoneProgressParams {
                        work_done_token: None,
                    },
                })
                .await
            }
            LspRequest::Hover { path, position } => {
                trace!("Hover(path={}, position={:?})", path.display(), position);
                self.call_method::<HoverRequest>(HoverParams {
                    text_document_position_params: TextDocumentPositionParams {
                        position,
                        text_document: TextDocumentIdentifier {
                            uri: parse_path_as_uri(&path),
                        },
                    },
                    work_done_progress_params: WorkDoneProgressParams {
                        work_done_token: None,
                    },
                })
                .await
            }
            LspRequest::GoToDefinition { path, position } => {
                trace!(
                    "GoToDefinition(path={}, position={:?})",
                    path.display(),
                    position
                );
                self.call_method::<GotoDefinition>(GotoDefinitionParams {
                    text_document_position_params: TextDocumentPositionParams {
                        position,
                        text_document: TextDocumentIdentifier {
                            uri: parse_path_as_uri(&path),
                        },
                    },
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

unsafe impl Send for LspServer {}
