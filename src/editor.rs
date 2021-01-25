use crate::buffer::Buffer;
use crate::finder::{Finder, FinderItem};
use crate::line_edit::LineEdit;
use crate::rope::{Cursor, Point, Range};
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, RawMouseEvent, Terminal};
use std::cell::RefCell;
use std::cmp::min;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver};
use std::time::Instant;

pub enum Event {
    Key(KeyEvent),
    KeyBatch(String),
    Mouse(RawMouseEvent),
    Resize { rows: usize, cols: usize },
    Redraw,
}

#[derive(Clone, Copy)]
pub enum NotificationLevel {
    Info,
    Error,
}

pub struct Notification {
    pub level: NotificationLevel,
    pub message: String,
    pub created_at: Instant,
}

enum EditorMode {
    Normal,
    Finder,
}

pub enum ExitStatus {
    Gracefully,
    ForceExit { unsaved_files: Vec<PathBuf> },
}

pub struct Editor {
    exited: bool,
    mode: EditorMode,
    terminal: Terminal,
    finder: Finder,
    prompt_input: LineEdit,
    event_queue: Receiver<Event>,
    buffers: HashMap<PathBuf, Rc<RefCell<Buffer>>>,
    recent_buffers: Vec<Rc<RefCell<Buffer>>>,
    current_buffer: Rc<RefCell<Buffer>>,
    notification: RefCell<Option<Notification>>,
    backup_dir: PathBuf,
    cursor_hover: Option<Point>,
}

impl Editor {
    pub fn new() -> Editor {
        let (tx, rx) = channel();

        // Prepare the noa directory.
        let noa_dir = dirs::home_dir().unwrap().join(".noa");
        std::fs::create_dir_all(&noa_dir).expect("failed to create ~/.noa");

        // Open the scratch buffer.
        let mut scratch_buffer = Buffer::new();
        scratch_buffer.set_name("scratch");

        Editor {
            exited: false,
            mode: EditorMode::Normal,
            terminal: Terminal::new(tx.clone()),
            finder: Finder::new(tx),
            prompt_input: LineEdit::new(),
            event_queue: rx,
            buffers: HashMap::new(),
            recent_buffers: Vec::new(),
            current_buffer: Rc::new(RefCell::new(scratch_buffer)),
            notification: RefCell::new(None),
            backup_dir: noa_dir.join("backup"),
            cursor_hover: None,
        }
    }

    pub fn open_file(&mut self, path: &Path) {
        // Switch the buffer if the file is already opened.
        let abs_path = match std::fs::canonicalize(path) {
            Ok(abs_path) => {
                if let Some(buffer) = self.buffers.get(&abs_path) {
                    self.current_buffer = buffer.clone();
                    self.recent_buffers.remove(
                        self.recent_buffers
                            .iter()
                            .enumerate()
                            .find(|(i, b)| Rc::ptr_eq(b, buffer))
                            .map(|(i, _)| i)
                            .unwrap(),
                    );
                    self.recent_buffers.push(buffer.clone());
                    return;
                } else {
                    abs_path
                }
            }
            Err(err) => {
                self.error(format!("couldn't resolve the path: {:?}", err));
                return;
            }
        };

        let writable = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .is_ok();
        if !writable {
            self.error(format!("not writable: {}", path.display()));
        }

        match Buffer::open_file(path) {
            Ok(mut buffer) => {
                // Update the buffer name.
                let current_dir = std::env::current_dir().unwrap();
                let name = match abs_path.strip_prefix(&current_dir) {
                    Ok(stripped_path) => stripped_path.to_str().unwrap(),
                    Err(_) => abs_path.to_str().unwrap(),
                };
                buffer.set_name(name);

                let buffer_rc = Rc::new(RefCell::new(buffer));
                self.buffers.insert(abs_path, buffer_rc.clone());
                self.recent_buffers.push(buffer_rc.clone());
                self.current_buffer = buffer_rc;
            }
            Err(err) => {
                self.error(format!("couldn't open: {:?}", err));
            }
        }
    }

    pub fn run(&mut self) -> ExitStatus {
        loop {
            if self.exited {
                let unsaved_files: Vec<PathBuf> = self
                    .buffers
                    .values()
                    .filter(|b| {
                        let b = b.borrow();
                        !b.is_virtual_file() && b.is_dirty()
                    })
                    .map(|b| b.borrow().path().unwrap().to_owned())
                    .collect();

                if unsaved_files.is_empty() {
                    return ExitStatus::Gracefully;
                } else {
                    return ExitStatus::ForceExit { unsaved_files };
                }
            }

            match self.mode {
                EditorMode::Normal => {
                    self.terminal.draw_buffer(
                        &mut *self.current_buffer.borrow_mut(),
                        self.notification.borrow().as_ref(),
                        self.cursor_hover.as_ref(),
                    );
                }
                EditorMode::Finder => {
                    self.terminal
                        .draw_finder(&self.finder, &mut self.prompt_input);
                }
            }

            // Wait for the next event...
            if let Ok(ev) = self.event_queue.recv() {
                self.handle_event(ev);
            }

            // Receive other queued events.
            while let Ok(ev) = self.event_queue.try_recv() {
                self.handle_event(ev);
            }

            // Clear the notification.
            if matches!(&*self.notification.borrow(), Some(Notification { created_at, .. }) if created_at.elapsed().as_secs() > 5)
            {
                *self.notification.borrow_mut() = None;
            }

            self.update_modes();
        }
    }

