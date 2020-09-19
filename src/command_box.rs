use std::process::{Command, Stdio};
use std::path::PathBuf;
use std::io::{self, Read, Write};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use crate::rope::Range;
use crate::buffer::BufferId;

#[derive(Serialize, Deserialize)]
pub struct File {
    display_name: String,
    path: PathBuf,
}

#[derive(Serialize, Deserialize)]
pub enum Item {
    Print(String),
    PrintWithPath {
        file: File,
        body: String,
    },
    File(File),
    FilePosition {
        file: File,
        line: usize,
        column: usize,
    }
}

#[derive(Serialize, Deserialize)]
pub enum ResponseBody {
    Executed,
    Preview {
        items: Vec<Item>,
    },
    Select {
        items: Vec<Item>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct Response {
    message: String,
    num_filtered: usize,
    body: Vec<ResponseBody>,
}

#[derive(Serialize, Deserialize)]
pub enum RequestBody {
    Location {
        path: File,
        ranges: Vec<Range>,
    },
    File(File),
}

#[derive(Serialize, Deserialize)]
pub struct Request {
    global: bool,
    enter: bool,
    body: Vec<RequestBody>,
}

pub struct CommandBox {
    script_file: NamedTempFile,
    script_file_path: String,
}

impl CommandBox {
    pub fn new() -> CommandBox {
        let mut script_file = NamedTempFile::new().unwrap();
        writeln!(&mut script_file, include_str!("command_box.rb"));
        let script_file_path = script_file.path().to_str().unwrap().to_owned();

        CommandBox {
            script_file,
            script_file_path,
        }
    }

    pub fn run(&mut self, ruby_script: &str, request: Request) -> io::Result<Response> {
        let mut child = Command::new("ruby")
            .args(&[&self.script_file_path])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();

        write!(&mut stdin, "{}", serde_json::to_string(&request).unwrap()).ok();
        drop(stdin);

        let mut json_string = String::with_capacity(2048);
        stdout.read_to_string(&mut json_string).ok();
        let resp: Response = serde_json::from_str(&json_string)?;
        Ok(resp)
    }
}

