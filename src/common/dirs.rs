use std::path::{Path, PathBuf};

pub fn path_into_dotted_str(path: &Path) -> String {
    path.to_str()
        .unwrap()
        .trim_start_matches('/')
        .replace('/', ".")
}

pub fn noa_dir() -> PathBuf {
    dirs::home_dir()
        .expect("where's your home dir?")
        .join(".noa")
}

pub fn lsp_sock_path(workdir: &Path, lang: &str) -> PathBuf {
    noa_dir()
        .join(Path::new(&path_into_dotted_str(workdir)))
        .join(&format!("{}.sock", lang))
}

pub fn lsp_pid_path(workdir: &Path, lang: &str) -> PathBuf {
    noa_dir()
        .join(Path::new(&path_into_dotted_str(workdir)))
        .join(&format!("{}.pid", lang))
}

pub fn log_file_path(name: &str) -> PathBuf {
    noa_dir().join("log").join(&format!("{}.log", name))
}
