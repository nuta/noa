use crate::buffer::{Buffer, BufferId};
use crate::editor::{Diagnostic, DiagnosticSeverity, Event, EventQueue};
use crate::fuzzy::FuzzySet;
use crate::helpers::open_log_file;
use crate::language::{Language, LspSettings};
use crate::rope::{Point, Range};
use jsonrpc_core::{types::Value, Call, Id};
use lsp_types::{
    notification::{DidChangeTextDocument, DidOpenTextDocument},
    request::{Completion, GotoDefinition, Initialize, SignatureHelpRequest},
    CompletionParams, DidChangeTextDocumentParams, DidOpenTextDocumentParams, GotoDefinitionParams,
    InitializeParams, PartialResultParams, Position, SignatureHelpParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentPositionParams,
    VersionedTextDocumentIdentifier, WorkDoneProgressParams,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

enum Request {
    Initialize {
        root_path: PathBuf,
    },
    OpenFile {
        path: PathBuf,
        text: String,
    },
    ChangeFile {
        path: PathBuf,
        text: String,
        version: usize,
    },
    Completion {
        buffer_id: BufferId,
        path: PathBuf,
        y: usize,
        x: usize,
    },
    GotoDefinition {
        path: PathBuf,
        pos: Point,
    },
    SignatureHelp {
        path: PathBuf,
        pos: Point,
    },
}

enum SentRequest {
    Completion { buffer_id: BufferId },
    GotoDefinition,
    SignatureHelp,
}

fn parse_path_as_uri(path: &Path) -> lsp_types::Url {
    let uri = &format!("file:///{}", path.canonicalize().unwrap().to_str().unwrap());
    lsp_types::Url::parse(uri).unwrap()
}

fn parse_uri_as_path(uri: &str) -> Option<PathBuf> {
    uri.strip_prefix("file://").map(PathBuf::from)
}

fn serialize_request<T: lsp_types::request::Request>(id: Id, params: T::Params) -> String {
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

static NEXT_REQUEST_ID: AtomicUsize = AtomicUsize::new(0);

fn alloc_id() -> Id {
    Id::Num(NEXT_REQUEST_ID.fetch_add(1, Ordering::SeqCst) as u64)
}

fn send_requests(
    lsp: &LspSettings,
    rx: Receiver<Request>,
    mut stdin: ChildStdin,
    sent_reqs: Arc<Mutex<HashMap<Id, SentRequest>>>,
) {
    while let Ok(req) = rx.recv() {
        let body = match req {
            Request::Initialize { root_path } => {
                trace!("Initialize(root_path={})", root_path.display());
                serialize_request::<Initialize>(
                    alloc_id(),
                    #[allow(deprecated)]
                    InitializeParams {
                        process_id: None,
                        root_path: None,
                        root_uri: Some(parse_path_as_uri(&root_path)),
                        initialization_options: None,
                        capabilities: lsp_types::ClientCapabilities {
                            workspace: None,
                            text_document: None,
                            window: None,
                            experimental: None,
                        },
                        trace: None,
                        workspace_folders: None,
                        client_info: None,
                    },
                )
            }
            Request::OpenFile { path, text } => {
                trace!("DidOpenTextDocument(path={})", path.display());
                serialize_notification::<DidOpenTextDocument>(DidOpenTextDocumentParams {
                    text_document: lsp_types::TextDocumentItem {
                        uri: parse_path_as_uri(&path),
                        language_id: lsp.language_id.to_owned(),
                        version: 0,
                        text,
                    },
                })
            }
            Request::ChangeFile {
                path,
                text,
                version,
            } => {
                trace!("DidChangeTextDocument(path={})", path.display());
                serialize_notification::<DidChangeTextDocument>(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: parse_path_as_uri(&path),
                        version: Some(version as i64),
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text,
                    }],
                })
            }
            Request::Completion {
                path,
                buffer_id,
                y,
                x,
            } => {
                trace!("Completion(path={})", path.display());
                let id = alloc_id();
                sent_reqs
                    .lock()
                    .unwrap()
                    .insert(id.clone(), SentRequest::Completion { buffer_id });

                serialize_request::<Completion>(
                    id,
                    CompletionParams {
                        text_document_position: TextDocumentPositionParams {
                            position: Position {
                                line: y as u64,
                                character: x as u64,
                            },
                            text_document: TextDocumentIdentifier {
                                uri: parse_path_as_uri(&path),
                            },
                        },
                        work_done_progress_params: WorkDoneProgressParams {
                            work_done_token: None,
                        },
                        partial_result_params: PartialResultParams {
                            partial_result_token: None,
                        },
                        context: None,
                    },
                )
            }
            Request::GotoDefinition { path, pos } => {
                trace!("GotoDefinition(buffer={}, pos={})", path.display(), pos);
                let id = alloc_id();
                sent_reqs
                    .lock()
                    .unwrap()
                    .insert(id.clone(), SentRequest::GotoDefinition);

                serialize_request::<GotoDefinition>(
                    id,
                    GotoDefinitionParams {
                        text_document_position_params: TextDocumentPositionParams {
                            position: Position {
                                line: pos.y as u64,
                                character: pos.x as u64,
                            },
                            text_document: TextDocumentIdentifier {
                                uri: parse_path_as_uri(&path),
                            },
                        },
                        work_done_progress_params: WorkDoneProgressParams {
                            work_done_token: None,
                        },
                        partial_result_params: PartialResultParams {
                            partial_result_token: None,
                        },
                    },
                )
            }
            Request::SignatureHelp { path, pos } => {
                trace!("SignatureHelp(buffer={}, pos={})", path.display(), pos);
                let id = alloc_id();
                sent_reqs
                    .lock()
                    .unwrap()
                    .insert(id.clone(), SentRequest::SignatureHelp);

                serialize_request::<SignatureHelpRequest>(
                    id,
                    SignatureHelpParams {
                        text_document_position_params: TextDocumentPositionParams {
                            position: Position {
                                line: pos.y as u64,
                                character: pos.x as u64,
                            },
                            text_document: TextDocumentIdentifier {
                                uri: parse_path_as_uri(&path),
                            },
                        },
                        work_done_progress_params: WorkDoneProgressParams {
                            work_done_token: None,
                        },
                        context: None,
                    },
                )
            }
        };

        use std::io::Write;
        write!(stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).ok();
    }
}

