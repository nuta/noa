use std::collections::HashMap;

use anyhow::Context;
use noa_compositor::terminal::{KeyCode, KeyModifiers};

use once_cell::sync::Lazy;

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Shift,
    Ctrl,
    Alt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyBindingScope {
    Buffer,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct KeyBinding {
    pub scope: KeyBindingScope,
    pub modifiers: Vec<Modifier>,
    // "enter", "tab", "F1",  "up", "down", "right", "left", or "a".."z".
    pub key: String,
    pub action: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ConfigFile {
    key_bindings: Vec<KeyBinding>,
}

fn parse_keybindings(
    map: &mut HashMap<(KeyBindingScope, KeyCode, KeyModifiers), KeyBinding>,
    bindings: &[KeyBinding],
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
            s if s.len() == 1 => KeyCode::Char(s.chars().next().unwrap()),
            s => {
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

        map.insert((binding.scope, keycode, modifiers), binding.clone());
    }
}

static DEFAULT_CONFIG_FILE: Lazy<ConfigFile> = Lazy::new(|| {
    toml::from_str(include_str!("defaults.toml"))
        .context("failed to parse defaults.toml")
        .unwrap()
});

static USER_CONFIG_FILE: Lazy<ConfigFile> = Lazy::new(|| {
    let paths = &[
        dirs::home_dir().unwrap().join(".noa.toml"),
        dirs::home_dir().unwrap().join(".config/noa/config.toml"),
    ];

    for path in paths {
        if path.exists() {
            return toml::from_str(&std::fs::read_to_string(path).unwrap())
                .with_context(|| format!("failed to parse {}", path.display()))
                .unwrap();
        }
    }

    Default::default()
});

static KEY_BINDINGS: Lazy<HashMap<(KeyBindingScope, KeyCode, KeyModifiers), KeyBinding>> =
    Lazy::new(|| {
        let mut map = HashMap::new();
        parse_keybindings(&mut map, &DEFAULT_CONFIG_FILE.key_bindings);
        parse_keybindings(&mut map, &USER_CONFIG_FILE.key_bindings);
        map
    });

pub fn get_keybinding_for(
    scope: KeyBindingScope,
    keycode: KeyCode,
    modifiers: KeyModifiers,
) -> Option<KeyBinding> {
    KEY_BINDINGS.get(&(scope, keycode, modifiers)).cloned()
}
