use std::path::PathBuf;
use std::fs::{create_dir, File, OpenOptions};

fn resolve_path(filename: &str, sub_dir: Option<&str>) -> PathBuf {
    let mut dir = dirs::home_dir().unwrap().join(".noa");
    if let Some(sub_dir) = sub_dir {
        dir = dir.join(sub_dir);
    }

    if !dir.exists() {
        create_dir(&dir).ok();
    }

    dir.join(filename).to_path_buf()
}

pub fn open_log_file(filename: &str) -> std::io::Result<File> {
    OpenOptions::new()
        .read(false)
        .write(true)
        .append(true)
        .create(true)
        .open(resolve_path(filename, Some("log")))
}
