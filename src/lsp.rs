use crate::editor::Event;
use crate::helpers::open_log_file;
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender, Receiver};
use std::process::{Command, Child, Stdio, ChildStdin, ChildStdout};
use std::path::{Path, PathBuf};

pub enum Request {
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
    let uri = &format!("file://{}", path.to_str().unwrap());
    lsp_types::Url::parse(uri).unwrap()
}

static LANG_ID_TABLE: phf::Map<&'static str, &'static str> = phf::phf_map! {
    "c" => "c",
    "h" => "h",
    "cpp" => "cpp",
    "cxx" => "cpp",
    "rs" => "rust",
};

fn path_to_lang_id(path: &Path) -> Option<&'static str> {
    match path.extension() {
        Some(ext) => match ext.to_str() {
            Some(ext) => LANG_ID_TABLE.get(ext).map(|s| *s),
            None => None,
        }
        None => None,
    }
}

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

fn send_requests(rx: Receiver<Request>, mut stdin: ChildStdin) {
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
                let language_id = match path_to_lang_id(&path) {
                    Some(id) => id,
                    None => {
                        warn!("unknown extension, ignoring: {}", path.display());
                        continue;
                    }
                };

                serialize_notification::<DidOpenTextDocument>(
                    DidOpenTextDocumentParams {
                        text_document: lsp_types::TextDocumentItem {
                            uri: parse_path_as_uri(&path),
                            language_id: language_id.to_owned(),
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

fn receive_responses(event_queue: Sender<Event>, stdout: ChildStdout) {
    use std::io::{BufReader, Read, BufRead};

    let mut reader = BufReader::new(stdout);
    'thread_loop: loop {
        // Read headers.
        let mut headers = HashMap::new();
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(_) => {},
                Err(_) => break 'thread_loop,
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
                continue 'thread_loop;
            }
        };

        // Read the content.
        let mut buf = vec![0; len];
        let body = match reader.read_exact(&mut buf) {
            Ok(_) => { String::from_utf8(buf).unwrap() },
            Err(err) => {
                warn!("failed to read from the LSP server: {}", err);
                continue 'thread_loop;
            }
        };

        // Parse the JSON.
        trace!("body = '{}'", body);
        let resp = match serde_json::from_str::<jsonrpc_core::Output>(&body) {
            Ok(jsonrpc_core::Output::Success(json)) => json,
            Ok(jsonrpc_core::Output::Failure(failure)) => {
                warn!("LSP: {:?}", failure);
                continue 'thread_loop;
            }
            Err(_) => {
                // Perhaps it is a notification from the server.
                match serde_json::from_str::<jsonrpc_core::Request>(&body) {
                    Ok(jsonrpc_core::Request::Single(req)) => {
                        trace!("request from server = {:?}", req);
                        continue 'thread_loop;
                    }
                    Ok(jsonrpc_core::Request::Batch(reqs)) => {
                        trace!("request from server = {:?}", reqs);
                        continue 'thread_loop;
                    }
                    Err(err) => {
                        warn!("failed to parse the body from the LSP server: {}", err);
                        continue 'thread_loop;
                    }
                }
            }
        };

        trace!("LSP: {:#?}", resp);
    }
}

pub struct Lsp {
    tx: Sender<Request>,
    server: Child,
}

impl Drop for Lsp {
    fn drop(&mut self) {
        self.server.kill().ok();
    }
}

impl Lsp {
    pub fn new(event_queue: Sender<Event>) -> std::io::Result<Lsp> {
        let (tx, rx) = mpsc::channel();

        // Start the LSP server.
        let mut server = Command::new("clangd")
            .args(&["-j=8", "--log=verbose", "--pretty"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(open_log_file("clangd.log").unwrap()))
            .spawn()?;

        let stdin = server.stdin.take().unwrap();
        let stdout = server.stdout.take().unwrap();

        //
        //  Send messages to the LSP sever.
        //
        std::thread::spawn(move || send_requests(rx, stdin));

        //
        //  Handle messages from the LSP sever.
        //
        std::thread::spawn(move || {
            receive_responses(event_queue, stdout);
            warn!("the LSP server seems to be terminated");
        });

        Ok(Lsp {
            tx,
            server,
        })
    }

    pub fn send(&mut self, req: Request) {
        self.tx.send(req).ok();
    }
}

