use std::collections::HashMap;

use noa_compositor::canvas::{Color, Style};
use noa_languages::language::SyntaxSpan;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::minimap::LineStatus;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ThemeKey {
    SyntaxSpan(SyntaxSpan),
    Flash,
    CurrentLine,
    ErrorNotification,
    WarnNotification,
    InfoNotification,
    LineStatus(LineStatus),
    FinderInput,
    FinderSelectedItem,
    FindMatch,
}

pub struct Theme {
    mapping: HashMap<ThemeKey, Style>,
}

static THEME: Lazy<HashMap<String, Style>> = Lazy::new(|| {
    toml::from_str(include_str!("theme.toml")).expect("failed to parse the default theme file")
});

pub fn theme_for(key: &str) -> Style {
    match THEME.get(key) {
        Some(style) => *style,
        None => {
            panic!("the \"{}\" is not defined in the theme", key);
        }
    }
}
