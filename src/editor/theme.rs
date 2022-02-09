use std::collections::HashMap;

use anyhow::Result;
use noa_compositor::canvas::{Color, Style};
use noa_languages::language::SyntaxSpan;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::minimap::LineStatus;

static THEME: Lazy<HashMap<String, Style>> = Lazy::new(|| {
    parse_theme(include_str!("theme.toml")).expect("failed to parse the default theme file")
});

fn parse_theme(text: &str) -> Result<HashMap<String, Style>> {
    let theme = toml::from_str(text)?;
    Ok(theme)
}

pub fn theme_for(key: &str) -> Style {
    match THEME.get(key) {
        Some(style) => *style,
        None => {
            panic!("the \"{}\" is not defined in the theme", key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme_parsing() {
        assert_eq!(
            parse_theme(
                r#"
        "buffer.find_match" = { fg = "red" }
        "#,
            )
            .unwrap()
            .get("buffer.find_match"),
            Some(&Style {
                fg: Color::Red,
                bg: Color::Reset,
                ..Default::default()
            })
        );
    }
}
