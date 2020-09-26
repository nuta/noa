use crate::editor::Event;
use crate::language::{Language, LspSettings};
use crate::helpers::open_log_file;
use crate::buffer::Buffer;
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender, Receiver};
use std::process::{Command, Child, Stdio, ChildStdin, ChildStdout};
use std::path::{Path, PathBuf};
use lsp_types::{
    request::{
        Initialize,
    },
    notification::{
        DidOpenTextDocument,
        DidChangeTextDocument,
    },
    InitializeParams,
    DidOpenTextDocumentParams,
    VersionedTextDocumentIdentifier,
    DidChangeTextDocumentParams,
    TextDocumentContentChangeEvent,
};

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
}

fn parse_path_as_uri(path: &Path) -> lsp_types::Url {
    let uri = &format!("file:///{}", path.canonicalize().unwrap().to_str().unwrap());
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

fn send_requests(lsp: &LspSettings, rx: Receiver<Request>, mut stdin: ChildStdin) {
    while let Ok(req) = rx.recv() {
        let body = match req {
            Request::Initialize { root_path } => {
                info!("Initialize(root_path={})", root_path.display());
                serialize_request::<Initialize>(
                    0,
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
                    }
                )
            }
            Request::OpenFile { path, text } => {
                info!("DidOpenTextDocument(path={})", path.display());
                serialize_notification::<DidOpenTextDocument>(
                    DidOpenTextDocumentParams {
                        text_document: lsp_types::TextDocumentItem {
                            uri: parse_path_as_uri(&path),
                            language_id: lsp.language_id.to_owned(),
                            version: 0,
                            text,
                        }
                    }
                )
            }
            Request::ChangeFile { path, text, version } => {
                info!("DidChangeTextDocument(path={})", path.display());
                serialize_notification::<DidChangeTextDocument>(
                    DidChangeTextDocumentParams {
                        text_document: VersionedTextDocumentIdentifier {
                            uri: parse_path_as_uri(&path),
                            version: Some(version as i64),
                        },
                        content_changes: vec![TextDocumentContentChangeEvent {
                            range: None,
                            range_length: None,
                            text,
                        }]
                    }
                )
            }
        };

        use std::io::Write;
        write!(stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).ok();
    }
}

fn receive_requests(event_queue: Sender<Event>, stdout: ChildStdout) {
    use std::io::{BufReader, Read, BufRead};

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

            let words: Vec<&str> = line.split(":").collect();
            if words.len() != 2 {
                warn!("malformed LSP header: '{}'", line);
                continue;
            }

            headers.insert(words[0].trim().to_owned(),
                words[1].trim().to_owned());
        }

        // Parse Content-Length.
        let len = match headers.get("Content-Length")
            .and_then(|value| value.parse::<usize>().ok()) {
            Some(len) => len,
            None => {
                warn!("missing valid LSP Content-Length header");
                continue 'mainloop;
            }
        };

        // Read the content.
        let mut buf = vec![0; len];
        let body = match reader.read_exact(&mut buf) {
            Ok(_) => { String::from_utf8(buf).unwrap() },
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
    }
}

struct Server {
    process: Child,
    tx: Sender<Request>,
}

pub struct Lsp {
    event_queue: Sender<Event>,
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
    pub fn new(root_path: &Path, event_queue: Sender<Event>) -> Lsp {
        Lsp {
            event_queue,
            root_path: root_path.to_path_buf(),
            servers: HashMap::new(),
        }
    }

    pub fn open_buffer(&mut self, buffer: &Buffer) {
        if let Some(path) = buffer.path() {
            self.send(buffer.lang(), Request::OpenFile {
                path: path.to_owned(),
                text: buffer.text(),
            });
        }
    }

    pub fn modify_buffer(&mut self, buffer: &Buffer) {
        if let Some(path) = buffer.path() {
            self.send(buffer.lang(), Request::ChangeFile {
                path: path.to_owned(),
                text: buffer.text(),
                version: buffer.version(),
            });
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

    fn start_server(&mut self,
        lang: &'static Language,
        lsp: &'static LspSettings
    ) -> std::io::Result<&Server> {
        let (tx, rx) = mpsc::channel();
        let mut process = Command::new(lsp.command[0])
            .args(&lsp.command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(open_log_file(&format!("lsp-{}.log", lang.name)).unwrap()))
            .spawn()?;

        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();

        std::thread::spawn(move || send_requests(lsp, rx, stdin));
        let event_queue = self.event_queue.clone();
        std::thread::spawn(move || {
            receive_requests(event_queue, stdout);
            // TODO: Restart the server when it crashed.
            warn!("the LSP server seems to be terminated");
        });

        tx.send(Request::Initialize {
            root_path: self.root_path.to_owned(),
        }).ok();

        self.servers.insert(lang, Server { tx, process });
        Ok(self.servers.get(lang).unwrap())
    }
}
