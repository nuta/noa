use crate::layout::Layout;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Event {
    Unknown,
    Char(char),
    Ctrl(char),
    Alt(char),
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug)]
pub struct ScreenSize {
    pub height: usize,
    pub width: usize,
}

pub trait FrontEnd {
    fn read_event(&mut self) -> Event;
    fn render(&mut self, layout: &Layout);
    fn get_screen_size(&self) -> ScreenSize;
}
