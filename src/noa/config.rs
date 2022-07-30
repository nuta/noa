use std::collections::HashMap;

use anyhow::{Context, Result};
use noa_common::warn_once;
use noa_compositor::{
    canvas::{Color, Style},
    terminal::{KeyCode, KeyModifiers},
};

use once_cell::sync::Lazy;

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Shift,
    Ctrl,
    Alt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyBindingScope {
    Buffer,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct KeyBinding {
    pub scope: KeyBindingScope,
    pub modifiers: Vec<Modifier>,
    // "enter", "tab", "F1",  "up", "down", "right", "left", or "a".."z".
    pub key: String,
    pub action: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ThemeDecoration {
    Underline,
    Bold,
    Inverted,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ThemeItem {
    #[serde(default)]
    pub fg: String,
    #[serde(default)]
    pub bg: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub inverted: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ConfigFile {
    key_bindings: Vec<KeyBinding>,
    theme: HashMap<String, ThemeItem>,
    colors: HashMap<String, String>,
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

static THEME: Lazy<HashMap<String, Style>> = Lazy::new(|| {
    let mut styles = HashMap::new();
    let mut color_mappings = HashMap::new();

    color_mappings.insert("".to_owned(), Color::Reset);
    color_mappings.insert("default".to_owned(), Color::Reset);
    color_mappings.insert("black".to_owned(), Color::Black);
    color_mappings.insert("darkgrey".to_owned(), Color::DarkGrey);
    color_mappings.insert("red".to_owned(), Color::Red);
    color_mappings.insert("darkred".to_owned(), Color::DarkRed);
    color_mappings.insert("green".to_owned(), Color::Green);
    color_mappings.insert("darkgreen".to_owned(), Color::DarkGreen);
    color_mappings.insert("yellow".to_owned(), Color::Yellow);
    color_mappings.insert("darkyellow".to_owned(), Color::DarkYellow);
    color_mappings.insert("blue".to_owned(), Color::Blue);
    color_mappings.insert("darkblue".to_owned(), Color::DarkBlue);
    color_mappings.insert("magenta".to_owned(), Color::Magenta);
    color_mappings.insert("darkmagenta".to_owned(), Color::DarkMagenta);
    color_mappings.insert("cyan".to_owned(), Color::Cyan);
    color_mappings.insert("darkcyan".to_owned(), Color::DarkCyan);
    color_mappings.insert("white".to_owned(), Color::White);
    color_mappings.insert("grey".to_owned(), Color::Grey);

    parse_theme(
        &mut styles,
        &mut color_mappings,
        &DEFAULT_CONFIG_FILE.theme,
        &DEFAULT_CONFIG_FILE.colors,
    )
    .expect("failed to parse default theme");
    parse_theme(
        &mut styles,
        &mut color_mappings,
        &USER_CONFIG_FILE.theme,
        &USER_CONFIG_FILE.colors,
    )
    .expect("failed to parse user theme");
    styles
});

pub fn get_keybinding_for(
    scope: KeyBindingScope,
    keycode: KeyCode,
    modifiers: KeyModifiers,
) -> Option<KeyBinding> {
    KEY_BINDINGS.get(&(scope, keycode, modifiers)).cloned()
}

fn parse_color(color: &str) -> Result<Color> {
    let color = match color {
        "default" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "blue" => Color::Blue,
        "yellow" => Color::Yellow,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "grey" => Color::Grey,
        "magenta" => Color::Magenta,
        "darkgrey" => Color::DarkGrey,
        "darkred" => Color::DarkRed,
        "darkgreen" => Color::DarkGreen,
        "darkyellow" => Color::DarkYellow,
        "darkblue" => Color::DarkBlue,
        "darkmagenta" => Color::DarkMagenta,
        rgb if rgb.starts_with('#') && rgb.len() == 7 => {
            let r = u8::from_str_radix(&rgb[1..3], 16)
                .with_context(|| format!("failed to parse rgb: {}", rgb))?;
            let g = u8::from_str_radix(&rgb[3..5], 16)
                .with_context(|| format!("failed to parse rgb: {}", rgb))?;
            let b = u8::from_str_radix(&rgb[5..7], 16)
                .with_context(|| format!("failed to parse rgb: {}", rgb))?;
            Color::Rgb { r, g, b }
        }
        _ => return Err(anyhow::anyhow!("invalid color: {}", color)),
    };

    Ok(color)
}

fn parse_theme(
    styles: &mut HashMap<String, Style>,
    color_mappings: &mut HashMap<String, Color>,
    theme: &HashMap<String, ThemeItem>,
    colors: &HashMap<String, String>,
) -> Result<()> {
    for (name, color) in colors {
        color_mappings.insert(name.to_string(), parse_color(color)?);
    }

    for (key, value) in theme {
        let fg = color_mappings
            .get(&value.fg)
            .copied()
            .with_context(|| format!("failed to find color \"{}\"", value.fg))?;
        let bg = color_mappings
            .get(&value.bg)
            .copied()
            .with_context(|| format!("failed to find color \"{}\"", value.bg))?;

        styles.insert(
            key.to_owned(),
            Style {
                fg,
                bg,
                bold: value.bold,
                underline: value.underline,
                inverted: value.inverted,
            },
        );
    }

    Ok(())
}

pub fn theme_for(key: &str) -> Style {
    match THEME.get(key) {
        Some(style) => *style,
        None => {
            warn_once!("not defined theme: \"{}\"", key);
            Default::default()
        }
    }
}

pub fn parse_config_files() {
    Lazy::force(&KEY_BINDINGS);
    Lazy::force(&THEME);
}
