use std::collections::HashMap;

use anyhow::{Context, Result};
use noa_compositor::{
    canvas::{Color, Decoration, Style},
    terminal::{KeyCode, KeyModifiers},
};

use once_cell::sync::Lazy;

use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
enum Modifier {
    Shift,
    Ctrl,
    Alt,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct KeyBinding {
    pub modifiers: Vec<Modifier>,
    // "enter", "tab", "F1",  "up", "down", "right", "left", or "a".."z".
    pub key: String,
    pub action: String,
}

#[derive(Clone, Debug, Deserialize)]
struct KeyBindings {
    buffer: Vec<KeyBinding>,
}

fn parse_keybindings(
    scope: &'static str,
    bindings: Vec<KeyBinding>,
    map: &mut HashMap<(&'static str, KeyCode, KeyModifiers), KeyBinding>,
) {
    for binding in bindings {
        let keycode = match binding.key.as_str() {
            "enter" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "backspace" => KeyCode::Backspace,
            "delete" => KeyCode::Delete,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "esc" => KeyCode::Esc,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "F1" => KeyCode::F(1),
            "F2" => KeyCode::F(2),
            "F3" => KeyCode::F(3),
            "F4" => KeyCode::F(4),
            "F5" => KeyCode::F(5),
            "F6" => KeyCode::F(6),
            "F7" => KeyCode::F(7),
            "F8" => KeyCode::F(8),
            "F9" => KeyCode::F(9),
            "F10" => KeyCode::F(10),
            "F11" => KeyCode::F(11),
            "F12" => KeyCode::F(12),
            s @ _ if s.len() == 1 => KeyCode::Char(s.chars().next().unwrap()),
            s @ _ => {
                panic!("invalid key binding: key='{}'", s);
            }
        };

        let mut modifiers = KeyModifiers::empty();
        for modifier in &binding.modifiers {
            match modifier {
                Modifier::Shift => modifiers |= KeyModifiers::SHIFT,
                Modifier::Ctrl => modifiers |= KeyModifiers::CONTROL,
                Modifier::Alt => modifiers |= KeyModifiers::ALT,
            }
        }

        map.insert((scope, keycode, modifiers), binding);
    }
}

static KEY_BINDINGS: Lazy<HashMap<(&'static str, KeyCode, KeyModifiers), KeyBinding>> =
    Lazy::new(|| {
        let KeyBindings { buffer } = toml::from_str(include_str!("keybindings.toml"))
            .expect("failed to parse builtin keybindings.toml");

        let mut map = HashMap::new();
        parse_keybindings("buffer", buffer, &mut map);
        map
    });

pub fn get_keybinding_for(
    scope: &'static str,
    keycode: KeyCode,
    modifiers: KeyModifiers,
) -> Option<KeyBinding> {
    KEY_BINDINGS.get(&(scope, keycode, modifiers)).cloned()
}
