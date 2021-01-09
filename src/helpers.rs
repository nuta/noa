use std::fs::{create_dir_all, File, OpenOptions};
use std::path::PathBuf;

fn resolve_noa_path(filename: &str, sub_dir: Option<&str>) -> PathBuf {
    let mut dir = dirs::home_dir().unwrap().join(".noa");
    if let Some(sub_dir) = sub_dir {
        dir = dir.join(sub_dir);
    }

    trace!(
        "dir={}, file={}",
        dir.display(),
        dir.join(filename).display()
    );
    if !dir.exists() {
        create_dir_all(&dir).ok();
    }

    dir.join(filename)
}

pub fn open_log_file(filename: &str) -> std::io::Result<File> {
    OpenOptions::new()
        .read(false)
        .write(true)
        .append(true)
        .create(true)
        .open(resolve_noa_path(filename, Some("log")))
}
