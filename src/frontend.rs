use crate::screen::Screen;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Event {
    Unknown,
    Char(char),
    AnyChar,
    Ctrl(char),
    Alt(char),
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Esc,

    CommandMenu,
}

#[derive(Debug)]
pub struct ScreenSize {
    pub height: usize,
    pub width: usize,
}

pub trait FrontEnd {
    fn read_event(&mut self) -> Event;
    fn render(&mut self, screen: &Screen);
    fn get_screen_size(&self) -> ScreenSize;
}
