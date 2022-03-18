use std::collections::HashMap;

use anyhow::{Context, Result};
use noa_terminal::canvas::{Color, Style};

use once_cell::sync::Lazy;

use serde::Deserialize;

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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ThemeColor(String);

#[derive(Deserialize)]
struct Theme {
    theme: HashMap<String, ThemeItem>,
    colors: HashMap<String, ThemeColor>,
}

static THEME: Lazy<HashMap<String, Style>> = Lazy::new(|| {
    parse_theme(include_str!("theme.toml")).expect("failed to parse the default theme file")
});

fn parse_color(text: &ThemeColor) -> Result<Color> {
    let color = match text.0.as_str() {
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
        _ => return Err(anyhow::anyhow!("invalid color: {}", text.0)),
    };

    Ok(color)
}

fn parse_theme(text: &str) -> Result<HashMap<String, Style>> {
    let theme: Theme = toml::from_str(text)?;

    let mut colors = HashMap::new();
    colors.insert("".to_string(), Color::Reset);
    for (name, color) in &theme.colors {
        colors.insert(name.to_string(), parse_color(color)?);
    }

    let mut styles = HashMap::new();
    for (key, value) in theme.theme {
        let fg = colors
            .get(&value.fg)
            .copied()
            .with_context(|| format!("failed to find color \"{}\"", value.fg))?;
        let bg = colors
            .get(&value.bg)
            .copied()
            .with_context(|| format!("failed to find color \"{}\"", value.bg))?;

        styles.insert(
            key,
            Style {
                fg,
                bg,
                bold: value.bold,
                underline: value.underline,
                inverted: value.inverted,
            },
        );
    }

    Ok(styles)
}

pub fn theme_for(key: &str) -> Style {
    match THEME.get(key) {
        Some(style) => *style,
        None => {
            // warn!("the \"{}\" is not defined in the theme", key);
            Default::default()
        }
    }
}

pub fn parse_default_theme() {
    Lazy::force(&THEME);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_theme_parsing() {
        assert_eq!(
            parse_theme(
                r##"
                [theme]
                "buffer.find_match" = { fg = "red" }

                [colors]
                red = "#ff0000"
                "##,
            )
            .unwrap()
            .get("buffer.find_match"),
            Some(&Style {
                fg: Color::Rgb { r: 255, g: 0, b: 0 },
                bg: Color::Reset,
                ..Default::default()
            })
        );
    }
}
