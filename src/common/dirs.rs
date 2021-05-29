use std::path::{Path, PathBuf};

pub fn path_into_dotted_str(path: &Path) -> String {
    path.to_str()
        .unwrap()
        .trim_start_matches('/')
        .replace('/', ".")
}

pub fn lsp_sock_path(workdir: &Path, lang: &str) -> PathBuf {
    Path::new(&path_into_dotted_str(workdir)).join(&format!("{}.sock", lang))
}
