use std::path::PathBuf;
use std::sync::LazyLock;

pub static NOA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let home_dir = std::env::home_dir().expect("failed to get home directory");
    let noa_dir = home_dir.join(".noa");

    if !noa_dir.exists() {
        std::fs::create_dir_all(&noa_dir).expect("failed to create noa directory");
    }

    noa_dir
});
