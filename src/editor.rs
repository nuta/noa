use std::fs;
use std::rc::Rc;
use std::cmp::min;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::time::{Instant, Duration};
use git2::Repository;
use crate::watcher::FileWatcher;
use crate::buffer::{Buffer, BufferId, compute_str_checksum};
use crate::command_box::{CommandBox, RequestBody};
use crate::completion::WordCompJob;
use crate::view::View;
use crate::worker::Worker;
use crate::fuzzy::FuzzySet;
use crate::lsp::Lsp;
use crate::rope::Point;
use crate::terminal::{
    Terminal, KeyCode, KeyModifiers, KeyEvent, RawMouseEvent, MouseEvent,
};
use crate::status_map::{compute_git_diff, StatusMap};

pub enum NotificationLevel {
    Report,
    Info,
    Error,
}

pub struct Notification {
    pub level: NotificationLevel,
    pub message: String,
    pub created_at: Instant,
    pub persist: bool,
}

impl PartialEq for Notification {
    fn eq(&self, other: &Notification) -> bool {
        self.message == other.message
    }
}

pub struct Popup {
    items: Vec<String>,
    selected: usize,
}

impl Popup {
    pub fn new(items: Vec<String>) -> Popup {
        Popup {
            items,
            selected: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn items(&self) -> &[String] {
        &self.items
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_str(&self) -> &str {
        &self.items[self.selected]
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_next(&mut self) {
        self.selected = min(self.selected + 1, self.len().saturating_sub(1));
    }
}

pub enum Event {
    Key(KeyEvent),
    Mouse(RawMouseEvent),
    FileChanged(PathBuf),
    NoCompletion,
    Completion {
        buffer_id: BufferId,
        items: FuzzySet,
    },
    GoTo {
        path: PathBuf,
        pos: Point,
    },
    HoverMessage {
        message: String,
    },
    Resize {
        rows: usize,
        cols: usize,
    }
}

#[derive(Clone)]
pub struct EventQueue {
    tx: Sender<Event>,
}

impl EventQueue {
    pub fn new(tx: Sender<Event>) -> EventQueue {
        EventQueue {
            tx,
        }
    }

    pub fn enqueue(&self, ev: Event) {
        self.tx.send(ev).unwrap();
    }
}

pub enum EditorMode {
    Normal,
    CommandBox,
}

pub struct Editor {
    mode: EditorMode,
    terminal: Terminal,
    current: Rc<RefCell<View>>,
    views: Vec<Rc<RefCell<View>>>,
    event_queue: Receiver<Event>,
    exited: bool,
    notifications: RefCell<Vec<Notification>>,
    popup: Option<Popup>,
    popup_selected: usize,
    worker: Worker,
    command_box: CommandBox,
    command_box_input: Buffer,
    workspace_dir: PathBuf,
    backup_dir: PathBuf,
    git: Option<Repository>,
    lsp: Lsp,
    watcher: FileWatcher,
    status_map: StatusMap,
    time_last_clicked: Instant,
}

impl Editor {
    pub fn new(workspace_dir: PathBuf) -> Editor {
        let (tx, rx) = channel();
        let scratch_buffer = Rc::new(RefCell::new(Buffer::new()));
        scratch_buffer.borrow_mut().set_name("*scratch*");
        let scratch_view = Rc::new(RefCell::new(View::new(scratch_buffer)));

        let lsp = Lsp::new(&workspace_dir, EventQueue::new(tx.clone()));
        let git = match Repository::open(&workspace_dir) {
            Ok(repo) => Some(repo),
            Err(err) => {
                error!("failed to open a Git repository: {}", err);
                None
            }
        };

        Editor {
            mode: EditorMode::Normal,
            terminal: Terminal::new(EventQueue::new(tx.clone())),
            current: scratch_view.clone(),
            views: vec![scratch_view],
            event_queue: rx,
            exited: false,
            notifications: RefCell::new(Vec::new()),
            popup: None,
            popup_selected: 0,
            worker: Worker::new(EventQueue::new(tx.clone())),
            command_box: CommandBox::new(),
            command_box_input: Buffer::new(),
            workspace_dir,
            backup_dir: dirs::home_dir().unwrap().join(".noa/backup"),
            git,
            lsp,
            watcher: FileWatcher::new(tx),
            status_map: StatusMap::new(),
            time_last_clicked: Instant::now(),
        }
    }

    pub fn workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    pub fn open_file(&mut self, path: &Path) {
        let abspath = match path.canonicalize() {
            Ok(abspath) => abspath,
            Err(err) => {
                self.error(format!("failed to resolve path: {} ({})", path.display(), err));
                return;
            }
        };

        for view in &self.views {
            match view.borrow().buffer().borrow().file() {
                Some(view_path) if abspath == *view_path => {}
                _ => continue,
            }

            // We have already opened the file. Switch to the view.a
            self.current = view.clone();
            self.after_switching_buffer();
            return;
        }

        let mut buffer = match Buffer::open_file(path) {
            Ok(buffer) => buffer,
            Err(err) => {
                self.error(format!("failed to open: {} ({})", path.display(), err));
                return;
            }
        };

        buffer.set_name(path.file_name().unwrap().to_str().unwrap());
        self.lsp.open_buffer(&buffer);

        let view = Rc::new(RefCell::new(View::new(Rc::new(RefCell::new(buffer)))));
        self.views.push(view.clone());
        self.watcher.start_watching(path);
        self.current = view;

        self.after_switching_buffer();
    }

    pub fn switch_buffer(&mut self, buffer_id: BufferId) {
        for view in &self.views {
            if buffer_id == view.borrow().buffer().borrow().id() {
                self.current = view.clone();
                self.after_switching_buffer();
                return;
            }
        }

        warn!("failed to switch to the buffer {:?}", buffer_id);
    }

    fn after_switching_buffer(&mut self) {
        self.update_status_map();
    }

    pub fn run(&mut self) {
        self.on_modified();
        self.draw();
        loop {
            if self.exited {
                return;
            }

            match self.event_queue.recv() {
                Ok(ev) => {
                    let started_at = Instant::now();
                    let snapshot = self.current
                        .borrow().buffer().borrow().snapshot();

                    self.handle_event(ev);
                    while let Ok(ev) = self.event_queue.try_recv() {
                        self.handle_event(ev);
                    }

                    let current = self.current
                        .borrow().buffer().borrow().snapshot();
                    if snapshot.buffer_id != current.buffer_id || snapshot.buf != current.buf {
                        self.on_modified();
                    }

                    self.draw();
                    trace!("took {} us", started_at.elapsed().as_micros());
                }
                Err(err) => {
                   warn!("failed recv from the event queue: {:?}", err);
                }
            }
        }
    }

    fn draw(&mut self) {
        let mut view = self.current.borrow_mut();
        self.terminal.draw(
            &mut *view,
            &*self.notifications.borrow(),
            &self.popup,
            match self.mode {
                EditorMode::CommandBox => Some((&self.command_box, &self.command_box_input)),
                EditorMode::Normal => None,
            },
            &self.status_map,
        );
    }

    fn on_modified(&mut self) {
        let view = self.current.borrow();
        let buffer = view.buffer().borrow();
        self.lsp.modify_buffer(&buffer);
        self.lsp.request_signature_help(&buffer);
    }

    fn update_status_map(&mut self) {
        let view = self.current.borrow();
        let buffer = view.buffer().borrow();

        self.status_map.clear();
        if let Some(git) = self.git.as_ref() {
            match compute_git_diff(&mut self.status_map, git, &*buffer) {
                Ok(_) => {}
                Err(err) => { trace!("failed to get diff: {}", err); }
            }
        }
   }

    fn notify<T: Into<String>>(&self, level: NotificationLevel, message: T) {
        let message = message.into();
        trace!("notification: {}", message);
        self.notifications.borrow_mut().push(Notification {
            level,
            created_at: Instant::now(),
            persist: false,
            message: message.replace("\n", " "),
        });
    }

    fn hover_message<T: Into<String>>(&self, message: T) {
        let message = message.into();
        let duplicated = match self.notifications.borrow_mut().last() {
            Some(last) => last.message == message,
            None => false,
        };

        if !duplicated {
            self.notifications.borrow_mut().push(Notification {
                level: NotificationLevel::Report,
                created_at: Instant::now(),
                persist: true,
                message: message.replace("\n", " "),
            });
        }
    }

    fn report<T: Into<String>>(&self, message: T) {
        self.notify(NotificationLevel::Report, message);
    }

    fn info<T: Into<String>>(&self, message: T) {
        self.notify(NotificationLevel::Info, message);
    }

    fn error<T: Into<String>>(&self, message: T) {
        self.notify(NotificationLevel::Error, message);
    }

    fn handle_event(&mut self, ev: Event) {
        match ev {
            Event::Key(key) => {
                match self.mode {
                    EditorMode::Normal => {
                        self.handle_key_event(key);
                    }
                    EditorMode::CommandBox => {
                        self.handle_key_event_in_command_box(key);
                    }
                }
            }
            Event::Mouse(mouse) => {
                if let Some(ev) = self.terminal.convert_raw_mouse_event(mouse) {
                    self.handle_mouse_event(ev);
                }
            }
            Event::Resize { rows, cols } => {
                self.terminal.resize(rows, cols);
            }
            Event::NoCompletion => {
                self.clear_popup();
            }
            Event::Completion { buffer_id, items } => {
                self.handle_completion_event(buffer_id, items);
            }
            Event::HoverMessage { message } => {
                self.hover_message(message);
            }
            Event::GoTo { path, pos } => {
                self.open_file(&path);
                let mut view = self.current.borrow_mut();
                view.goto(pos.y, pos.x);
                view.centering(self.terminal.rows());
            }
            Event::FileChanged(_path) => {
                self.check_if_current_changed();
            }
        }
    }

    fn check_if_current_changed(&mut self) {
        let view = self.current.borrow_mut();
        let mut buffer = view.buffer().borrow_mut();
        if let Some(path) = buffer.path() {
            let new_text = match fs::read_to_string(path) {
                Ok(new_text) => new_text,
                Err(err) => {
                    let message = format!("failed to reload {}: {}", path.display(), err);
                    self.report(message);
                    return;
                }
            };

            if buffer.checksum() != compute_str_checksum(&new_text) {
                // It looks the file has changed on disk. Reload the file if the
                // buffer is not dirty.
                if buffer.is_dirty() {
                    self.info("this file has changed on disk");
                } else {
                    buffer.set_text(&new_text);
                    self.info("changes on disk detected; reloaded the file");
                }
            }
        }
    }

    fn is_popup_active(&self) -> bool {
        self.popup.is_some()
    }

    fn clear_popup(&mut self) {
        self.popup = None;
    }

    fn handle_completion_event(&mut self, id: BufferId, items: FuzzySet) {
        let view = self.current.borrow();
        let buffer = view.buffer().borrow();
        if buffer.id() != id {
            drop(buffer);
            drop(view);
            self.clear_popup();
            return;
        }

        self.popup_selected = 0;
        self.popup = {
            buffer.current_word()
                .map(|current_word| {
                    let mut filtered = Vec::new();
                    for item in items.search(&current_word, 5) {
                        if item != current_word {
                            filtered.push(item.to_owned());
                        }
                    }

                    if filtered.is_empty() {
                        None
                    } else {
                        Some(Popup::new(filtered))
                    }
                })
                .unwrap_or(None)
        };
    }

    fn open_command_box(&mut self, prefix: &str) {
        self.command_box_input.clear();
        self.command_box_input.insert(prefix);
        self.mode = EditorMode::CommandBox;
        self.command_box.open();
    }

    fn close_command_box(&mut self) {
        self.mode = EditorMode::Normal;
    }

    fn execute_command(&mut self, preview: bool) {
        use crate::command_box::{Request, Response, ResponseBody};
        use crate::search::{list_files, grep_buffer, grep_dir, NUM_MATCHES_MAX};

        let input = self.command_box_input.text();
        let mut words = input.splitn(2, ' ');
        let pat = words.next().unwrap();
        let script = words.next().unwrap_or("").to_owned();

        let body;
        let mut pat_chars = pat.chars();
        match pat_chars.next() {
            Some('g') => {
                // Search all files.
                if pat.len() < 5 {
                    self.report("too short pattern");
                    return;
                }

                let pat = &pat[2..];
                match grep_dir(self.workspace_dir(), pat) {
                    Ok(locations) => {
                        if locations.len() >= NUM_MATCHES_MAX {
                            self.error("aborted due to too many matches");
                        }
                        body = RequestBody::SelectMatch { locations };
                    }
                    Err(err) => {
                        self.error(format!("grep_dir: {}", err));
                        return;
                    }
                }
            }
            Some('f') => {
                // Search the current buffer.
                if pat.len() < 3 {
                    self.report("too short pattern");
                    return;
                }

                let view = self.current.borrow();
                let mut buffer = view.buffer().borrow_mut();
                buffer.update_tmpfile();
                match grep_buffer(&*buffer, &pat[2..]) {
                    Ok(locations) => {
                        if locations.len() >= NUM_MATCHES_MAX {
                            self.error("aborted due to too many matches");
                        }
                        body = RequestBody::SelectMatch { locations };
                    }
                    Err(err) => {
                        self.error(format!("grep: {}", err));
                        return;
                    }
                }
            }
            Some('p') => {
                // Filter file paths.
                if pat.len() < 3 {
                    self.report("too short pattern");
                    return;
                }

                let files = list_files(self.workspace_dir(), &pat[2..]);
                if files.len() >= NUM_MATCHES_MAX {
                    self.error("aborted due to too many matches");
                }
                body = RequestBody::SelectFile { files };
            }
            _ => {
                self.error("invalid prefix");
                return;
            }
        }

        let req = Request {
            script: script.clone(),
            selected: self.command_box.selected(),
            preview,
            body,
        };

        match self.command_box.execute(req) {
            Ok(()) => {}
            Err(err) => {
                error!("ruby script error: {}", err);
                self.error(format!("{}", err));
            }
        }

        let last_stderr = self.command_box.last_stderr();
        if !last_stderr.is_empty() {
            error!("stderr from ruby script: {}\n{}", script, last_stderr);
        }

        // Handle the response.
        let resp = self.command_box.last_response().cloned();
        if let Some(Response { body, .. }) = resp {
            match body {
                ResponseBody::Preview { items } => {
                    self.info(format!("found {} items", items.len()));
                }
                ResponseBody::GoTo { file, position } => {
                    if let Some(buffer_id) = file.buffer_id {
                        self.switch_buffer(buffer_id);
                    } else {
                        self.open_file(&file.path);
                    }

                    if let Some(pos) = position {
                        let mut view = self.current.borrow_mut();
                        view.goto(pos.y, pos.x);
                        view.centering(self.terminal.rows());
                    }

                    self.close_command_box();
                }
            }
        }
    }

    fn handle_key_event_in_command_box(&mut self, key: KeyEvent) {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let mut modified = false;
        match (key.code, key.modifiers) {
            (KeyCode::Enter, NONE) => {
                self.execute_command(false);
            }
            (KeyCode::Esc, NONE)
            | (KeyCode::Char('m'), CTRL) => {
                self.close_command_box();
                return;
            }
            (KeyCode::Up, NONE) => {
                self.command_box.move_up();
            }
            (KeyCode::Down, NONE) => {
                self.command_box.move_down();
            }
            (KeyCode::Char('a'), CTRL) => {
                self.command_box_input.move_to_beginning_of_line();
            }
            (KeyCode::Char('e'), CTRL) => {
                self.command_box_input.move_to_end_of_line();
            }
            (KeyCode::Left, NONE) => {
                self.command_box_input.move_cursors(0, 0, 1, 0);
            }
            (KeyCode::Right, NONE) => {
                self.command_box_input.move_cursors(0, 0, 0, 1);
            }
            (KeyCode::Char('k'), CTRL) => {
                self.command_box_input.truncate();
                modified = true;
            }
            (KeyCode::Char('r'), CTRL) => {
                let input = self.command_box_input.text();
                match input.chars().next() {
                    Some(first_ch @ 'a' ..= 'z') => {
                        let mut new_input = String::new();
                        new_input.push(first_ch.to_ascii_uppercase());
                        new_input.push_str(&input[1..]);
                        self.command_box_input.set_text(&new_input);
                    }
                    _ => {}
                }
                modified = true;
            }
            (KeyCode::Char(ch), NONE)
            | (KeyCode::Char(ch), SHIFT) => {
                self.command_box_input.insert_char(ch);
                modified = true;
            }
            (KeyCode::Backspace, NONE) => {
                self.command_box_input.backspace();
                modified = true;
            }
            (KeyCode::Delete, NONE)
            | (KeyCode::Char('d'), CTRL) => {
                self.command_box_input.delete();
                modified = true;
            }
            _ => {
                trace!("unhandled key event: {:?}", key);
            }
        }

        if modified {
            self.execute_command(true);
        }
    }

    fn handle_mouse_event(&mut self, ev: MouseEvent) {
        trace!("mouse: {:?}", ev);
        trace!("{:?}", self.time_last_clicked.elapsed());
        let mut view = self.current.borrow_mut();
        match ev {
            MouseEvent::ClickedText { pos, alt: true } => {
                view.goto(pos.y, pos.x);
                self.lsp.request_goto_definition(&*view.buffer().borrow());
            }
            MouseEvent::ClickedText { pos, .. }
                if self.time_last_clicked.elapsed() < Duration::from_millis(400)
            => {
                let mut buffer = view.buffer().borrow_mut();
                trace!("clicked multi");
                if let Some(range) = buffer.current_word_range() {
                    buffer.select_by_ranges(&[range]);
                } else {
                    drop(buffer);
                    view.goto(pos.y, pos.x)
                }
            }
            MouseEvent::ClickedText { pos, .. } => {
                view.goto(pos.y, pos.x);
                self.time_last_clicked = Instant::now();
            }
            MouseEvent::ScrollUp => view.scroll_up(5),
            MouseEvent::ScrollDown => view.scroll_down(5),
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let mut view = self.current.borrow_mut();
        let mut buffer = view.buffer().borrow_mut();
        let mut clear_popup = true;
        let mut update_completion = false;
        match (key.code, key.modifiers) {
            (KeyCode::Up, NONE) if self.is_popup_active() => {
                self.popup.as_mut().unwrap().select_prev();
                clear_popup = false;
            }
            (KeyCode::Down, NONE) if self.is_popup_active() => {
                self.popup.as_mut().unwrap().select_next();
                clear_popup = false;
            }
            (KeyCode::Esc, NONE) if self.is_popup_active() => {
                // Clear the popup.
            }
            (KeyCode::Enter, NONE) | (KeyCode::Tab, NONE) if self.is_popup_active() => {
                let selected = self.popup.as_mut().unwrap().selected_str();
                if let Some(current_word_range) = buffer.current_word_range() {
                    buffer.select_by_ranges(&[current_word_range]);
                    buffer.backspace();
                    buffer.insert(selected);
                }
            }
            (KeyCode::Char('q'), CTRL) => {
                self.exited = true;
            }
            (KeyCode::Char('l'), CTRL) => {
                drop(buffer);
                view.centering(self.terminal.rows());
                return;
            }
            (KeyCode::Char('s'), CTRL) => {
                match buffer.save(&self.backup_dir) {
                    Ok(_) => {
                        self.info(format!("saved ({} lines)", buffer.num_lines()));
                        drop(buffer);
                        drop(view);
                        self.update_status_map();
                        return;
                    }
                    Err(err) => {
                        self.error(format!("failed to save: {}", err));
                    }
                }
            }
            (KeyCode::Char('x'), CTRL) => {
                drop(buffer);
                drop(view);
                self.open_command_box("");
                return;
            }
            (KeyCode::Char('f'), CTRL) => {
                drop(buffer);
                drop(view);
                self.open_command_box("f/");
                return;
            }
            (KeyCode::Char('g'), CTRL) => {
                drop(buffer);
                drop(view);
                self.open_command_box("g/");
                return;
            }
            (KeyCode::Char('p'), CTRL) => {
                drop(buffer);
                drop(view);
                self.open_command_box("p/");
                return;
            }
            (KeyCode::Char('k'), CTRL) => {
                buffer.truncate();
            }
            (KeyCode::Char('z'), CTRL) => {
                buffer.undo();
                self.report("undo");
            }
            (KeyCode::Char('r'), CTRL) => {
                buffer.redo();
                self.report("redo");
            }
            (KeyCode::Char('a'), CTRL) => {
                buffer.move_to_beginning_of_line();
            }
            (KeyCode::Char('e'), CTRL) => {
                buffer.move_to_end_of_line();
            }
            (KeyCode::Char('b'), ALT) => {
                buffer.move_to_prev_word();
            }
            (KeyCode::Char('f'), ALT) => {
                buffer.move_to_next_word();
            }
            (KeyCode::Char('w'), ALT) => {
                if let Some(current_word) = buffer.current_word() {
                    let matches = &buffer.find(&current_word);
                    buffer.select_by_ranges(matches);
                }
            }
            (KeyCode::Char('w'), CTRL) => {
                if let Some(current_word_range) = buffer.current_word_range() {
                    buffer.select_by_ranges(&[current_word_range]);
                    buffer.backspace();
                }
            }
            (KeyCode::Char(ch), NONE) | (KeyCode::Char(ch), SHIFT) => {
                buffer.insert_char(ch);
                update_completion = true;
                clear_popup = false;
            }
            (KeyCode::Tab, NONE) => {
                buffer.tab();
            }
            (KeyCode::BackTab, NONE) => {
                buffer.back_tab();
            }
            (KeyCode::Enter, NONE) => {
                buffer.insert_char('\n');
            }
            (KeyCode::Char('j'), CTRL) => {
                buffer.insert_char('\n');
                buffer.tab();
            }
            (KeyCode::Backspace, NONE) => {
                buffer.backspace();
                update_completion = true;
            }
            (KeyCode::Delete, NONE) | (KeyCode::Char('d'), CTRL) => {
                buffer.delete();
                update_completion = true;
            }
            (KeyCode::PageUp, NONE) => {
                drop(buffer);
                view.scroll_up(self.terminal.rows());
                return;
            }
            (KeyCode::PageDown, NONE) => {
                drop(buffer);
                view.scroll_down(self.terminal.rows());
                return;
            }
            (KeyCode::Char('d'), ALT) => {
                drop(buffer);
                view.scroll_up(5);
                return;
            }
            (KeyCode::Char('c'), ALT) => {
                drop(buffer);
                view.scroll_down(5);
                return;
            }
            (KeyCode::Up, NONE) => {
                buffer.move_cursors(1, 0, 0, 0);
            }
            (KeyCode::Down, NONE) => {
                buffer.move_cursors(0, 1, 0, 0);
            }
            (KeyCode::Left, NONE) => {
                buffer.move_cursors(0, 0, 1, 0);
            }
            (KeyCode::Right, NONE) => {
                buffer.move_cursors(0, 0, 0, 1);
            }
            (KeyCode::Up, SHIFT) => {
                buffer.select(1, 0, 0, 0);
            }
            (KeyCode::Down, SHIFT) => {
                buffer.select(0, 1, 0, 0);
            }
            (KeyCode::Left, SHIFT) => {
                buffer.select(0, 0, 1, 0);
            }
            (KeyCode::Right, SHIFT) => {
                buffer.select(0, 0, 0, 1);
            }
            _ => {
                trace!("unhandled key event: {:?}", key);
            }
        }

        drop(buffer);
        drop(view);

        if clear_popup {
            self.clear_popup();
        }

        let view = self.current.borrow_mut();
        let buffer = view.buffer().borrow_mut();
        if update_completion {
            // Run a completion.
            let snapshot = buffer.snapshot();
            self.worker.request(Box::new(WordCompJob::new(snapshot)));
            self.lsp.request_completions(&*buffer);
        }
    }
}
