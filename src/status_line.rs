use std::{borrow::Cow, path::Path};

use crate::{buffer::Buffer, terminal::Frame, utils::HOME_DIR};

fn format_path(cwd: &Path, path: &Path) -> String {
    if let Ok(rel_path) = path.strip_prefix(cwd) {
        rel_path.to_string_lossy().to_string()
    } else if let Ok(rel_path) = path.strip_prefix(&*HOME_DIR) {
        format!("~/{}", rel_path.display())
    } else {
        path.to_string_lossy().to_string()
    }
}

pub enum Level {
    Info,
    Warning,
    Error,
}

pub struct Message {
    level: Level,
    message: Cow<'static, str>,
}

pub struct StatusLine {
    message: Option<Message>,
}

impl StatusLine {
    pub fn new() -> Self {
        Self { message: None }
    }

    pub fn message(&mut self, level: Level, message: impl Into<Cow<'static, str>>) {
        self.message = Some(Message {
            level,
            message: message.into(),
        });
    }

    pub fn render(&self, frame: &mut Frame, y: u16, buffer: &Buffer, cwd: &Path, path: &Path) {
        frame.fill_reversed(y, 0, frame.width);
        frame.draw_str(y, 1, &format_path(cwd, path));

        if let Some(Message { level, message }) = &self.message {
            frame.draw_str(y + 1, 1, &message);
        }
    }
}
