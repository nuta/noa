use std::{path::Path, process::Command};

#[path = "languages.rs"]
mod languages;
use languages::{TreeSitter, LANGUAGES};

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
    }
}

fn extract_inherits_in_scm(scm_path: &Path) -> Vec<String> {
    let mut inherits = Vec::new();
    for line in std::fs::read_to_string(scm_path).unwrap().lines() {
        if line.starts_with("; inherits:") {
            let mut parts = line.split("inherits:");
            parts.next();
            for lang in parts.next().unwrap().split(',') {
                inherits.push(lang.trim().to_string());
            }
        }
    }

    inherits
}

fn get_query_path(lang_name: &str, scm_name: &str) -> String {
    format!(
        "tree_sitter/nvim_treesitter/queries/{}/{}.scm",
        lang_name, scm_name
    )
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=languages.rs");

    let nvim_treesitter_dir = Path::new("tree_sitter/nvim_treesitter");
    git_clone_and_pull(NVIM_TREESITTER_REPO, nvim_treesitter_dir);

    let grammars_dir = Path::new("tree_sitter/grammars");
    for lang in LANGUAGES {
        println!("Downloading {}", lang.name);
        let repo_dir = grammars_dir.join(&lang.name);

        if let Some(TreeSitter { dir, url, sources }) = &lang.tree_sitter {
            git_clone_and_pull(url, &repo_dir);

            let mut src_files = Vec::new();
            for file in *sources {
                let path = repo_dir.join(dir.unwrap_or(".")).join(file);
                src_files.push(path);
            }

            println!("Compiling {}", lang.name);
            let mut c_files = Vec::new();
            let mut cpp_files = Vec::new();

            for file in src_files {
                match file.extension().unwrap().to_str().unwrap() {
                    "c" => c_files.push(file),
                    "cpp" | "cxx" | "cc" => cpp_files.push(file),
                    _ => panic!("unsupported source file: {}", file.display()),
                }
            }

            let include_dir = repo_dir.join(dir.unwrap_or(".")).join("src");
            if !c_files.is_empty() {
                cc::Build::new()
                    .include(&include_dir)
                    .opt_level(3)
                    .cargo_metadata(true)
                    .warnings(false)
                    .files(c_files)
                    .compile(&format!("tree-sitter-{}-c", lang.name));
            }

            if !cpp_files.is_empty() {
                cc::Build::new()
                    .include(&include_dir)
                    .opt_level(3)
                    .cpp(true)
                    .cargo_metadata(true)
                    .warnings(false)
                    .files(cpp_files)
                    .compile(&format!("tree-sitter-{}-cpp", lang.name));
            }
        }
    }

    println!("Generating tree_sitter/mod.rs");
    let mut mod_rs = String::new();
    mod_rs.push_str("#![allow(clippy::all)]\n");
    mod_rs.push_str("pub use tree_sitter::*;\n");
    mod_rs.push_str("extern \"C\" {\n");
    for lang in LANGUAGES {
        if lang.tree_sitter.is_some() {
            mod_rs.push_str(&format!(
                "    fn tree_sitter_{}() -> Language;\n",
                lang.name
            ));
        }
    }
    mod_rs.push_str("}\n\n");
    mod_rs.push_str("pub fn get_tree_sitter_parser(name: &str) -> Option<Language> {\n");
    mod_rs.push_str("   match name {\n");
    for lang in LANGUAGES {
        if lang.tree_sitter.is_some() {
            mod_rs.push_str(&format!(
                "        \"{}\" => Some(unsafe {{ tree_sitter_{}() }}),\n",
                lang.name, lang.name
            ));
        }
    }
    mod_rs.push_str("    _ => None\n");
    mod_rs.push_str("    }\n");
    mod_rs.push_str("}\n\n");

    for scm_name in &["highlights", "indents"] {
        mod_rs.push_str(&format!(
            "pub fn get_{}_query(name: &str) -> Option<&str> {{\n",
            scm_name
        ));
        mod_rs.push_str("   match name {\n");
        for lang in LANGUAGES {
            let scm = get_query_path(lang.name, scm_name);
            let scm_path = Path::new(&scm);
            if scm_path.exists() {
                mod_rs.push_str(&format!("        \"{}\" => Some(concat!(\n", lang.name));
                for inherit in extract_inherits_in_scm(scm_path) {
                    let scm = get_query_path(&inherit, scm_name);
                    if !Path::new(&scm).exists() {
                        panic!(
                            "{} is referenced from {}, but does not exist",
                            scm, lang.name
                        );
                    }
                    mod_rs.push_str(&format!("            include_str!(\"../{}\"),\n", scm));
                }

                mod_rs.push_str(&format!("            include_str!(\"../{}\"),\n", scm));
                mod_rs.push_str("        )),\n");
            }
        }
        mod_rs.push_str("    _ => None\n");
        mod_rs.push_str("    }\n");
        mod_rs.push_str("}\n");
    }

    std::fs::write("tree_sitter/mod.rs", mod_rs).unwrap();
}
