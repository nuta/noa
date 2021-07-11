use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
};

use anyhow::Context;

pub fn path_into_dotted_str(path: &Path) -> String {
    path.to_str()
        .unwrap()
        .trim_start_matches('/')
        .replace('/', ".")
}

pub fn noa_dir() -> PathBuf {
    let dir = dirs::home_dir()
        .expect("where's your home dir?")
        .join(".noa");

    create_dir_all(&dir).expect("failed to create dir");
    dir
}

pub fn noa_workdir(workdir: &Path) -> PathBuf {
    let workdir = workdir
        .canonicalize()
        .with_context(|| format!("failed to resolve the workspace dir: {}", workdir.display()))
        .unwrap();

    let dir = noa_dir()
        .join("workdirs")
        .join(Path::new(&path_into_dotted_str(&workdir)));
    create_dir_all(&dir).expect("failed to create dir");
    dir
}

pub fn sync_sock_path(workdir: &Path, daemon_type: &str, lsp_lang: Option<&str>) -> PathBuf {
    let dir = noa_workdir(workdir);
    let name = match lsp_lang {
        Some(lang) => {
            format!("{}-{}.sock", daemon_type, lang)
        }
        None => {
            format!("{}.sock", daemon_type)
        }
    };

    dir.join(&name)
}

pub fn log_file_path(name: &str) -> PathBuf {
    let log_dir = noa_dir().join("log");
    create_dir_all(&log_dir).expect("failed to create dir");
    log_dir.join(&format!("{}.log", name))
}

pub fn backup_dir() -> PathBuf {
    let backup_dir = noa_dir().join("backup");
    create_dir_all(&backup_dir).expect("failed to create dir");
    backup_dir
}

pub fn noa_bin_args() -> &'static [&'static str] {
    if cfg!(debug_assertions) {
        &["cargo", "run", "--bin", "noa", "--"]
    } else {
        &["noa"]
    }
}
