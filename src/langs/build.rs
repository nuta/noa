use std::{ffi::OsStr, fs, path::Path, process::Command};

struct Lang {
    name: &'static str,
    url: &'static str,
}

static LANGS: &[Lang] = &[
    Lang {
        name: "c",
        url: "https://github.com/tree-sitter/tree-sitter-c",
    },
    Lang {
        name: "cpp",
        url: "https://github.com/tree-sitter/tree-sitter-cpp",
    },
    Lang {
        name: "rust",
        url: "https://github.com/tree-sitter/tree-sitter-rust",
    },
];

fn main() {
    let grammars_dir = Path::new("tree_sitter/grammars");
    for lang in LANGS {
        let repo_dir = grammars_dir.join(lang.name);
        let src_dir = repo_dir.join("src");

        if !repo_dir.exists() {
            let ok = Command::new("git")
                .arg("clone")
                .args(&["--depth", "1"])
                .arg(lang.url)
                .arg(&repo_dir)
                .spawn()
                .expect("failed to clone a tree-sitter grammar repo")
                .wait()
                .expect("failed to wait git(1)")
                .success();

            if !ok {
                panic!("failed to clone {}", lang.url);
            }
        }

        let mut src_files = Vec::new();
        for file in fs::read_dir(&src_dir).unwrap() {
            let path = file.unwrap().path();
            if path.extension() == Some(OsStr::new("c")) {
                src_files.push(path);
            }
        }

        cc::Build::new()
            .include(&src_dir)
            .files(src_files)
            .warnings(false)
            .compile(&format!("tree-sitter-{}", lang.name));
    }
}
