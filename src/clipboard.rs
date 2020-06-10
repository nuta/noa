use std::fs;
use std::path::PathBuf;

pub fn clipboard_path() -> PathBuf {
    let noa_dir = dirs::home_dir().unwrap().join(".noa");
    if !noa_dir.exists() {
        fs::create_dir(&noa_dir).ok();
    }

    noa_dir.join("clipboard").to_path_buf()
}

pub fn copy_from_clipboard() -> String {
    // TODO: Use the OS's clipboard (pbcopy/pbpaste).
    std::fs::read_to_string(clipboard_path()).unwrap_or_else(|_| String::new())
}

pub fn copy_into_clipboard(s: String) {
    // TODO:
    std::fs::write(clipboard_path(), s).ok();
}