fn receive_requests(
    event_queue: EventQueue,
    stdout: ChildStdout,
    sent_reqs: Arc<Mutex<HashMap<Id, SentRequest>>>,
) {
    use std::io::{BufRead, BufReader, Read};

    let mut reader = BufReader::new(stdout);
    'mainloop: loop {
        // Read headers.
        let mut headers = HashMap::new();
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
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
                continue 'mainloop;
            }
        };

        // Read the content.
        let mut buf = vec![0; len];
        let body = match reader.read_exact(&mut buf) {
            Ok(_) => String::from_utf8(buf).unwrap(),
            Err(err) => {
                warn!("failed to read from the LSP server: {}", err);
                continue 'mainloop;
            }
        };

        // Parse the JSON.
        let resp = match serde_json::from_str::<jsonrpc_core::Output>(&body) {
            Ok(jsonrpc_core::Output::Success(json)) => json,
            Ok(jsonrpc_core::Output::Failure(failure)) => {
                warn!("LSP: {:?}", failure);
                continue 'mainloop;
            }
            Err(_) => {
                // Perhaps it is a notification from the server.
                match serde_json::from_str::<jsonrpc_core::Request>(&body) {
                    Ok(jsonrpc_core::Request::Single(req)) => {
                        trace!("request from server = {:?}", req);
                        handle_push_from_server(&event_queue, req);
                        continue 'mainloop;
                    }
                    Ok(jsonrpc_core::Request::Batch(reqs)) => {
                        trace!("request from server = {:?}", reqs);
                        continue 'mainloop;
                    }
                    Err(err) => {
                        warn!("failed to parse the body from the LSP server: {}", err);
                        continue 'mainloop;
                    }
                }
            }
        };

        trace!("LSP response: {:#?}", resp);
        if let Some(req) = sent_reqs.lock().unwrap().get(&resp.id) {
            match req {
                SentRequest::Completion { buffer_id } => {
                    let mut items = FuzzySet::new();
                    if let Value::Object(fields) = resp.result {
                        if let Some(Value::Array(results)) = fields.get("items") {
                            for item in results {
                                if let Some(Value::String(name)) = item.get("insertText") {
                                    items.append(name.to_string());
                                }
                            }
                        }
                    }

                    event_queue.enqueue(Event::Completion {
                        buffer_id: *buffer_id,
                        items,
                    });
                }
                SentRequest::GotoDefinition => {
                    // FIXME: Help me :/
                    if let Value::Array(results) = resp.result {
                        if let Some(Value::Object(definition)) = results.get(0) {
                            if let (Some(Value::Object(range)), Some(Value::String(uri))) =
                                (definition.get("range"), definition.get("uri"))
                            {
                                if let Some(Value::Object(position)) = range.get("start") {
                                    if let (Some(Value::Number(y)), Some(Value::Number(x))) =
                                        (position.get("line"), position.get("character"))
                                    {
                                        if let (Some(y), Some(x)) = (y.as_u64(), x.as_u64()) {
                                            if let Some(path) = parse_uri_as_path(uri) {
                                                event_queue.enqueue(Event::GoTo {
                                                    path,
                                                    pos: Point::new(y as usize, x as usize),
                                                })
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                SentRequest::SignatureHelp => {
                    if let Value::Object(fields) = resp.result {
                        if let Some(Value::Array(sigs)) = fields.get("signatures") {
                            if let Some(Value::Object(sig)) = sigs.get(0) {
                                if let Some(Value::String(label)) = sig.get("label") {
                                    event_queue.enqueue(Event::HoverMessage {
                                        message: label.to_owned(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn handle_push_from_server(event_queue: &EventQueue, req: Call) {
    match req {
        Call::Notification(noti) => match noti.method.as_str() {
            "textDocument/publishDiagnostics" => {
                handle_diagnotics(event_queue, noti);
            }
            _ => {}
        },
        _ => {}
    }
}

fn handle_diagnotics(event_queue: &EventQueue, noti: jsonrpc_core::Notification) {
    if let jsonrpc_core::Params::Map(map) = noti.params {
        if let Some(Value::Array(diagnostics)) = map.get("diagnostics") {
            let mut diags = Vec::with_capacity(diagnostics.len());
            for diag in diagnostics {
                if let Value::Object(diag) = diag {
                    if let Some(Value::Object(range)) = diag.get("range") {
                        if let Some(Value::Object(start)) = range.get("start") {
                            if let Some(Value::Object(end)) = range.get("end") {
                                let start_pos = match (start.get("line"), start.get("character")) {
                                    (Some(Value::Number(line)), Some(Value::Number(column))) => {
                                        match (line.as_u64(), column.as_u64()) {
                                            (Some(line), Some(column)) => {
                                                Point::new(line as usize, column as usize)
                                            }
                                            _ => continue,
                                        }
                                    }
                                    _ => continue,
                                };
                                let end_pos = match (end.get("line"), end.get("character")) {
                                    (Some(Value::Number(line)), Some(Value::Number(column))) => {
                                        match (line.as_u64(), column.as_u64()) {
                                            (Some(line), Some(column)) => {
                                                Point::new(line as usize, column as usize)
                                            }
                                            _ => continue,
                                        }
                                    }
                                    _ => continue,
                                };

                                let severity = match diag.get("severity") {
                                    Some(Value::Number(severity)) => match severity.as_u64() {
                                        Some(1) => DiagnosticSeverity::Error,
                                        Some(2) => DiagnosticSeverity::Warning,
                                        Some(3) => DiagnosticSeverity::Info,
                                        Some(4) => DiagnosticSeverity::Hint,
                                        _ => continue,
                                    },
                                    _ => continue,
                                };

                                let message = match diag.get("message") {
                                    Some(Value::String(message)) => message.to_owned(),
                                    _ => continue,
                                };

                                diags.push(Diagnostic {
                                    severity,
                                    range: Range::from_points(start_pos, end_pos),
                                    message,
                                });
                            }
                        }
                    }
                }
            }

            event_queue.enqueue(Event::Diagnostics(diags));
        }
    }
}

struct Server {
    process: Child,
    tx: Sender<Request>,
}

pub struct Lsp {
    event_queue: EventQueue,
    servers: HashMap<&'static Language, Server>,
    root_path: PathBuf,
}

impl Drop for Lsp {
    fn drop(&mut self) {
        for server in self.servers.values_mut() {
            server.process.kill().ok();
        }
    }
}

impl Lsp {
    pub fn new(root_path: &Path, event_queue: EventQueue) -> Lsp {
        Lsp {
            event_queue,
            root_path: root_path.to_path_buf(),
            servers: HashMap::new(),
        }
    }

    pub fn open_buffer(&mut self, buffer: &Buffer) {
        if let Some(path) = buffer.path() {
            self.send(
                buffer.lang(),
                Request::OpenFile {
                    path: path.to_owned(),
                    text: buffer.text(),
                },
            );
        }
    }

    pub fn modify_buffer(&mut self, buffer: &Buffer) {
        if let Some(path) = buffer.path() {
            self.send(
                buffer.lang(),
                Request::ChangeFile {
                    path: path.to_owned(),
                    text: buffer.text(),
                    version: buffer.version(),
                },
            );
        }
    }

    pub fn request_signature_help(&mut self, buffer: &Buffer) {
        if let Some(path) = buffer.path() {
            self.send(
                buffer.lang(),
                Request::SignatureHelp {
                    path: path.to_owned(),
                    pos: *buffer.main_cursor_pos(),
                },
            );
        }
    }

    pub fn request_goto_definition(&mut self, buffer: &Buffer) {
        if let Some(path) = buffer.path() {
            self.send(
                buffer.lang(),
                Request::GotoDefinition {
                    path: path.to_owned(),
                    pos: *buffer.main_cursor_pos(),
                },
            );
        }
    }

    pub fn request_completions(&mut self, buffer: &Buffer) {
        if let Some(path) = buffer.path() {
            let main_pos = buffer.main_cursor_pos();
            self.send(
                buffer.lang(),
                Request::Completion {
                    buffer_id: buffer.id(),
                    path: path.to_owned(),
                    y: main_pos.y,
                    x: main_pos.x,
                },
            );
        }
    }

    fn send(&mut self, lang: &'static Language, req: Request) {
        if let Some(lsp) = &lang.lsp {
            let server = match self.servers.get(lang) {
                Some(server) => server,
                None => {
                    // The server for the language has not yet been spawned.
                    match self.start_server(lang, lsp) {
                        Ok(server) => server,
                        Err(err) => {
                            warn!("failed to start a LSP server: {:?}", err);
                            return;
                        }
                    }
                }
            };

            server.tx.send(req).ok();
        }
    }

    fn start_server(
        &mut self,
        lang: &'static Language,
        lsp: &'static LspSettings,
    ) -> std::io::Result<&Server> {
        let (tx, rx) = mpsc::channel();
        let mut process = Command::new(lsp.command[0])
            .args(&lsp.command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(
                open_log_file(&format!("lsp-{}.log", lang.name)).unwrap(),
            ))
            .spawn()?;

        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();

        let sent_reqs = Arc::new(Mutex::new(HashMap::new()));
        let sent_reqs_lock1 = sent_reqs.clone();
        let sent_reqs_lock2 = sent_reqs;
        std::thread::spawn(move || send_requests(lsp, rx, stdin, sent_reqs_lock1));
        let event_queue = self.event_queue.clone();
        std::thread::spawn(move || {
            receive_requests(event_queue, stdout, sent_reqs_lock2);
            // TODO: Restart the server when it crashed.
            warn!("the LSP server seems to be terminated");
        });

        tx.send(Request::Initialize {
            root_path: self.root_path.to_owned(),
        })
        .ok();

        self.servers.insert(lang, Server { tx, process });
        Ok(self.servers.get(lang).unwrap())
    }
}
