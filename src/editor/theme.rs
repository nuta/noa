use std::collections::HashMap;

use noa_compositor::canvas::{Color, Decoration, Style};
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
}

pub struct Theme {
    mapping: HashMap<ThemeKey, Style>,
}

impl Theme {
    pub fn default() -> Theme {
        let mut mapping = HashMap::new();

        mapping.insert(
            ThemeKey::LineStatus(LineStatus::ADDED),
            Style {
                bg: Color::Rgb { r: 0, g: 255, b: 0 },
                ..Default::default()
            },
        );

        mapping.insert(
            ThemeKey::LineStatus(LineStatus::REMOVED),
            Style {
                bg: Color::Rgb { r: 255, g: 0, b: 0 },
                ..Default::default()
            },
        );

        mapping.insert(
            ThemeKey::LineStatus(LineStatus::MODIFIED),
            Style {
                bg: Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 128,
                },
                ..Default::default()
            },
        );

        mapping.insert(
            ThemeKey::Flash,
            Style {
                bg: Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 50,
                },
                ..Default::default()
            },
        );

        mapping.insert(
            ThemeKey::CurrentLine,
            Style {
                bg: Color::Rgb {
                    r: 55,
                    g: 55,
                    b: 55,
                },
                ..Default::default()
            },
        );

        mapping.insert(
            ThemeKey::SyntaxSpan(SyntaxSpan::Comment),
            Style {
                fg: Color::Green,
                ..Default::default()
            },
        );
        mapping.insert(
            ThemeKey::SyntaxSpan(SyntaxSpan::Ident),
            Style {
                fg: Color::Cyan,
                ..Default::default()
            },
        );
        mapping.insert(
            ThemeKey::SyntaxSpan(SyntaxSpan::StringLiteral),
            Style {
                fg: Color::Red,
                ..Default::default()
            },
        );
        mapping.insert(
            ThemeKey::SyntaxSpan(SyntaxSpan::PrimitiveType),
            Style {
                fg: Color::Blue,
                ..Default::default()
            },
        );
        mapping.insert(
            ThemeKey::SyntaxSpan(SyntaxSpan::EscapeSequence),
            Style {
                fg: Color::Yellow,
                ..Default::default()
            },
        );
        mapping.insert(
            ThemeKey::SyntaxSpan(SyntaxSpan::CMacro),
            Style {
                fg: Color::Magenta,
                ..Default::default()
            },
        );
        mapping.insert(
            ThemeKey::SyntaxSpan(SyntaxSpan::CIncludeArg),
            Style {
                fg: Color::Magenta,
                ..Default::default()
            },
        );

        Theme { mapping }
    }

    pub fn get(&self, key: ThemeKey) -> Style {
        self.mapping.get(&key).copied().unwrap_or_default()
    }
}

static THEME: Lazy<Mutex<Theme>> = Lazy::new(|| Mutex::new(Theme::default()));

pub fn theme_for(key: ThemeKey) -> Style {
    THEME.lock().get(key)
}
