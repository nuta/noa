use crate::editor::Event;
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender, Receiver};
use std::process::{Command, Child, Stdio, ChildStdin, ChildStdout};

pub enum Request {
    Initialize {
        root_path: String,
    }
}

fn send_requests(rx: Receiver<Request>, stdin: ChildStdin) {
    while let Ok(req) = rx.recv() {
        match req {
            Request::Initialize { root_path } => {
            }
        }
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

            if line.is_empty() {
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
        let resp = match serde_json::from_str::<jsonrpc_core::Output>(&body) {
            Ok(jsonrpc_core::Output::Success(json)) => json,
            Ok(jsonrpc_core::Output::Failure(failure)) => {
                warn!("LSP: {:?}", failure);
                continue 'thread_loop;
            }
            Err(err) => {
                warn!("failed to parse the boddy from the LSP server: {}", err);
                continue 'thread_loop;
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
            .args(&["--pretty"])
            .stdout(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
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

    pub fn tx_mut(&mut self) -> &mut Sender<Request> {
        &mut self.tx
    }
}
