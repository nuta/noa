use crossterm::style::{Attribute, Color};

use super::canvas::Style;

#[derive(Clone, Debug)]
pub struct Theme {
    pub bottom_bar_text: Style,
}

pub const DEFAULT_THEME: &'static Theme = &Theme {
    bottom_bar_text: Style {
        fg: Color::White,
        bg: Color::Grey,
        attr: Attribute::Reset,
    },
};
