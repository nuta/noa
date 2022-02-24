use bincode::Encode;
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command};

#[derive(Deserialize, Serialize, Encode)]
struct TreeSitter {
    #[serde(default)]
    dir: String,
    url: String,
    sources: Vec<String>,
}

#[derive(Deserialize, Serialize, Encode)]
struct Language {
    name: String,
    #[serde(default)]
    filenames: Vec<String>,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    tree_sitter: Option<TreeSitter>,
}

#[derive(Deserialize, Serialize, Encode)]
struct Languages {
    languages: Vec<Language>,
}

const NVIM_TREESITTER_REPO: &str = "https://github.com/nvim-treesitter/nvim-treesitter";

fn git_clone_and_pull(repo_url: &str, repo_dir: &Path) {
    if !repo_dir.exists() {
        println!("Cloning {}", repo_url);
        let ok = Command::new("git")
            .arg("clone")
            .args(&["--depth", "1"])
            .arg(repo_url)
            .arg(&repo_dir)
            .spawn()
            .expect("failed to clone a tree-sitter grammar repo")
            .wait()
            .expect("failed to wait git-clone(1)")
            .success();

        if !ok {
            panic!("failed to clone {}", repo_url);
        }
    } else {
        println!("Pulling {}", repo_url);
        Command::new("git")
            .arg("pull")
            .spawn()
            .expect("failed to pull a tree-sitter grammar repo")
            .wait()
            .expect("failed to wait git-pull(1)");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=languages.yaml");

    let nvim_treesitter_dir = Path::new("tree_sitter/nvim_treesitter");
    git_clone_and_pull(NVIM_TREESITTER_REPO, &nvim_treesitter_dir);

    let yaml: Languages = serde_yaml::from_str(include_str!("languages.yaml"))
        .expect("failed to parse languages.yaml");

    let grammars_dir = Path::new("tree_sitter/grammars");
    for lang in &yaml.languages {
        println!("Downloading {}", lang.name);
        let repo_dir = grammars_dir.join(&lang.name);

        if let Some(TreeSitter { dir, url, sources }) = &lang.tree_sitter {
            git_clone_and_pull(url, &repo_dir);

            let mut src_files = Vec::new();
            for file in sources {
                let path = repo_dir.join(dir).join(file);
                src_files.push(path);
            }

            println!("Compiling {}", lang.name);
            cc::Build::new()
                .include(repo_dir.join(dir).join("src"))
                .files(src_files)
                .warnings(false)
                .compile(&format!("tree-sitter-{}", lang.name));
        }
    }

    println!("Generating languages.bincode");
    let mut file = std::fs::File::create("languages.bincode").unwrap();
    bincode::encode_into_std_write(yaml, &mut file, bincode::config::standard()).unwrap();
}
