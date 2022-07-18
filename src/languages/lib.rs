#[allow(unused_imports)]
#[macro_use]
extern crate log;

use once_cell::sync::Lazy;

pub use crate::languages::*;
use std::{collections::HashMap, ffi::OsString, path::Path};

pub mod languages;
pub mod tree_sitter;

pub fn guess_language(path: &Path) -> Option<&'static Language> {
    static FILE_NAMES: Lazy<HashMap<OsString, &'static Language>> = Lazy::new(|| {
        let mut file_names = HashMap::new();
        for language in LANGUAGES.iter() {
            for file_name in language.filenames.iter() {
                file_names.insert(file_name.into(), language);
            }
        }
        file_names
    });

    static EXTENSIONS: Lazy<HashMap<OsString, &'static Language>> = Lazy::new(|| {
        let mut extensions = HashMap::new();
        for language in LANGUAGES.iter() {
            for extension in language.extensions.iter() {
                extensions.insert(extension.into(), language);
            }
        }
        extensions
    });

    if let Some(file_name) = path.file_name() {
        if let Some(language) = FILE_NAMES.get(file_name) {
            return Some(language);
        }
    }

    if let Some(extension) = path.extension() {
        if let Some(language) = EXTENSIONS.get(extension) {
            return Some(language);
        }
    }

    None
}

pub fn get_language_by_name(name: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|&lang| lang.name == name)
}
