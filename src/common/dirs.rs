use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
};

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
    let dir = noa_dir()
        .join("workdirs")
        .join(Path::new(&path_into_dotted_str(workdir)));
    create_dir_all(&dir).expect("failed to create dir");
    dir
}

pub fn lsp_sock_path(workdir: &Path, lang: &str) -> PathBuf {
    let dir = noa_workdir(workdir);
    dir.join(&format!("{}.sock", lang))
}

pub fn lsp_pid_path(workdir: &Path, lang: &str) -> PathBuf {
    let dir = noa_workdir(workdir);
    dir.join(&format!("{}.pid", lang))
}

pub fn log_file_path(name: &str) -> PathBuf {
    let log_dir = noa_dir().join("log");
    create_dir_all(&log_dir).expect("failed to create dir");
    log_dir.join(&format!("{}.log", name))
}
