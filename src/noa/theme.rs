use noa_cui::{Color, Decoration, Style};

#[derive(Clone, Debug)]
pub struct Theme {
    pub bottom_bar_text: Style,
    pub line_status_visible: Color,
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
    line_status_visible: Color::DarkGrey,
    line_status_cursor: Style {
        fg: Color::Magenta,
        bg: Color::Reset,
        deco: Decoration::empty(),
    },
    line_status_warning: Style {
        fg: Color::DarkYellow,
        bg: Color::Reset,
        deco: Decoration::empty(),
    },
    line_status_error: Style {
        fg: Color::DarkRed,
        bg: Color::Reset,
        deco: Decoration::empty(),
    },
    line_status_added: Style {
        fg: Color::Green,
        bg: Color::Reset,
        deco: Decoration::empty(),
    },
    line_status_removed: Style {
        fg: Color::Red,
        bg: Color::Reset,
        deco: Decoration::empty(),
    },
    line_status_modified: Style {
        fg: Color::Yellow,
        bg: Color::Reset,
        deco: Decoration::empty(),
    },
};
