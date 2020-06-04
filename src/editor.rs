use crate::buffer::Buffer;
use crate::diff::Line;
use crate::finder::Finder;
use crate::terminal::{Key, Rgb, Terminal};
use crate::clipboard::{copy_from_clipboard, copy_into_clipboard};
use crate::lsp::Lsp;
use signal_hook::{self, iterator::Signals};
use std::cell::RefCell;
use std::cmp::min;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

pub enum Event {
    Key(Key),
    ForceRender,
    ScreenResized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorMode {
    Normal,
    Finder,
}

pub struct Editor {
    mode: EditorMode,
    term: Terminal,
    rx: Receiver<Event>,
    tx: Sender<Event>,
    repo_dir: PathBuf,
    buffers: Vec<Rc<RefCell<Buffer>>>,
    current: Rc<RefCell<Buffer>>,
    messages: Vec<String>,
    statuses: HashMap<&'static str, (String, Rgb)>,
    hide_message_after: usize,
    quitting: bool,
    prompt_input: Line,
    prompt_cursor: usize,
    prompt_selected: usize,
    finder: Finder,
    lsp: Lsp,
}

impl Editor {
    pub fn new() -> Editor {
        let (tx, rx) = mpsc::channel();
        tx.send(Event::ForceRender).unwrap();

        let scratch = Rc::new(RefCell::new(Buffer::new()));
        scratch
            .borrow_mut()
            .set_display_name("*scratch*".to_owned());

        // Handle signals.
        let tx2 = tx.clone();
        std::thread::spawn(move || {
            let signals = Signals::new(&[signal_hook::SIGWINCH]).unwrap();
            for signal in &signals {
                match signal {
                    signal_hook::SIGWINCH => {
                        tx2.send(Event::ScreenResized).ok();
                    }
                    _ => {
                        warn!("unhandled signal: {}", signal);
                    }
                }
            }

            unreachable!();
        });

        let repo_dir = std::env::current_dir().unwrap().to_path_buf();
        Editor {
            mode: EditorMode::Normal,
            term: Terminal::new(tx.clone()),
            lsp: Lsp::new(tx.clone()).expect("failed to spawn lsp server"),
            tx,
            rx,
            repo_dir,
            buffers: vec![scratch.clone()],
            current: scratch,
            messages: Vec::new(),
            statuses: HashMap::new(),
            hide_message_after: 0,
            quitting: false,
            prompt_input: Line::new(),
            prompt_cursor: 0,
            prompt_selected: 0,
            finder: Finder::new(),
        }
    }

    pub fn open_file(&mut self, path: &Path) {
        match Buffer::open_file(path) {
            Ok(buffer) => {
                if !path.exists() {
                    self.notify("(new file)");
                }

                if let Some(backup_path) = buffer.backup_path() {
                    if backup_path.exists() {
                        self.notify(&format!("backup exists: {}", backup_path.display()));
                    }
                }

                let buffer = Rc::new(RefCell::new(buffer));
                self.buffers.push(buffer.clone());
                self.current = buffer;
                self.update_display_names();
            }
            Err(err) => {
                self.notify(&format!("failed to open {} ({})", path.display(), err));
            }
        }
    }

    fn update_display_names(&self) {
        // TODO:
    }

    fn set_status(&mut self, status: &'static str, body: String) {
        let color = match status {
            "modified" => Rgb::new(90, 50, 14),
            "unsaved" => Rgb::new(125, 48, 31),
            _ => unreachable!(),
        };

        self.statuses.insert(status, (body, color));
    }

    fn unset_status(&mut self, status: &'static str) {
        self.statuses.remove(status);
    }

    fn notify(&mut self, s: &str) {
        info!("{}", s);
        self.messages.push(s.to_owned());
        self.hide_message_after = 3;
    }

    pub fn run(&mut self) {
        loop {
            let ev = match self.rx.recv_timeout(Duration::from_millis(500)) {
                Ok(ev) => ev,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    self.interval_work();
                    continue;
                }
                Err(err) => {
                    trace!("failed to receive a event: {}", err);
                    return;
                }
            };

            let started_at = std::time::SystemTime::now();
            self.process(ev);
            trace!("took {}us", started_at.elapsed().unwrap().as_micros());

            if self.quit() {
                break;
            }

            self.update_statuses();
            self.render();
        }
    }