    fn notify<T: Into<String>>(&self, level: NotificationLevel, message: T) {
        let message = message.into();
        trace!("notify: {}", message);
        *self.notification.borrow_mut() = Some(Notification {
            level,
            created_at: Instant::now(),
            message: message.replace("\n", " "),
        });
    }

    fn info<T: Into<String>>(&self, message: T) {
        self.notify(NotificationLevel::Info, message);
    }

    fn error<T: Into<String>>(&self, message: T) {
        self.notify(NotificationLevel::Error, message);
    }

    fn enter_in_finder(&mut self) {
        if let Some(item) = self.finder.selected_item() {
            match item {
                FinderItem::File { path, pos } => {
                    self.open_file(&path);
                    if let Some(pos) = pos {
                        self.current_buffer.borrow_mut().goto(pos.y, pos.x);
                    }
                }
                FinderItem::Buffer { path } => {
                    self.open_file(&path);
                }
            }
        }
    }

    fn update_modes(&mut self) {
        match self.mode {
            EditorMode::Normal => {}
            EditorMode::Finder => {
                let query = self.prompt_input.text();
                let being_updated = self.finder.query(&query);
                if being_updated {
                    for (i, buffer) in self.recent_buffers.iter().enumerate() {
                        self.finder
                            .provide_buffer(&query, &*buffer.borrow(), i as isize);
                    }
                }
            }
        }
    }

