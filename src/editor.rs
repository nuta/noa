use crate::buffer::Buffer;
use crate::rope::Cursor;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers, Terminal};
use std::cell::RefCell;
use std::sync::mpsc::{channel, Receiver};

pub enum Event {
    Key(KeyEvent),
    KeyBatch(String),
    Resize { rows: usize, cols: usize },
}

enum EditorMode {
    Normal,
}

pub struct Editor {
    exited: bool,
    mode: EditorMode,
    terminal: Terminal,
    event_queue: Receiver<Event>,
    current_buffer: RefCell<Buffer>,
}

impl Editor {
    pub fn new() -> Editor {
        let (tx, rx) = channel();
        let scratch = RefCell::new(Buffer::new());
        Editor {
            exited: false,
            mode: EditorMode::Normal,
            terminal: Terminal::new(tx),
            event_queue: rx,
            current_buffer: scratch,
        }
    }

    pub fn run(&mut self) {
        loop {
            if self.exited {
                break;
            }

            self.terminal.draw();

            if let Ok(ev) = self.event_queue.recv() {
                self.handle_event(ev);
            }

            while let Ok(ev) = self.event_queue.try_recv() {
                self.handle_event(ev);
            }
        }
    }

    fn handle_event(&mut self, ev: Event) {
        match ev {
            Event::Key(key) => self.handle_key_event(key),
            Event::KeyBatch(s) => self.handle_key_batch_event(s),
            Event::Resize { rows, cols } => self.terminal.resize(rows, cols),
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
        match self.mode {
            EditorMode::Normal => match (key.code, key.modifiers) {
                (KeyCode::Char('q'), CTRL) => {
                    self.exited = true;
                }
                (KeyCode::Char('l'), CTRL) => {
                    self.current_buffer
                        .borrow_mut()
                        .centering(self.terminal.rows());
                    return;
                }
                //
                //  Text Editing.
                //
                (KeyCode::Char(ch), NONE) | (KeyCode::Char(ch), SHIFT) => {
                    self.current_buffer.borrow_mut().insert_char(ch);
                }
                (KeyCode::Enter, NONE) => {
                    self.current_buffer.borrow_mut().enter_with_indent();
                }
                (KeyCode::Backspace, NONE) => {
                    self.current_buffer.borrow_mut().backspace();
                }
                (KeyCode::Delete, NONE) | (KeyCode::Char('d'), CTRL) => {
                    self.current_buffer.borrow_mut().delete();
                }
                (KeyCode::Tab, NONE) => {
                    self.current_buffer.borrow_mut().tab();
                }
                (KeyCode::BackTab, _) => {
                    self.current_buffer.borrow_mut().back_tab();
                }
                (KeyCode::Char('/'), ALT) => {
                    self.current_buffer.borrow_mut().toggle_comment_out();
                }
                (KeyCode::Char('w'), CTRL) => {
                    let mut buffer = self.current_buffer.borrow_mut();
                    if let Some(current_word_range) = buffer.prev_word_range() {
                        buffer.set_cursor(Cursor::Selection(current_word_range));
                        buffer.backspace();
                    }
                }
                (KeyCode::Char('w'), ALT) => {
                    let mut buffer = self.current_buffer.borrow_mut();
                    if let Some(current_word_range) = buffer.current_word_range() {
                        buffer.set_cursor(Cursor::Selection(current_word_range));
                        buffer.backspace();
                    }
                }
                (KeyCode::Char('k'), CTRL) => {
                    self.current_buffer.borrow_mut().truncate();
                }
                (KeyCode::Char('k'), ALT) => {
                    self.current_buffer.borrow_mut().truncate_reverse();
                }
                //
                //  Move cursors.
                //
                (KeyCode::PageUp, NONE) => {
                    self.current_buffer
                        .borrow_mut()
                        .scroll_up(self.terminal.rows());
                }
                (KeyCode::PageDown, NONE) => {
                    self.current_buffer
                        .borrow_mut()
                        .scroll_down(self.terminal.rows());
                }
                (KeyCode::Char('d'), ALT) => {
                    self.current_buffer.borrow_mut().scroll_up(5);
                }
                (KeyCode::Char('c'), ALT) => {
                    self.current_buffer.borrow_mut().scroll_down(5);
                }
                (KeyCode::Up, NONE) => {
                    self.current_buffer.borrow_mut().move_cursor(1, 0, 0, 0);
                }
                (KeyCode::Down, NONE) => {
                    self.current_buffer.borrow_mut().move_cursor(0, 1, 0, 0);
                }
                (KeyCode::Left, NONE) => {
                    self.current_buffer.borrow_mut().move_cursor(0, 0, 1, 0);
                }
                (KeyCode::Right, NONE) => {
                    self.current_buffer.borrow_mut().move_cursor(0, 0, 0, 1);
                }
                (KeyCode::Up, SHIFT) => {
                    self.current_buffer.borrow_mut().select(1, 0, 0, 0);
                }
                (KeyCode::Down, SHIFT) => {
                    self.current_buffer.borrow_mut().select(0, 1, 0, 0);
                }
                (KeyCode::Left, SHIFT) => {
                    self.current_buffer.borrow_mut().select(0, 0, 1, 0);
                }
                (KeyCode::Right, SHIFT) => {
                    self.current_buffer.borrow_mut().select(0, 0, 0, 1);
                }
                (KeyCode::Char('a'), CTRL) => {
                    self.current_buffer.borrow_mut().move_to_beginning_of_line();
                }
                (KeyCode::Char('e'), CTRL) => {
                    self.current_buffer.borrow_mut().move_to_end_of_line();
                }
                (KeyCode::Char('b'), ALT) => {
                    self.current_buffer.borrow_mut().move_to_prev_word();
                }
                (KeyCode::Char('f'), ALT) => {
                    self.current_buffer.borrow_mut().move_to_next_word();
                }
                _ => {}
            },
        }
    }

    fn handle_key_batch_event(&mut self, s: String) {
        match self.mode {
            EditorMode::Normal => {
                self.current_buffer.borrow_mut().insert(&s);
            }
        }
    }
}