    fn quit(&mut self) -> bool {
        if !self.quitting {
            return false;
        }

        self.quitting = false;

        let num_unsaved = self.num_unsaved_files();
        if num_unsaved > 0 {
            self.notify(&format!(
                "can't quit: {} files are still unsaved!",
                num_unsaved
            ));
            return false;
        }

        true
    }

    fn interval_work(&mut self) {
        self.current.borrow_mut().add_undo_stop();
    }

    fn process(&mut self, ev: Event) {
        match ev {
            Event::Key(key) => {
                trace!("key = {:?}", key);
                match self.mode {
                    EditorMode::Normal => self.input_in_editor(key),
                    EditorMode::Finder => self.input_in_prompt(key),
                }
            }
            Event::ScreenResized => {
                self.term.update_screen_size();
                // Adjust the cursor positions.
                self.current.borrow_mut().move_by(0, 0);
            }
            Event::ForceRender => {}
        }
    }

    fn num_unsaved_files(&self) -> usize {
        let mut num_unsaved = 0;
        for buffer in &self.buffers {
            let mut buffer = buffer.borrow_mut();
            if buffer.file().is_some() && buffer.modified() {
                num_unsaved += 1;
            }
        }
        num_unsaved
    }

    fn update_statuses(&mut self) {
        if self.current.borrow_mut().modified() {
            self.set_status("modified", "[+]".to_owned());
        } else {
            self.unset_status("modified");
        }

        let num_unsaved = self.num_unsaved_files();
        if num_unsaved > 1 {
            self.set_status("unsaved", format!("{} unsaved", num_unsaved));
        } else {
            self.unset_status("unsaved");
        }
    }

    fn render(&mut self) {
        // Update the screen.
        match self.mode {
            EditorMode::Normal => {
                let message = if self.hide_message_after == 0 {
                    None
                } else {
                    self.messages.last().map(|s| s.as_str())
                };

                self.term.render_editor(
                    &mut *self.current.borrow_mut(),
                    message,
                    self.statuses.values(),
                );
            }
            EditorMode::Finder => {
                self.term.render_prompt(
                    ">",
                    &self.prompt_input,
                    self.prompt_cursor,
                    self.prompt_selected,
                    self.finder.filtered(),
                );
            }
        }
    }

