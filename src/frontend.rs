use std::sync::mpsc::Sender;
use crate::screen::Screen;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
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

    ScreenResized,

    /// Invoke a command from the command menu.
    CommandMenu(String),
}

#[derive(Debug)]
pub struct ScreenSize {
    pub height: usize,
    pub width: usize,
}

pub trait FrontEnd {
    fn init(&mut self, event_queue: Sender<Event>);
    fn render(&mut self, screen: &Screen);
    fn get_screen_size(&self) -> ScreenSize;
}
