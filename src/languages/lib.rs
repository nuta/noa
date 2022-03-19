#[allow(unused_imports)]
#[macro_use]
extern crate log;

pub use crate::languages::*;
use std::{ffi::OsStr, path::Path};

pub mod languages;
pub mod tree_sitter;

pub fn guess_language(path: &Path) -> Option<&'static Language> {
    for lang in LANGUAGES {
        for ext in lang.filenames {
            if path.file_name() == Some(OsStr::new(ext)) {
                return Some(lang);
            }
        }

        for ext in lang.extensions {
            if path.extension() == Some(OsStr::new(ext)) {
                return Some(lang);
            }
        }
    }

    None
}

pub fn get_language_by_name(name: &str) -> Option<&'static Language> {
    for lang in LANGUAGES {
        if lang.name == name {
            return Some(lang);
        }
    }

    None
}
