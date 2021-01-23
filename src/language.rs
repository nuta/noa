use std::hash::{Hash, Hasher};
use std::path::Path;

pub struct Language {
    pub name: &'static str,
    pub comment_out: Option<&'static str>,
}

lazy_static! {
    pub static ref PLAIN: Language = {
        Language {
            name: "plain",
            comment_out: None,
        }
    };
}

lazy_static! {
    pub static ref CXX: Language = {
        Language {
            name: "cxx",
            comment_out: Some("// "),
        }
    };
}

lazy_static! {
    pub static ref RUST: Language = {
        Language {
            name: "rust",
            comment_out: Some("// "),
        }
    };
}

pub fn guess_language(path: &Path) -> &'static Language {
    match path.extension().map(|s| s.to_str().unwrap()) {
        Some("rs") => &RUST,
        Some("c") | Some("cpp") | Some("cxx") | Some("h") | Some("hpp") | Some("hxx") => &CXX,
        _ => &PLAIN,
    }
}
