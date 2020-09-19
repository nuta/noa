use std::path::PathBuf;
use serde::{Deserialize, Serialize};
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
