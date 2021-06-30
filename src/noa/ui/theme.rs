use crossterm::style::Color;

use super::{canvas::Style, Decoration};

#[derive(Clone, Debug)]
pub struct Theme {
    pub bottom_bar_text: Style,
}

pub const DEFAULT_THEME: &Theme = &Theme {
    bottom_bar_text: Style {
        fg: Color::White,
        bg: Color::DarkGrey,
        deco: Decoration::empty(),
    },
};
