use std::path::Path;

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

pub struct StatusLine {}

impl StatusLine {
    pub fn new() -> Self {
        Self {}
    }

    pub fn render(&self, frame: &mut Frame, buffer: &Buffer, cwd: &Path, path: &Path) {
        let y = frame.height.saturating_sub(2);
        if y < 3 {
            frame.draw_str(y, 0, "too small view");
            return;
        }

        frame.draw_str(y, 1, &format_path(cwd, path));

        frame.fill_reversed(y, 0, frame.width);
    }
}