    fn handle_event(&mut self, ev: Event) {
        match ev {
            Event::Key(_) | Event::KeyBatch(_) | Event::Mouse(_) => {
                self.cursor_hover = None;
            }
            _ => {}
        }

        match ev {
            Event::Key(key) => self.handle_key_event(key),
            Event::KeyBatch(s) => self.handle_key_batch_event(s),
            Event::Mouse(mouse) => {
                if let Some(ev) = self.terminal.convert_raw_mouse_event(mouse) {
                    self.handle_mouse_event(ev);
                }
            }
            Event::Resize { rows, cols } => self.terminal.resize(rows, cols),
            Event::Redraw => {}
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
                    // Quit if all buffers are clean.
                    let num_unsaved = self
                        .buffers
                        .values()
                        .filter(|b| b.borrow().is_dirty())
                        .count();
                    if num_unsaved > 0 {
                        self.error(format!("{} files remain dirty", num_unsaved));
                    } else {
                        self.exited = true;
                    }
                }
                (KeyCode::Char('q'), ALT) => {
                    // Force quit.
                    self.exited = true;
                }
                (KeyCode::Char('s'), CTRL) => {
                    // Save the current buffer.
                    let mut buffer = self.current_buffer.borrow_mut();
                    match buffer.save(&self.backup_dir) {
                        Ok(_) => {
                            self.info(format!("saved {} lines", buffer.num_lines()));
                        }
                        Err(err) => {
                            self.error(format!("failed to save: {}", err));
                        }
                    }
                }
                (KeyCode::Char('o'), CTRL) => {
                    // Save all buffers.
                    let mut failure = None;
                    for buffer in self.buffers.values() {
                        let mut buffer = buffer.borrow_mut();
                        if let Err(err) = buffer.save(&self.backup_dir) {
                            failure = Some((buffer.name().to_string(), err));
                        }
                    }

                    match failure {
                        Some((name, err)) => {
                            self.error(format!("failed to save: {}: {}", name, err));
                        }
                        None => {
                            self.info(format!("saved all {} files", self.buffers.len()));
                        }
                    }
                }
                (KeyCode::Char('f'), CTRL) => {
                    self.prompt_input.clear();
                    self.mode = EditorMode::Finder;
                }
                //
                //  Text Editing.
                //
                (KeyCode::Char(ch), NONE) | (KeyCode::Char(ch), SHIFT) => {
                    self.current_buffer
                        .borrow_mut()
                        .insert_with_smart_indent(ch);
                }
                (KeyCode::Enter, NONE) => {
                    self.current_buffer
                        .borrow_mut()
                        .insert_with_smart_indent('\n');
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
                        buffer.set_cursor(Cursor::from_range(&current_word_range));
                        buffer.backspace();
                    }
                }
                (KeyCode::Char('w'), ALT) => {
                    let mut buffer = self.current_buffer.borrow_mut();
                    if let Some(current_word_range) = buffer.current_word_range() {
                        buffer.set_cursor(Cursor::from_range(&current_word_range));
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
                (KeyCode::Char('l'), CTRL) => {
                    self.current_buffer
                        .borrow_mut()
                        .centering(self.terminal.rows());
                }
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
                    self.current_buffer.borrow_mut().move_to_prev_block();
                }
                (KeyCode::Char('c'), ALT) => {
                    self.current_buffer.borrow_mut().move_to_next_block();
                }
                (KeyCode::Up, NONE) => {
                    self.current_buffer.borrow_mut().move_cursor_with_line_wrap(
                        self.terminal.text_cols(),
                        1,
                        0,
                    );
                }
                (KeyCode::Down, NONE) => {
                    self.current_buffer.borrow_mut().move_cursor_with_line_wrap(
                        self.terminal.text_cols(),
                        0,
                        1,
                    );
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
            EditorMode::Finder => match (key.code, key.modifiers) {
                (KeyCode::Char('q'), CTRL) | (KeyCode::Esc, _) => {
                    self.mode = EditorMode::Normal;
                }
                (KeyCode::Char(ch), NONE) | (KeyCode::Char(ch), SHIFT) => {
                    self.prompt_input.insert_char(ch);
                }
                (KeyCode::Enter, NONE) => {
                    self.enter_in_finder();
                    self.mode = EditorMode::Normal;
                }
                (KeyCode::Backspace, NONE) => {
                    self.prompt_input.backspace();
                }
                (KeyCode::Delete, NONE) | (KeyCode::Char('d'), CTRL) => {
                    self.prompt_input.delete();
                }
                (KeyCode::Left, NONE) => {
                    self.prompt_input.move_left();
                }
                (KeyCode::Right, NONE) => {
                    self.prompt_input.move_right();
                }
                (KeyCode::Up, _) => {
                    self.finder.move_prev();
                }
                (KeyCode::Down, _) => {
                    self.finder.move_next();
                }
                (KeyCode::Char('a'), CTRL) => {
                    self.prompt_input.move_to_beginning_of_line();
                }
                (KeyCode::Char('e'), CTRL) => {
                    self.prompt_input.move_to_end_of_line();
                }
                (KeyCode::Char('b'), ALT) => {
                    self.prompt_input.move_to_prev_word();
                }
                (KeyCode::Char('f'), ALT) => {
                    self.prompt_input.move_to_next_word();
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
            EditorMode::Finder => {
                self.prompt_input.insert(&s);
            }
        }
    }

    fn handle_mouse_event(&mut self, ev: MouseEvent) {
        trace!("mouse: {:?}", ev);
        let mut buffer = self.current_buffer.borrow_mut();
        match ev {
            MouseEvent::ClickText { pos, alt: true } => {
                // TODO:
            }
            MouseEvent::HoverText { pos, .. } => {
                self.cursor_hover = Some(pos);
            }
            MouseEvent::DoubleClickText { pos, .. } => {
                if let Some(range) = buffer.current_word_range() {
                    buffer.select_by_range(&range);
                } else {
                    buffer.goto(pos.y, pos.x)
                }
            }
            MouseEvent::ClickText { pos, .. } => {
                buffer.goto(pos.y, pos.x);
            }
            MouseEvent::ClickLineNo { y, .. } if y >= buffer.num_lines() => {
                // Out of bounds. Do nothing.
            }
            MouseEvent::ClickLineNo { y, .. } => {
                buffer.select_line(y);
            }
            MouseEvent::DragLineNo { y } => {
                let start_y = match buffer.cursor() {
                    Cursor::Selection { range, .. } => range.front().y,
                    Cursor::Normal { pos, .. } => pos.y,
                };

                let end_y = min(y, buffer.num_lines());
                let range = Range::new(start_y, 0, end_y, 0);
                buffer.select_by_range(&range);
            }
            MouseEvent::DragText { pos: mut drag_pos } => {
                let start_pos = match buffer.cursor() {
                    Cursor::Selection { range, .. } => range.start,
                    Cursor::Normal { pos, .. } => *pos,
                };

                drag_pos.y = min(drag_pos.y, buffer.num_lines() - 1);
                drag_pos.x = min(drag_pos.x, buffer.line_len(drag_pos.y));
                let range = Range::from_points(start_pos, drag_pos);
                buffer.select_by_range(&range);
            }
            MouseEvent::ScrollUp => buffer.scroll_up(3),
            MouseEvent::ScrollDown => buffer.scroll_down(3),
        }
    }
}
