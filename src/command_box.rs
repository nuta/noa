use std::cmp::min;
use std::process::{Command, Stdio};
use std::path::PathBuf;
use std::io::{self, Read, Write};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use std::time::{Instant};
use crate::rope::{Range, Point};
use crate::buffer::{BufferId};

#[derive(Serialize, Deserialize, Clone)]
pub struct File {
    pub display_name: String,
    pub path: PathBuf,
    pub buffer_id: Option<BufferId>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum PreviewItem {
    #[serde(rename = "print")]
    Print {
        body: String
    },
    #[serde(rename = "print_with_file")]
    PrintWithFile {
        file: File,
        body: String,
        lineno: Option<usize>,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Change {
    pub location: Location,
    pub new_str: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ResponseBody {
    #[serde(rename = "preview")]
    Preview {
        items: Vec<PreviewItem>,
        selectable: bool,
    },
    #[serde(rename = "goto")]
    GoTo {
        file: File,
        position: Option<Point>,
    },
    #[serde(rename = "replace_with")]
    ReplaceWith {
        changes: Vec<Change>,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Response {
    pub message: Option<String>,
    pub body: ResponseBody,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Location {
    pub file: File,
    pub range: Range,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RequestBody {
    #[serde(rename = "select_match")]
    SelectMatch {
        locations: Vec<Location>,
    },
    #[serde(rename = "goto")]
    GoTo {
        file: File,
        position: Point,
    },
    #[serde(rename = "select_file")]
    SelectFile {
        files: Vec<File>
    },
    #[serde(rename = "replace_with")]
    ReplaceWith {
        locations: Vec<Location>,
        new_str: String,
    },
}

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub preview: bool,
    pub selected: usize,
    pub script: String,
    pub body: RequestBody,
}

pub struct CommandBox {
    last_response: Option<Response>,
    last_stderr: String,
    selected: usize,
    num_items: usize,
    #[allow(unused)]
    script_file: NamedTempFile,
    script_file_path: String,
}

impl CommandBox {
    pub fn new() -> CommandBox {
        let mut script_file = NamedTempFile::new().unwrap();
        writeln!(&mut script_file, "{}", include_str!("command_box.rb")).unwrap();
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

    pub fn last_response(&self) -> Option<&Response> {
        self.last_response.as_ref()
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn open(&mut self) {
        self.selected = 0;
        self.last_response = None;
    }

    pub fn execute(&mut self, request: Request) -> io::Result<()> {
        self.last_stderr.clear();

        trace!("running ruby script: {}", self.script_file_path);
        let started_at = Instant::now();

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
        stderr.read_to_string(&mut self.last_stderr).ok();
        child.wait().ok();
        error!("{}", json_string);
        trace!("rb: took {} ms", started_at.elapsed().as_millis());

        let resp: Response = serde_json::from_str(&json_string)?;
        self.num_items = match &resp.body {
            ResponseBody::Preview { items, .. } => items.len(),
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

