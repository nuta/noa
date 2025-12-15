use std::path::PathBuf;
use std::sync::LazyLock;

pub static HOME_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| std::env::home_dir().expect("failed to get home directory"));

pub static NOA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let noa_dir = HOME_DIR.join(".noa");
    if !noa_dir.exists() {
        std::fs::create_dir_all(&noa_dir).expect("failed to create noa directory");
    }

    noa_dir
});