    fn input_in_editor(&mut self, key: Key) {
        self.hide_message_after = self.hide_message_after.saturating_sub(1);
        match key {
            Key::Ctrl('q') => {
                self.quitting = true;
            }
            Key::Ctrl('f') => {
                self.finder.reload(&self.repo_dir, &self.buffers);
                self.prompt_input.clear();
                self.prompt_cursor = 0;
                self.prompt_selected = 0;
                self.mode = EditorMode::Finder;
            }
            Key::Ctrl('s') => {
                if self.current.borrow().file().is_some() {
                    let result = self.current.borrow_mut().save();
                    match result {
                        Ok(_) => {
                            let num_lines = self.current.borrow().num_lines();
                            self.notify(&format!("wrote {} lines", num_lines));
                        }
                        Err(err) => {
                            self.notify(&format!("failed to save: {}", err));
                        }
                    }
                }
            }
            Key::Ctrl('k') => {
                self.current.borrow_mut().truncate();
            }
            Key::Char('\t') => {
                self.current.borrow_mut().tab(false);
            }
            Key::Char(ch) => {
                if ch == '\n' {
                    let result = self.current.borrow().backup();
                    if let Err(err) = result {
                        self.notify(&format!("failed to backup: {}", err));
                    }
                }

                self.current.borrow_mut().insert(ch);
            }
            Key::Esc => {
                let mut buffer = self.current.borrow_mut();
                buffer.end_selection();
                buffer.clear_cursors();
            }
            Key::Ctrl('x') => {
                copy_into_clipboard(self.current.borrow_mut().cut());
                self.notify("cut");
            }
            Key::Ctrl('c') => {
                copy_into_clipboard(self.current.borrow_mut().copy());
                self.notify("copied");
            }
            Key::Ctrl('v') => {
                self.current.borrow_mut().paste(&copy_from_clipboard());
                self.notify("pasted");
            }
            Key::Ctrl('o') => {
                self.current.borrow_mut().start_selection();
                self.notify("selection");
            }
            Key::Alt('w') => {
                // TODO:
                let new_cursor = crate::diff::Point {
                    y: self.current.borrow_mut().cursors().len(),
                    x: 0
                };
                self.current.borrow_mut().add_cursor(new_cursor);
            }
            Key::Backspace => {
                self.current.borrow_mut().backspace();
            }
            Key::Delete | Key::Ctrl('d') => {
                self.current.borrow_mut().delete();
            }
            Key::Up => {
                self.current.borrow_mut().move_by(-1, 0);
            }
            Key::Down => {
                self.current.borrow_mut().move_by(1, 0);
            }
            Key::Right => {
                self.current.borrow_mut().move_by(0, 1);
            }
            Key::Left => {
                self.current.borrow_mut().move_by(0, -1);
            }
            Key::Ctrl('a') => {
                self.current.borrow_mut().move_to_begin();
            }
            Key::Ctrl('e') => {
                self.current.borrow_mut().move_to_end();
            }
            Key::Ctrl('u') => {
                self.current.borrow_mut().undo();
            }
            Key::Ctrl('y') => {
                self.current.borrow_mut().redo();
            }
            Key::PageUp | Key::Alt('d') => {
                self.current.borrow_mut().scroll_up(self.term.text_height());
            }
            Key::PageDown | Key::Alt('c') => {
                self.current
                    .borrow_mut()
                    .scroll_down(self.term.text_height());
            }
            _ => {
                trace!("unhandled key input: {:?}", key);
            }
        }

        // Adjust the cursor position.
        // FIXME: Do this in buffer.
        self.current.borrow_mut().move_by(0, 0);
    }

    fn input_in_prompt(&mut self, key: Key) {
        match key {
            Key::Ctrl('q') | Key::Ctrl('f') | Key::Esc => {
                self.mode = EditorMode::Normal;
            }
            Key::Backspace => {
                if self.prompt_cursor > 0 {
                    self.prompt_cursor -= 1;
                    self.prompt_input.remove(self.prompt_cursor);
                }
            }
            Key::Left => {
                self.prompt_cursor = self.prompt_cursor.saturating_sub(1);
            }
            Key::Right => {
                self.prompt_cursor = min(self.prompt_cursor + 1, self.prompt_input.len());
            }
            Key::Up => {
                self.prompt_selected = self.prompt_selected.saturating_sub(1);
            }
            Key::Down | Key::Char('\t') => {
                self.prompt_selected += 1;
            }
            Key::Char('\n') => {
                if !self.finder.filtered().is_empty() {
                    self.mode = EditorMode::Normal;
                    self.select_finder_item();
                }
            }
            Key::Char(ch) => {
                self.prompt_input.insert(self.prompt_cursor, ch);
                self.prompt_cursor += 1;
            }
            _ => {
                trace!("unhandled key input: {:?}", key);
            }
        }

        self.finder.filter(self.prompt_input.as_str());
        self.prompt_selected = min(
            self.prompt_selected,
            self.finder.filtered().len().saturating_sub(1),
        );
    }

    fn select_finder_item(&mut self) {
        let item = &self.finder.filtered()[self.prompt_selected];
        trace!("select: {}", item.title);
        match item.label {
            // Buffer.
            'b' | '*' => {
                for buffer in &self.buffers {
                    if buffer.borrow().display_name() == item.title {
                        self.current = buffer.clone();
                    }
                }
            }
            // Path.
            'p' => {
                let path = &Path::new(&item.title).to_path_buf();
                self.open_file(&path);
            }
            _ => unreachable!(),
        }
    }
}
