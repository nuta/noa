use std::cmp::min;
use std::process::{Command, Stdio};
use std::path::PathBuf;
use std::io::{self, Read, Write, Stdout};
use serde::{Deserialize, Serialize};
use ignore::WalkBuilder;
use tempfile::NamedTempFile;
use crate::terminal::{Terminal, KeyCode, KeyModifiers, KeyEvent};
use crate::rope::Range;
use crate::buffer::{BufferId, Buffer};
use crate::editor::Editor;

#[derive(Serialize, Deserialize)]
pub struct File {
    pub display_name: String,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Item {
    #[serde(rename = "print")]
    Print(String),
    #[serde(rename = "print_with_path")]
    PrintWithPath {
        file: File,
        body: String,
    },
    #[serde(rename = "file")]
    File(File),
    #[serde(rename = "file_position")]
    FilePosition {
        file: File,
        line: usize,
        column: usize,
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseBody {
    #[serde(rename = "executed")]
    Executed,
    #[serde(rename = "preview")]
    Preview {
        items: Vec<Item>,
    },
    #[serde(rename = "select")]
    Select {
        items: Vec<Item>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct Response {
    pub message: String,
    pub num_filtered: usize,
    pub body: ResponseBody,
}

#[derive(Serialize, Deserialize)]
pub struct Location {
    pub path: File,
    pub ranges: Vec<Range>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RequestBody {
    #[serde(rename = "locations")]
    Locations {
        locations: Vec<Location>,
    },
    #[serde(rename = "files")]
    Files {
        files: Vec<File>
    },
}

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub global: bool,
    pub preview: bool,
    pub script: String,
    pub body: RequestBody,
}

pub struct CommandBox {
    last_response: Option<Response>,
    last_stderr: String,
    selected: usize,
    num_items: usize,
    script_file: NamedTempFile,
    script_file_path: String,
}

impl CommandBox {
    pub fn new() -> CommandBox {
        let mut script_file = NamedTempFile::new().unwrap();
        writeln!(&mut script_file, include_str!("command_box.rb"));
        let script_file_path = script_file.path().to_str().unwrap().to_owned();

        CommandBox {
            last_response: None,
            last_stderr: String::new(),
            selected: 0,
            num_items: 0,
            script_file,
            script_file_path,
        }
    }

    pub fn last_stderr(&self) -> &str {
        &self.last_stderr
    }

    pub fn last_response(&self) -> &Option<Response> {
        &self.last_response
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn execute(&mut self, request: Request) -> io::Result<()> {
        self.last_stderr.clear();

        trace!("rb: {}", self.script_file_path);
        let mut child = Command::new("ruby")
            .args(&[&self.script_file_path])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();

        let input = serde_json::to_string(&request).unwrap();
        stdin.write_all(input.as_bytes()).ok();
        drop(stdin);

        let mut json_string = String::with_capacity(2048);
        stdout.read_to_string(&mut json_string).ok();
        stderr.read_to_string(&mut self.last_stderr);
        trace!("Response = {}", serde_json::to_string(&Response{ message: "".to_owned(), num_filtered: 0, body: ResponseBody::Executed }).unwrap());

        let resp: Response = serde_json::from_str(&json_string)?;
        self.num_items = match &resp.body {
            ResponseBody::Select { items } => items.len(),
            _ => 0,
        };

        self.last_response = Some(resp);
        Ok(())
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        self.selected = min(self.selected + 1, self.num_items);
    }
}

