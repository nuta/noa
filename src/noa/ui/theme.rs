use crossterm::style::Color;

use super::{canvas::Style, Decoration};

#[derive(Clone, Debug)]
pub struct Theme {
    pub bottom_bar_text: Style,
    pub line_status_cursor: Style,
    pub line_status_warning: Style,
    pub line_status_error: Style,
    pub line_status_added: Style,
    pub line_status_removed: Style,
    pub line_status_modified: Style,
}

pub const DEFAULT_THEME: &Theme = &Theme {
    bottom_bar_text: Style {
        fg: Color::White,
        bg: Color::DarkGrey,
        deco: Decoration::empty(),
    },
    line_status_cursor: Style {
        fg: Color::Reset,
        bg: Color::DarkGrey,
        deco: Decoration::empty(),
    },
    line_status_warning: Style {
        fg: Color::Reset,
        bg: Color::DarkYellow,
        deco: Decoration::empty(),
    },
    line_status_error: Style {
        fg: Color::Reset,
        bg: Color::DarkRed,
        deco: Decoration::empty(),
    },
    line_status_added: Style {
        fg: Color::Reset,
        bg: Color::Green,
        deco: Decoration::empty(),
    },
    line_status_removed: Style {
        fg: Color::Reset,
        bg: Color::Red,
        deco: Decoration::empty(),
    },
    line_status_modified: Style {
        fg: Color::Reset,
        bg: Color::Yellow,
        deco: Decoration::empty(),
    },
};
